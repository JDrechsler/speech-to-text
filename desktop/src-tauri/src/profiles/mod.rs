use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engines::Engine;
use crate::paths;
use crate::rewrite::{ModelPreset, RewriteModel, RewriteTarget, model_key_id};
use crate::secrets;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RewriteStyle {
    #[default]
    Off,
    Cleanup,
    Email,
    Chat,
    Prompt,
    Custom,
}

impl RewriteStyle {
    pub const ALL: [RewriteStyle; 6] = [
        RewriteStyle::Off,
        RewriteStyle::Cleanup,
        RewriteStyle::Email,
        RewriteStyle::Chat,
        RewriteStyle::Prompt,
        RewriteStyle::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            RewriteStyle::Off => "off",
            RewriteStyle::Cleanup => "cleanup",
            RewriteStyle::Email => "email",
            RewriteStyle::Chat => "chat",
            RewriteStyle::Prompt => "prompt",
            RewriteStyle::Custom => "custom",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            RewriteStyle::Off => "Off",
            RewriteStyle::Cleanup => "Clean up",
            RewriteStyle::Email => "Email",
            RewriteStyle::Chat => "Chat message",
            RewriteStyle::Prompt => "Prompt for AI",
            RewriteStyle::Custom => "Custom",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            RewriteStyle::Off => "Pastes exactly what was heard.",
            RewriteStyle::Cleanup => "Fixes punctuation, drops filler words and false starts, keeps your wording.",
            RewriteStyle::Email => "Turns your words into a clear, friendly email.",
            RewriteStyle::Chat => "Short and casual, like a message to a colleague.",
            RewriteStyle::Prompt => "Turns rambling into a clear prompt for Claude, ChatGPT or a coding agent.",
            RewriteStyle::Custom => "Follows your own instructions, for example \"Translate to English\".",
        }
    }

    pub fn instruction(self) -> Option<&'static str> {
        match self {
            RewriteStyle::Off | RewriteStyle::Custom => None,
            RewriteStyle::Cleanup => Some(
                "Fix punctuation and capitalization and obvious speech recognition mistakes. Remove only filler words (um, uh, you know), stutters and repeated words. Keep everything else the speaker said, including phrases like \"I think\" or \"I was thinking\", in their wording and tone.",
            ),
            RewriteStyle::Email => Some(
                "Rewrite it as a clear, friendly email body in the speaker's voice. Keep every point they made. Use paragraphs. Only include a greeting or sign-off if they said one. No subject line.",
            ),
            RewriteStyle::Chat => Some(
                "Rewrite it as a short, casual chat message in the speaker's voice. Keep every point, cut the padding. No greeting or sign-off unless they said one.",
            ),
            RewriteStyle::Prompt => Some(
                "Rewrite it as a clear prompt for an AI assistant, written as the speaker's own request. Keep every requirement, constraint, name and detail they mentioned. Remove rambling and repetition. Use a short bulleted list when there are several separate points.",
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppProfile {
    #[serde(rename = "bundleID")]
    pub bundle_id: String,
    pub app_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<Engine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<RewriteStyle>,
    #[serde(rename = "rewriterID", skip_serializing_if = "Option::is_none")]
    pub rewriter_id: Option<String>,
    pub custom_instructions: String,
}

impl AppProfile {
    pub fn new(bundle_id: &str, app_name: &str) -> AppProfile {
        AppProfile {
            bundle_id: bundle_id.into(),
            app_name: app_name.into(),
            ..AppProfile::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileSettings {
    pub default_style: RewriteStyle,
    #[serde(rename = "defaultRewriterID")]
    pub default_rewriter_id: String,
    pub default_custom_instructions: String,
    pub apps: Vec<AppProfile>,
    pub models: Vec<RewriteModel>,
}

impl Default for ProfileSettings {
    fn default() -> Self {
        ProfileSettings {
            default_style: RewriteStyle::Off,
            default_rewriter_id: RewriteTarget::APPLE_ID.into(),
            default_custom_instructions: String::new(),
            apps: Vec::new(),
            models: Vec::new(),
        }
    }
}

impl ProfileSettings {
    pub fn rewrite_targets(&self) -> Vec<RewriteTarget> {
        let models = self.models.iter().cloned().map(RewriteTarget::Model);
        std::iter::once(RewriteTarget::Apple).chain(models).collect()
    }

    pub fn default_rewriter(&self) -> RewriteTarget {
        self.target(Some(&self.default_rewriter_id))
            .unwrap_or(RewriteTarget::Apple)
    }

    pub fn target(&self, id: Option<&str>) -> Option<RewriteTarget> {
        let id = id?;
        if id == RewriteTarget::APPLE_ID {
            return Some(RewriteTarget::Apple);
        }
        let model = self.models.iter().find(|model| model.id == id)?;
        Some(RewriteTarget::Model(model.clone()))
    }

    pub fn app(&self, bundle_id: &str) -> Option<&AppProfile> {
        self.apps.iter().find(|app| app.bundle_id == bundle_id)
    }

    pub fn resolve(
        &self,
        profile: Option<&AppProfile>,
        app_name: Option<&str>,
        default_engine: Engine,
    ) -> ActiveProfile {
        let own_style = profile.and_then(|profile| profile.style);
        let custom_instructions = match profile {
            Some(profile) if own_style == Some(RewriteStyle::Custom) => {
                profile.custom_instructions.clone()
            }
            _ => self.default_custom_instructions.clone(),
        };
        let rewriter_id = profile.and_then(|profile| profile.rewriter_id.as_deref());
        ActiveProfile {
            app_name: profile
                .map(|profile| profile.app_name.clone())
                .or_else(|| app_name.map(String::from)),
            has_app_profile: profile.is_some(),
            engine: profile
                .and_then(|profile| profile.engine)
                .unwrap_or(default_engine),
            style: own_style.unwrap_or(self.default_style),
            rewriter: self
                .target(rewriter_id)
                .unwrap_or_else(|| self.default_rewriter()),
            custom_instructions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveProfile {
    pub app_name: Option<String>,
    pub has_app_profile: bool,
    pub engine: Engine,
    pub style: RewriteStyle,
    pub rewriter: RewriteTarget,
    pub custom_instructions: String,
}

impl ActiveProfile {
    pub fn rewrites(&self) -> bool {
        match self.style {
            RewriteStyle::Off => false,
            RewriteStyle::Custom => !self.custom_instructions.trim().is_empty(),
            _ => true,
        }
    }

    pub fn context_line(&self) -> Option<String> {
        if !self.has_app_profile && !self.rewrites() {
            return None;
        }
        let style_text = if self.rewrites() {
            self.style.title()
        } else {
            "No rewrite"
        };
        let app_name = self.app_name.as_deref().filter(|_| self.has_app_profile);
        let parts: Vec<&str> = app_name.into_iter().chain([style_text]).collect();
        Some(parts.join(" · "))
    }
}

pub fn load() -> ProfileSettings {
    load_from(&paths::profiles_file())
}

pub fn save(settings: &ProfileSettings) -> Result<(), String> {
    save_to(&paths::profiles_file(), settings)
}

pub fn add_app(settings: &mut ProfileSettings, bundle_id: &str, app_name: &str) {
    if settings.app(bundle_id).is_some() {
        return;
    }
    settings.apps.push(AppProfile::new(bundle_id, app_name));
    settings
        .apps
        .sort_by_cached_key(|app| app.app_name.to_lowercase());
}

pub fn remove_app(settings: &mut ProfileSettings, bundle_id: &str) {
    settings.apps.retain(|app| app.bundle_id != bundle_id);
}

pub fn add_model_from_preset(settings: &mut ProfileSettings, preset: &ModelPreset) -> RewriteModel {
    let model = preset.new_model();
    settings.models.push(model.clone());
    if settings.models.len() == 1 {
        settings.default_rewriter_id = model.id.clone();
    }
    model
}

pub fn remove_model(settings: &mut ProfileSettings, model_id: &str) -> Result<(), String> {
    forget_model(settings, model_id);
    secrets::delete(&model_key_id(model_id))
}

fn forget_model(settings: &mut ProfileSettings, model_id: &str) {
    settings.models.retain(|model| model.id != model_id);
    if settings.default_rewriter_id == model_id {
        settings.default_rewriter_id = settings
            .models
            .first()
            .map_or_else(|| RewriteTarget::APPLE_ID.into(), |model| model.id.clone());
    }
    for app in &mut settings.apps {
        if app.rewriter_id.as_deref() == Some(model_id) {
            app.rewriter_id = None;
        }
    }
}

fn load_from(path: &Path) -> ProfileSettings {
    fs::read(path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_default()
}

fn save_to(path: &Path, settings: &ProfileSettings) -> Result<(), String> {
    let data = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}
