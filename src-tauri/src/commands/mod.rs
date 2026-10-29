mod history;
mod rewrite;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

use crate::audio::{self, InputDevice};
use crate::dictionary;
use crate::engines::{self, Engine, EngineInfo};
use crate::models::{self, WhisperModel};
use crate::nemotron::{self, NemotronModel};
use crate::login_item;
use crate::paste;
use crate::paths;
use crate::secrets;
use crate::settings::Settings;
use crate::state::{self, OverlayState};
use crate::windows;

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        overlay_state,
        get_settings,
        engine_catalog,
        engine_status,
        set_engine,
        use_cloud,
        set_whisper_language,
        set_stepfun_language,
        set_microphone,
        set_pause_media,
        whisper_models,
        whisper_languages,
        select_whisper_model,
        download_whisper_model,
        cancel_whisper_download,
        delete_whisper_model,
        nemotron_models,
        select_nemotron_model,
        download_nemotron,
        cancel_nemotron_download,
        delete_nemotron,
        saved_credentials,
        save_credential,
        remove_credential,
        input_devices,
        general_status,
        set_launch_at_login,
        request_accessibility,
        open_dictionary,
        open_recordings,
        open_url,
        history::history_entries,
        history::delete_history_entry,
        history::recording_audio,
        history::reveal_recording,
        history::retranscribe,
        copy_text,
        open_history_window,
        rewrite::rewrite_catalog,
        rewrite::profile_settings,
        rewrite::save_profile_settings,
        rewrite::rewrite_targets,
        rewrite::add_rewrite_model,
        rewrite::remove_rewrite_model,
        rewrite::list_model_names,
        rewrite::running_apps,
        rewrite::app_identity_of_file,
        rewrite::add_app_profile,
        rewrite::remove_app_profile,
    ]
}

#[tauri::command]
fn overlay_state(app: AppHandle) -> OverlayState {
    state::shared(&app).overlay_state()
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    state::shared(&app).settings()
}

