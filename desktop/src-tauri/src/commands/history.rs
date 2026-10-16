use tauri::AppHandle;
use tauri::ipc::Response;
use tauri_plugin_opener::OpenerExt;

use crate::engines::Engine;
use crate::history::{self, RecordingEntry};
use crate::paste;
use crate::session;
use crate::state;

#[tauri::command]
pub fn history_entries() -> Vec<RecordingEntry> {
    history::entries()
}

#[tauri::command]
pub fn delete_history_entry(id: String) {
    history::delete(&id);
}

#[tauri::command]
pub fn recording_audio(id: String) -> Result<Response, String> {
    std::fs::read(history::audio_path(&id))
        .map(Response::new)
        .map_err(|_| "This recording is no longer on disk.".to_string())
}

#[tauri::command]
pub fn reveal_recording(app: AppHandle, id: String) -> Result<(), String> {
    app.opener()
        .reveal_item_in_dir(history::audio_path(&id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn retranscribe(app: AppHandle, id: String, engine: Engine) -> Result<RecordingEntry, String> {
    let state = state::shared(&app);
    let entry = history::entries()
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or("This recording is no longer in the history.")?;
    if let Some(hint) = state::engine_setup_hint(engine, &state) {
        return Err(hint);
    }
    let text = session::transcribe_file(&state, engine, &history::audio_path(&id), entry.duration, |_| {})
        .await?;
    let text = text.trim().to_string();
    let label = engine.history_label(state.whisper_model().id);
    let updated = history::update_transcript(&id, &text, &label).ok_or("Could not update the history.")?;
    if !text.is_empty() {
        paste::copy(&text)?;
    }
    if engine.is_local() {
        session::schedule_whisper_unload(&state);
    }
    Ok(updated)
}
