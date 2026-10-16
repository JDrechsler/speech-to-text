use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

use crate::paths;

const RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    pub id: String,
    #[serde(with = "iso8601")]
    pub created_at: DateTime<Utc>,
    pub duration: f64,
    pub transcript: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewritten: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite_style: Option<String>,
    pub engine: String,
}

mod iso8601 {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&date.format("%Y-%m-%dT%H:%M:%SZ").to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DateTime<Utc>, D::Error> {
        let raw = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&raw)
            .map(|date| date.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

pub fn audio_path(id: &str) -> PathBuf {
    paths::recordings_dir().join(format!("{id}.wav"))
}

fn sidecar_path(id: &str) -> PathBuf {
    paths::recordings_dir().join(format!("{id}.json"))
}

pub fn new_recording_path() -> PathBuf {
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let mut path = audio_path(&stamp);
    let mut suffix = 2;
    while path.exists() {
        path = audio_path(&format!("{stamp}_{suffix}"));
        suffix += 1;
    }
    path
}

pub fn id_of(audio: &Path) -> String {
    audio
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn save_entry(entry: &RecordingEntry) {
    if let Ok(data) = serde_json::to_vec_pretty(entry) {
        let path = sidecar_path(&entry.id);
        let temporary = path.with_extension("json.tmp");
        if fs::write(&temporary, data).is_ok() {
            let _ = fs::rename(temporary, path);
        }
    }
}

pub fn update_transcript(id: &str, transcript: &str, engine: &str) -> Option<RecordingEntry> {
    let mut entry = entries().into_iter().find(|entry| entry.id == id)?;
    entry.transcript = transcript.to_string();
    entry.rewritten = None;
    entry.rewrite_style = None;
    entry.engine = engine.to_string();
    save_entry(&entry);
    Some(entry)
}

pub fn delete(id: &str) {
    let _ = fs::remove_file(audio_path(id));
    let _ = fs::remove_file(sidecar_path(id));
}

pub fn entries() -> Vec<RecordingEntry> {
    let Ok(files) = fs::read_dir(paths::recordings_dir()) else {
        return Vec::new();
    };
    let mut entries: Vec<RecordingEntry> = files
        .flatten()
        .map(|file| file.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
        .map(|audio| {
            let id = id_of(&audio);
            fs::read(sidecar_path(&id))
                .ok()
                .and_then(|data| serde_json::from_slice(&data).ok())
                .unwrap_or_else(|| RecordingEntry {
                    created_at: created(&audio).map(DateTime::<Utc>::from).unwrap_or_else(Utc::now),
                    id,
                    duration: 0.0,
                    transcript: String::new(),
                    rewritten: None,
                    rewrite_style: None,
                    engine: "none".into(),
                })
        })
        .collect();
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    entries
}

pub fn cleanup_expired() {
    let Ok(files) = fs::read_dir(paths::recordings_dir()) else {
        return;
    };
    let now = SystemTime::now();
    for audio in files.flatten().map(|file| file.path()) {
        if audio.extension().is_none_or(|extension| extension != "wav") {
            continue;
        }
        let expired = created(&audio)
            .and_then(|time| now.duration_since(time).ok())
            .is_some_and(|age| age > RETENTION);
        if expired {
            delete(&id_of(&audio));
        }
    }
}

fn created(path: &Path) -> Option<SystemTime> {
    let metadata = fs::metadata(path).ok()?;
    metadata.created().or_else(|_| metadata.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_matches_the_mac_app_format() {
        let swift_json = r#"{
          "createdAt" : "2026-10-16T09:12:03Z",
          "duration" : 4.2,
          "engine" : "scribe-v2",
          "id" : "2026-10-16_11-12-03",
          "rewriteStyle" : "email",
          "rewritten" : "Hi Anna,",
          "transcript" : "hi anna"
        }"#;
        let entry: RecordingEntry = serde_json::from_str(swift_json).unwrap();
        assert_eq!(entry.rewritten.as_deref(), Some("Hi Anna,"));
        let written = serde_json::to_string(&entry).unwrap();
        assert!(written.contains(r#""createdAt":"2026-10-16T09:12:03Z""#));

        let without_rewrite = r#"{"createdAt":"2026-10-16T09:12:03Z","duration":1,"engine":"none","id":"x","transcript":""}"#;
        let entry: RecordingEntry = serde_json::from_str(without_rewrite).unwrap();
        assert!(!serde_json::to_string(&entry).unwrap().contains("rewritten"));
    }
}
