use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::Recorder;
use crate::engines::{self, Engine};
use crate::media::Media;
use crate::models::{self, Downloads};
use crate::nemotron::{self, NemotronServer};
use crate::profiles::ActiveProfile;
use crate::secrets;
use crate::settings::{self, Settings};
use crate::whisper::LocalWhisper;
use crate::windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Recording,
    Finishing,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub is_local: bool,
    pub text: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    pub phase: Phase,
    pub status: String,
    pub context_line: Option<String>,
    pub engine_badge: Badge,
    pub rewrite_badge: Option<Badge>,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub whisper: LocalWhisper,
    pub nemotron: NemotronServer,
    pub downloads: Downloads,
    pub media: Media,
    pub generation: AtomicU64,
    pub phase: Mutex<Phase>,
    pub status: Mutex<String>,
    pub recorder: Mutex<Option<Recorder>>,
    pub active_profile: Mutex<Option<ActiveProfile>>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(media: Media) -> Shared {
        Arc::new(AppState {
            settings: Mutex::new(settings::load()),
            whisper: LocalWhisper::default(),
            nemotron: NemotronServer::default(),
            downloads: Downloads::default(),
            media,
            generation: AtomicU64::new(0),
            phase: Mutex::new(Phase::Idle),
            status: Mutex::new(String::new()),
            recorder: Mutex::new(None),
            active_profile: Mutex::new(None),
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) -> Settings {
        let mut current = self.settings.lock().unwrap();
        change(&mut current);
        settings::save(&current);
        current.clone()
    }

    pub fn phase(&self) -> Phase {
        *self.phase.lock().unwrap()
    }

    pub fn unload_local_engines_except(&self, engine: Engine) {
        if engine != Engine::Whisper {
            self.whisper.unload();
        }
        if engine != Engine::Nemotron {
            self.nemotron.stop();
        }
    }

    pub fn whisper_model(&self) -> &'static models::WhisperModel {
        models::selected(self.settings().whisper_model.as_deref())
    }

    pub fn nemotron_model(&self) -> &'static nemotron::NemotronModel {
        nemotron::selected(self.settings().nemotron_model.as_deref())
    }

    pub fn history_label(&self, engine: Engine) -> String {
        match engine {
            Engine::Nemotron => engine.history_label(self.nemotron_model().id),
            _ => engine.history_label(self.whisper_model().id),
        }
    }

    pub fn overlay_state(&self) -> OverlayState {
        let profile = self.active_profile.lock().unwrap().clone();
        let engine = profile.as_ref().map(|profile| profile.engine).unwrap_or(self.settings().engine);
        OverlayState {
            phase: self.phase(),
            status: self.status.lock().unwrap().clone(),
            context_line: profile.as_ref().and_then(ActiveProfile::context_line),
            engine_badge: self.engine_badge(engine),
            rewrite_badge: profile
                .filter(ActiveProfile::rewrites)
                .and_then(|profile| profile.rewriter)
                .map(|model| Badge { is_local: model.is_local(), text: model.badge() }),
        }
    }

    fn engine_badge(&self, engine: Engine) -> Badge {
        let text = match engine {
            Engine::Whisper => format!("On {} · Whisper {}", engines::THIS_DEVICE, self.whisper_model().name),
            Engine::Nemotron => format!("On {} · Nemotron {}", engines::THIS_DEVICE, self.nemotron_model().name),
            _ => format!("Cloud · {}", engine.provider()),
        };
        Badge { is_local: engine.is_local(), text }
    }
}

pub fn choose_engine(state: &Shared, engine: Engine) -> Settings {
    let settings = state.update_settings(|settings| {
        settings.engine = engine;
        if engine.is_local() {
            settings.last_local_engine = Some(engine);
        } else {
            settings.last_cloud_engine = Some(engine);
        }
    });
    let state = state.clone();
    std::thread::spawn(move || state.unload_local_engines_except(engine));
    settings
}

pub fn shared(app: &AppHandle) -> Shared {
    app.state::<Shared>().inner().clone()
}

pub fn publish(app: &AppHandle) {
    let state = shared(app).overlay_state();
    let _ = app.emit_to(windows::OVERLAY, "overlay-state", state);
}

pub fn set_status(app: &AppHandle, status: impl Into<String>) {
    *shared(app).status.lock().unwrap() = status.into();
    publish(app);
}

pub fn credential_value(field: &engines::CredentialField, settings: &Settings) -> Option<String> {
    if field.is_secret {
        return secrets::read(field.id);
    }
    match field.id {
        "AZURE_SPEECH_ENDPOINT" => settings.azure_endpoint.clone().filter(|value| !value.trim().is_empty()),
        _ => None,
    }
}

pub fn engine_setup_hint(engine: Engine, state: &AppState) -> Option<String> {
    if engine == Engine::Whisper {
        let model = state.whisper_model();
        return (!model.is_installed())
            .then(|| "No Whisper model downloaded yet. Open Settings and download one.".to_string());
    }
    if engine == Engine::Nemotron {
        return (!state.nemotron_model().is_installed())
            .then(|| "Nemotron 3.5 Transcribe is not downloaded yet. Open Settings and download it.".to_string());
    }
    let settings = state.settings();
    let missing = engine
        .credential_fields()
        .iter()
        .any(|field| credential_value(field, &settings).is_none());
    missing.then(|| format!("{} needs your {} API key. Add it in Settings.", engine.display_name(), engine.provider()))
}
