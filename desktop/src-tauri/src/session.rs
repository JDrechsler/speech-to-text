use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

use crate::audio::{self, Recorder};
use crate::cloud::{self, CloudRequest};
use crate::dictionary;
use crate::engines::Engine;
use crate::frontmost;
use crate::history::{self, RecordingEntry};
use crate::media;
use crate::microphone;
use crate::paste;
use crate::profiles::{self, ActiveProfile};
use crate::rewrite;
use crate::settings::Settings;
use crate::state::{self, Phase, Shared};
use crate::tray;
use crate::whisper;
use crate::windows;

const WHISPER_IDLE_UNLOAD: Duration = Duration::from_secs(5 * 60);
const ERROR_PAUSE: Duration = Duration::from_secs(3);
const NOTICE_PAUSE: Duration = Duration::from_millis(1500);
const MEDIA_FADE: Duration = Duration::from_millis(250);

pub fn toggle(app: &AppHandle) {
    let state = state::shared(app);
    match state.phase() {
        Phase::Idle => start(app, &state),
        Phase::Recording => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { finish(&app).await });
        }
        Phase::Finishing => {}
    }
}

pub fn cancel(app: &AppHandle) {
    let state = state::shared(app);
    if state.phase() != Phase::Recording {
        return;
    }
    let recorder = state.recorder.lock().unwrap().take();
    if let Some(recorder) = recorder {
        let path = recorder.path.clone();
        let duration = recorder.stop().unwrap_or(0.0);
        save_entry(&path, "", None, None, "cancelled", duration);
        let _ = app.emit("history-changed", ());
    }
    media::resume(&state.media);
    windows::hide_overlay(app);
    become_idle(app);
}

pub fn resolve_profile(default_engine: Engine) -> ActiveProfile {
    let settings = profiles::load();
    let front = frontmost::frontmost();
    let app_profile = front.as_ref().and_then(|front| settings.app(&front.id));
    settings.resolve(app_profile, front.as_ref().map(|front| front.name.as_str()), default_engine)
}

fn missing_setup(profile: &ActiveProfile, state: &Shared) -> Option<String> {
    state::engine_setup_hint(profile.engine, state).or_else(|| {
        (profile.rewrites() && !profile.rewriter.is_ready()).then(|| profile.rewriter.setup_hint())
    })
}

fn start(app: &AppHandle, state: &Shared) {
    let settings = state.settings();
    let profile = resolve_profile(settings.engine);

    if let Some(hint) = missing_setup(&profile, state) {
        *state.status.lock().unwrap() = hint;
        windows::open_settings(app);
        return;
    }
    if profile.engine.is_local() {
        preload_whisper(state);
    }

    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    *state.active_profile.lock().unwrap() = Some(profile);
    *state.phase.lock().unwrap() = Phase::Recording;
    *state.status.lock().unwrap() =
        audio::unavailable_preference_notice(settings.microphone.as_deref()).unwrap_or_default();
    tray::refresh(app);
    state::publish(app);
    windows::show_overlay(app);

    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || begin_capture(&app, &state, generation, &settings));
}

fn begin_capture(app: &AppHandle, state: &Shared, generation: u64, settings: &Settings) {
    if let Err(denied) = microphone::ensure_access() {
        if let Some(url) = microphone::PRIVACY_SETTINGS_URL {
            let _ = app.opener().open_url(url, None::<&str>);
        }
        fail_start(app, state, generation, denied);
        return;
    }
    if settings.pause_media && media::pause(&state.media) {
        std::thread::sleep(MEDIA_FADE);
    }
    let app_for_levels = app.clone();
    let recorder = Recorder::start(history::new_recording_path(), settings.microphone.clone(), move |level| {
        let _ = app_for_levels.emit_to(windows::OVERLAY, "level", level);
    });
    let mut slot = state.recorder.lock().unwrap();
    let still_wanted =
        state.phase() == Phase::Recording && state.generation.load(Ordering::SeqCst) == generation;
    match recorder {
        Ok(recorder) if still_wanted => *slot = Some(recorder),
        Ok(recorder) => {
            drop(slot);
            discard(recorder);
            media::resume(&state.media);
        }
        Err(error) => {
            drop(slot);
            media::resume(&state.media);
            fail_start(app, state, generation, format!("Could not start recording: {error}"));
        }
    }
}

fn fail_start(app: &AppHandle, state: &Shared, generation: u64, message: String) {
    let is_current = || state.phase() == Phase::Recording && state.generation.load(Ordering::SeqCst) == generation;
    if !is_current() {
        return;
    }
    state::set_status(app, message);
    std::thread::sleep(ERROR_PAUSE);
    if is_current() {
        windows::hide_overlay(app);
        become_idle(app);
    }
}

fn discard(recorder: Recorder) {
    let path = recorder.path.clone();
    let _ = recorder.stop();
    let _ = std::fs::remove_file(path);
}

fn preload_whisper(state: &Shared) {
    let state = state.clone();
    std::thread::spawn(move || {
        let model = state.whisper_model().path();
        let _ = state.whisper.load(&model);
    });
}

