use std::fs;
use std::path::PathBuf;

const APP_FOLDER: &str = "SpeechToText";

pub fn support_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_FOLDER)
}

fn ensured(path: PathBuf) -> PathBuf {
    let _ = fs::create_dir_all(&path);
    path
}

pub fn models_dir() -> PathBuf {
    ensured(support_dir().join("models"))
}

pub fn recordings_dir() -> PathBuf {
    ensured(support_dir().join("recordings"))
}

pub fn dictionary_file() -> PathBuf {
    support_dir().join("dictionary.txt")
}

pub fn profiles_file() -> PathBuf {
    support_dir().join("profiles.json")
}

pub fn keys_file() -> PathBuf {
    support_dir().join("api-keys.enc")
}

pub fn settings_file() -> PathBuf {
    support_dir().join("settings.json")
}

