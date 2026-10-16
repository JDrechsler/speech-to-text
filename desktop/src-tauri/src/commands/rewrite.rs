use serde::Serialize;
use tauri::AppHandle;

use crate::frontmost::{self, AppIdentity};
use crate::profiles::{self, ProfileSettings, RewriteStyle};
use crate::rewrite::{self, ModelApi, ModelPreset, RewriteModel};
use crate::secrets;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleInfo {
    id: RewriteStyle,
    title: &'static str,
    summary: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiInfo {
    id: ModelApi,
    title: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewriteCatalog {
    styles: Vec<StyleInfo>,
    apis: Vec<ApiInfo>,
    cloud_presets: [ModelPreset; 6],
    local_presets: [ModelPreset; 2],
    other_preset: ModelPreset,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    id: String,
    display_name: String,
    is_local: bool,
    destination: String,
    badge: String,
    menu_title: String,
    privacy_statement: String,
    is_ready: bool,
    setup_hint: Option<String>,
    has_key: bool,
    key_placeholder: String,
}

impl TargetInfo {
    fn of(model: &RewriteModel) -> TargetInfo {
        let is_ready = model.is_configured();
        TargetInfo {
            id: model.id.clone(),
            display_name: model.name.clone(),
            is_local: model.is_local(),
            destination: model.destination(),
            badge: model.badge(),
            menu_title: model.menu_title(),
            privacy_statement: model.privacy_statement(),
            is_ready,
            setup_hint: (!is_ready).then(|| model.setup_hint()),
            has_key: secrets::contains(&model.key_id()),
            key_placeholder: model.key_placeholder(),
        }
    }
}

#[tauri::command]
pub fn rewrite_catalog() -> RewriteCatalog {
    RewriteCatalog {
        styles: RewriteStyle::ALL
            .into_iter()
            .map(|style| StyleInfo { id: style, title: style.title(), summary: style.summary() })
            .collect(),
        apis: ModelApi::ALL.into_iter().map(|api| ApiInfo { id: api, title: api.title() }).collect(),
        cloud_presets: ModelPreset::CLOUD,
        local_presets: ModelPreset::LOCAL,
        other_preset: ModelPreset::OTHER,
    }
}

#[tauri::command]
pub fn profile_settings() -> ProfileSettings {
    profiles::load()
}

#[tauri::command]
pub fn save_profile_settings(settings: ProfileSettings) -> Result<ProfileSettings, String> {
    profiles::save(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn rewrite_targets() -> Vec<TargetInfo> {
    profiles::load().models.iter().map(TargetInfo::of).collect()
}

#[tauri::command]
pub fn add_rewrite_model(preset: String) -> Result<ProfileSettings, String> {
    let preset = ModelPreset::named(&preset).ok_or("Unknown model preset")?;
    let mut settings = profiles::load();
    profiles::add_model_from_preset(&mut settings, &preset);
    profiles::save(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn remove_rewrite_model(id: String) -> Result<ProfileSettings, String> {
    let mut settings = profiles::load();
    profiles::remove_model(&mut settings, &id)?;
    profiles::save(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub async fn list_model_names(model: RewriteModel) -> Result<Vec<String>, String> {
    rewrite::available_models(&model).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub fn running_apps(app: AppHandle) -> Vec<AppIdentity> {
    frontmost::running_apps(&app.config().identifier)
}

#[tauri::command]
pub fn app_identity_of_file(path: String) -> Result<AppIdentity, String> {
    frontmost::identity_of_app_file(std::path::Path::new(&path))
        .ok_or_else(|| "That is not an app this computer can identify.".into())
}

#[tauri::command]
pub fn add_app_profile(bundle_id: String, app_name: String) -> Result<ProfileSettings, String> {
    let mut settings = profiles::load();
    profiles::add_app(&mut settings, &bundle_id, &app_name);
    profiles::save(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn remove_app_profile(bundle_id: String) -> Result<ProfileSettings, String> {
    let mut settings = profiles::load();
    profiles::remove_app(&mut settings, &bundle_id);
    profiles::save(&settings)?;
    Ok(settings)
}