async fn finish(app: &AppHandle) {
    let state = state::shared(app);
    *state.phase.lock().unwrap() = Phase::Finishing;
    state::set_status(app, "Finalizing…");
    tray::refresh(app);

    let recorder = state.recorder.lock().unwrap().take();
    let profile = state.active_profile.lock().unwrap().clone();
    let (Some(recorder), Some(profile)) = (recorder, profile) else {
        media::resume(&state.media);
        windows::hide_overlay(app);
        become_idle(app);
        return;
    };
    let path = recorder.path.clone();
    let duration = tauri::async_runtime::spawn_blocking(move || recorder.stop())
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result)
        .unwrap_or(0.0);
    media::resume(&state.media);

    let engine = profile.engine;
    let label = engine.history_label(state.whisper_model().id);
    let status_app = app.clone();
    let transcription = transcribe_file(&state, engine, &path, duration, move |status| {
        state::set_status(&status_app, status)
    })
    .await;
    let (text, failure) = match transcription {
        Ok(text) => (text.trim().to_string(), None),
        Err(error) => (String::new(), Some(error)),
    };

    let mut rewritten = None;
    let mut rewrite_failed = false;
    if !text.is_empty() && profile.rewrites() {
        state::set_status(app, format!("Rewriting ({})…", profile.rewriter.display_name()));
        match rewrite::rewrite(&text, &profile).await {
            Ok(result) => rewritten = Some(result),
            Err(error) => {
                rewrite_failed = true;
                eprintln!("rewrite_error rewriter={} error={error}", profile.rewriter.display_name());
                state::set_status(app, format!("{error} Pasted the original text."));
            }
        }
    }

    let rewrite_style = rewritten.as_ref().map(|_| profile.style.id().to_string());
    save_entry(&path, &text, rewritten.clone(), rewrite_style, &label, duration);
    let _ = app.emit("history-changed", ());

    if text.is_empty() {
        match failure {
            Some(error) => {
                state::set_status(app, error);
                tokio::time::sleep(ERROR_PAUSE).await;
            }
            None => {
                state::set_status(app, "No speech detected. Audio saved to history.");
                tokio::time::sleep(NOTICE_PAUSE).await;
            }
        }
    } else {
        deliver(app, rewritten.unwrap_or(text)).await;
        if rewrite_failed {
            tokio::time::sleep(ERROR_PAUSE).await;
        }
    }

    windows::hide_overlay(app);
    become_idle(app);
    if engine.is_local() {
        schedule_whisper_unload(&state);
    }
}

pub async fn transcribe_file(
    state: &Shared,
    engine: Engine,
    path: &Path,
    duration: f64,
    on_status: impl Fn(String) + Send + Sync + 'static,
) -> Result<String, String> {
    let settings = state.settings();
    on_status(format!("Transcribing ({})…", engine.short_name()));
    if engine.is_local() {
        let state = state.clone();
        let wav: PathBuf = path.to_path_buf();
        let model = state.whisper_model().path();
        let language = settings.whisper_language.clone();
        let prompt = whisper::vocabulary_prompt(&dictionary::terms());
        return tauri::async_runtime::spawn_blocking(move || {
            state.whisper.transcribe(&wav, &model, &language, prompt.as_deref())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    let request = CloudRequest {
        engine,
        wav: path,
        recording_seconds: duration,
        dictionary: dictionary::terms(),
        azure_endpoint: settings.azure_endpoint.clone(),
        stepfun_language: settings.stepfun_language.clone(),
    };
    let attempts = Arc::new(AtomicU32::new(0));
    let counter = attempts.clone();
    cloud::transcribe(&request, move |attempt| {
        counter.store(attempt, Ordering::Relaxed);
        if attempt > 1 {
            on_status(format!("Transcribing ({})… retry {attempt}/3", engine.short_name()));
        }
    })
    .await
    .map_err(|error| match attempts.load(Ordering::Relaxed) {
        tries if tries > 1 => format!("{error} (gave up after {tries} tries)"),
        _ => error.to_string(),
    })
}

async fn deliver(app: &AppHandle, text: String) {
    if let Err(error) = paste::copy(&text) {
        state::set_status(app, error);
        tokio::time::sleep(ERROR_PAUSE).await;
        return;
    }
    let pasted = tauri::async_runtime::spawn_blocking(paste::paste_into_focused_app).await;
    if let Ok(Err(error)) = pasted {
        state::set_status(app, error);
        tokio::time::sleep(ERROR_PAUSE).await;
    }
}

fn save_entry(
    path: &Path,
    transcript: &str,
    rewritten: Option<String>,
    rewrite_style: Option<String>,
    engine: &str,
    duration: f64,
) {
    history::save_entry(&RecordingEntry {
        id: history::id_of(path),
        created_at: Utc::now(),
        duration,
        transcript: transcript.to_string(),
        rewritten,
        rewrite_style,
        engine: engine.to_string(),
    });
}

fn become_idle(app: &AppHandle) {
    let state = state::shared(app);
    *state.phase.lock().unwrap() = Phase::Idle;
    *state.active_profile.lock().unwrap() = None;
    state.status.lock().unwrap().clear();
    tray::refresh(app);
    state::publish(app);
}

pub fn schedule_whisper_unload(state: &Shared) {
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(WHISPER_IDLE_UNLOAD).await;
        if state.phase() == Phase::Idle {
            state.whisper.unload_if_idle_for(WHISPER_IDLE_UNLOAD);
        }
    });
}