#[tauri::command]
fn engine_catalog() -> Vec<EngineInfo> {
    Engine::available().map(Engine::info).collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStatus {
    id: Engine,
    setup_hint: Option<String>,
}

#[tauri::command]
fn engine_status(app: AppHandle) -> Vec<EngineStatus> {
    let state = state::shared(&app);
    Engine::available()
        .map(|id| EngineStatus { id, setup_hint: state::engine_setup_hint(id, &state) })
        .collect()
}

#[tauri::command]
fn set_engine(app: AppHandle, engine: Engine) -> Settings {
    state::choose_engine(&state::shared(&app), engine)
}

#[tauri::command]
fn use_cloud(app: AppHandle, cloud: bool) -> Settings {
    let settings = state::shared(&app).settings();
    let engine = if cloud { settings.preferred_cloud() } else { settings.preferred_local() };
    set_engine(app, engine)
}

#[tauri::command]
fn set_whisper_language(app: AppHandle, language: String) -> Settings {
    state::shared(&app).update_settings(|settings| settings.whisper_language = language)
}

#[tauri::command]
fn set_stepfun_language(app: AppHandle, language: String) -> Settings {
    state::shared(&app).update_settings(|settings| settings.stepfun_language = language)
}

#[tauri::command]
fn set_microphone(app: AppHandle, microphone: Option<String>) -> Settings {
    state::shared(&app).update_settings(|settings| settings.microphone = microphone)
}

#[tauri::command]
fn set_pause_media(app: AppHandle, enabled: bool) -> Settings {
    state::shared(&app).update_settings(|settings| settings.pause_media = enabled)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelStatus {
    #[serde(flatten)]
    model: WhisperModel,
    is_installed: bool,
    is_selected: bool,
    is_downloading: bool,
}

#[tauri::command]
fn whisper_models(app: AppHandle) -> Vec<ModelStatus> {
    let state = state::shared(&app);
    let selected = state.whisper_model().id;
    models::CATALOG
        .iter()
        .map(|model| ModelStatus {
            model: *model,
            is_installed: model.is_installed(),
            is_selected: model.id == selected,
            is_downloading: state.downloads.is_running(model.id),
        })
        .collect()
}

#[derive(Serialize)]
struct Language {
    code: &'static str,
    name: &'static str,
}

#[tauri::command]
fn whisper_languages() -> Vec<Language> {
    models::LANGUAGES.iter().map(|&(code, name)| Language { code, name }).collect()
}

#[tauri::command]
fn select_whisper_model(app: AppHandle, id: String) -> Result<Settings, String> {
    models::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    state.whisper.unload();
    state.update_settings(|settings| settings.whisper_model = Some(id));
    Ok(state::choose_engine(&state, Engine::Whisper))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: &'static str,
    fraction: f64,
    done: bool,
    error: Option<String>,
}

#[tauri::command]
async fn download_whisper_model(app: AppHandle, id: String) -> Result<(), String> {
    let model = models::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    let emitter = app.clone();
    let mut last_percent = -1i64;
    let result = state
        .downloads
        .download(model, move |fraction| {
            let percent = (fraction * 100.0) as i64;
            if percent != last_percent {
                last_percent = percent;
                let _ = emitter.emit(
                    "model-download",
                    DownloadProgress { id: model.id, fraction, done: false, error: None },
                );
            }
        })
        .await;
    let _ = app.emit(
        "model-download",
        DownloadProgress { id: model.id, fraction: 1.0, done: true, error: result.clone().err() },
    );
    result
}

#[tauri::command]
fn cancel_whisper_download(app: AppHandle, id: String) {
    if let Some(model) = models::find(&id) {
        state::shared(&app).downloads.cancel(model.id);
    }
}

#[tauri::command]
fn delete_whisper_model(app: AppHandle, id: String) -> Result<(), String> {
    let model = models::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    if state.whisper_model().id == model.id {
        state.whisper.unload();
    }
    models::delete(model)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NemotronModelStatus {
    #[serde(flatten)]
    model: NemotronModel,
    is_installed: bool,
    is_selected: bool,
    is_downloading: bool,
}

#[tauri::command]
fn nemotron_models(app: AppHandle) -> Vec<NemotronModelStatus> {
    let state = state::shared(&app);
    let selected = state.nemotron_model().id;
    nemotron::CATALOG
        .iter()
        .map(|model| NemotronModelStatus {
            model: *model,
            is_installed: model.is_installed(),
            is_selected: model.id == selected,
            is_downloading: state.downloads.is_running(model.id),
        })
        .collect()
}

#[tauri::command]
fn select_nemotron_model(app: AppHandle, id: String) -> Result<Settings, String> {
    nemotron::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    state.nemotron.stop();
    state.update_settings(|settings| settings.nemotron_model = Some(id));
    Ok(state::choose_engine(&state, Engine::Nemotron))
}

#[tauri::command]
async fn download_nemotron(app: AppHandle, id: String) -> Result<(), String> {
    let model = nemotron::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    if nemotron::CATALOG.iter().any(|other| state.downloads.is_running(other.id)) {
        return Err("Another Nemotron download is still running.".into());
    }
    let emitter = app.clone();
    let mut last_permille = -1i64;
    let result = state
        .downloads
        .run(model.id, |cancelled| async move {
            nemotron::download(model, &cancelled, |fraction| {
                let permille = (fraction * 1000.0) as i64;
                if permille != last_permille {
                    last_permille = permille;
                    let progress = DownloadProgress { id: model.id, fraction, done: false, error: None };
                    let _ = emitter.emit("model-download", progress);
                }
            })
            .await
        })
        .await;
    let _ = app.emit(
        "model-download",
        DownloadProgress { id: model.id, fraction: 1.0, done: true, error: result.clone().err() },
    );
    result
}

#[tauri::command]
fn cancel_nemotron_download(app: AppHandle, id: String) {
    if let Some(model) = nemotron::find(&id) {
        state::shared(&app).downloads.cancel(model.id);
    }
}

#[tauri::command]
fn delete_nemotron(app: AppHandle, id: String) -> Result<(), String> {
    let model = nemotron::find(&id).ok_or("Unknown model")?;
    let state = state::shared(&app);
    state.nemotron.stop();
    nemotron::delete(model)
}

#[tauri::command]
fn saved_credentials(app: AppHandle) -> Vec<String> {
    let settings = state::shared(&app).settings();
    engines::all_credential_fields()
        .filter(|field| state::credential_value(field, &settings).is_some())
        .map(|field| field.id.to_string())
        .collect()
}

#[tauri::command]
fn save_credential(app: AppHandle, id: String, value: String) -> Result<(), String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Nothing to save".into());
    }
    if id == engines::AZURE_SPEECH_ENDPOINT.id {
        state::shared(&app).update_settings(|settings| settings.azure_endpoint = Some(value));
        return Ok(());
    }
    secrets::save(&id, &value)
}

#[tauri::command]
fn remove_credential(app: AppHandle, id: String) -> Result<(), String> {
    if id == engines::AZURE_SPEECH_ENDPOINT.id {
        state::shared(&app).update_settings(|settings| settings.azure_endpoint = None);
        return Ok(());
    }
    secrets::delete(&id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Microphones {
    devices: Vec<InputDevice>,
    system_default: Option<String>,
}

#[tauri::command]
fn input_devices() -> Microphones {
    Microphones { devices: audio::input_devices(), system_default: audio::default_device_name() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeneralStatus {
    version: String,
    platform: &'static str,
    dictionary_terms: usize,
    accessibility_allowed: bool,
    launch_at_login: bool,
}

#[tauri::command]
fn general_status(app: AppHandle) -> GeneralStatus {
    GeneralStatus {
        version: app.package_info().version.to_string(),
        platform: std::env::consts::OS,
        dictionary_terms: dictionary::terms().len(),
        accessibility_allowed: paste::accessibility::is_trusted(false),
        launch_at_login: login_item::is_enabled(&app),
    }
}

#[tauri::command]
fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    login_item::set_enabled(&app, enabled)
}

#[tauri::command]
fn request_accessibility() -> bool {
    paste::accessibility::is_trusted(true)
}

#[tauri::command]
fn open_dictionary(app: AppHandle) -> Result<(), String> {
    dictionary::ensure_file_exists();
    app.opener()
        .open_path(paths::dictionary_file().to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_recordings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_path(paths::recordings_dir().to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    app.opener().open_url(url, None::<&str>).map_err(|error| error.to_string())
}

#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    paste::copy(&text)
}

#[tauri::command]
fn open_history_window(app: AppHandle) {
    windows::open_history(&app);
}
