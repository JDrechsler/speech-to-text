use std::fs;

use serde::{Deserialize, Serialize};

use crate::engines::Engine;
use crate::paths;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub engine: Engine,
    pub last_cloud_engine: Option<Engine>,
    pub whisper_model: Option<String>,
    pub whisper_language: String,
    pub stepfun_language: String,
    pub azure_endpoint: Option<String>,
    pub microphone: Option<String>,
    pub pause_media: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            engine: Engine::Whisper,
            last_cloud_engine: None,
            whisper_model: None,
            whisper_language: "auto".into(),
            stepfun_language: "en".into(),
            azure_endpoint: None,
            microphone: None,
            pause_media: true,
        }
    }
}

impl Settings {
    pub fn preferred_cloud(&self) -> Engine {
        self.last_cloud_engine
            .filter(|engine| !engine.is_local())
            .unwrap_or(Engine::Scribe)
    }
}

pub fn load() -> Settings {
    if let Ok(data) = fs::read(paths::settings_file()) {
        if let Ok(settings) = serde_json::from_slice(&data) {
            return settings;
        }
    }
    let settings = imported_from_mac_app().unwrap_or_default();
    save(&settings);
    settings
}

pub fn save(settings: &Settings) {
    if let Ok(data) = serde_json::to_vec_pretty(settings) {
        let _ = fs::create_dir_all(paths::support_dir());
        let _ = fs::write(paths::settings_file(), data);
    }
}

#[cfg(target_os = "macos")]
fn imported_from_mac_app() -> Option<Settings> {
    let path = dirs::home_dir()?.join("Library/Preferences/com.jdrechsler.SpeechToText.plist");
    let plist = plist::Value::from_file(path).ok()?;
    let values = plist.as_dictionary()?;
    let text = |key: &str| values.get(key).and_then(|value| value.as_string()).map(String::from);
    let engine = |key: &str| {
        text(key).and_then(|raw| serde_json::from_value::<Engine>(raw.into()).ok())
    };

    let defaults = Settings::default();
    Some(Settings {
        engine: engine("enginePreference").unwrap_or(defaults.engine),
        last_cloud_engine: engine("lastCloudEngine"),
        whisper_model: text("whisperModel"),
        whisper_language: text("whisperLanguage").unwrap_or(defaults.whisper_language),
        stepfun_language: text("stepFunLanguage").unwrap_or(defaults.stepfun_language),
        azure_endpoint: text("AZURE_SPEECH_ENDPOINT"),
        microphone: None,
        pause_media: values
            .get("pauseMediaEnabled")
            .and_then(|value| value.as_boolean())
            .unwrap_or(defaults.pause_media),
    })
}

#[cfg(not(target_os = "macos"))]
fn imported_from_mac_app() -> Option<Settings> {
    None
}
