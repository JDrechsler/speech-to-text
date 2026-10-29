use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::paths;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModel {
    pub id: &'static str,
    pub name: &'static str,
    pub detail: &'static str,
    pub bytes: u64,
    pub is_recommended: bool,
}

pub const CATALOG: [WhisperModel; 4] = [
    WhisperModel {
        id: "base",
        name: "Base",
        detail: "Fastest and smallest. Fine for quick notes in clear English.",
        bytes: 147_951_465,
        is_recommended: false,
    },
    WhisperModel {
        id: "small",
        name: "Small",
        detail: "Better accuracy, still quick on most computers.",
        bytes: 487_601_967,
        is_recommended: false,
    },
    WhisperModel {
        id: "large-v3-turbo-q5_0",
        name: "Large v3 Turbo (compact)",
        detail: "Near-best accuracy in 99 languages at a third of the download.",
        bytes: 574_041_195,
        is_recommended: true,
    },
    WhisperModel {
        id: "large-v3-turbo",
        name: "Large v3 Turbo",
        detail: "Best accuracy. Needs about 2 GB of memory while it is loaded.",
        bytes: 1_624_555_275,
        is_recommended: false,
    },
];

pub const LANGUAGES: [(&str, &str); 13] = [
    ("auto", "Detect automatically"),
    ("en", "English"),
    ("de", "German"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("ja", "Japanese"),
    ("zh", "Chinese"),
];

impl WhisperModel {
    pub fn path(&self) -> PathBuf {
        paths::models_dir().join(format!("ggml-{}.bin", self.id))
    }

    pub fn is_installed(&self) -> bool {
        self.path().exists()
    }

    fn download_url(&self) -> String {
        format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin", self.id)
    }
}

pub fn find(id: &str) -> Option<&'static WhisperModel> {
    CATALOG.iter().find(|model| model.id == id)
}

pub fn recommended() -> &'static WhisperModel {
    CATALOG.iter().find(|model| model.is_recommended).unwrap_or(&CATALOG[0])
}

pub fn selected(stored: Option<&str>) -> &'static WhisperModel {
    if let Some(model) = stored.and_then(find) {
        return model;
    }
    if recommended().is_installed() {
        return recommended();
    }
    CATALOG
        .iter()
        .rev()
        .find(|model| model.is_installed())
        .unwrap_or_else(recommended)
}

#[derive(Default)]
pub struct Downloads {
    cancelled: Mutex<HashMap<&'static str, Arc<AtomicBool>>>,
}

impl Downloads {
    pub fn is_running(&self, id: &str) -> bool {
        self.cancelled.lock().unwrap().contains_key(id)
    }

    pub fn cancel(&self, id: &str) {
        if let Some(flag) = self.cancelled.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub async fn run<F, Fut>(&self, id: &'static str, work: F) -> Result<(), String>
    where
        F: FnOnce(Arc<AtomicBool>) -> Fut,
        Fut: Future<Output = Result<(), String>>,
    {
        let flag = Arc::new(AtomicBool::new(false));
        self.cancelled.lock().unwrap().insert(id, flag.clone());
        let result = work(flag).await;
        self.cancelled.lock().unwrap().remove(id);
        result
    }

    pub async fn download(
        &self,
        model: &'static WhisperModel,
        mut on_progress: impl FnMut(f64),
    ) -> Result<(), String> {
        self.run(model.id, |cancelled| async move {
            fetch_file(&model.download_url(), &model.path(), model.bytes, &cancelled, |written| {
                on_progress((written as f64 / model.bytes as f64).min(1.0))
            })
            .await
        })
        .await
    }
}

pub async fn fetch_file(
    url: &str,
    destination: &Path,
    expected_bytes: u64,
    cancelled: &AtomicBool,
    mut on_bytes: impl FnMut(u64),
) -> Result<(), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|error| format!("Download failed: {error}"))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(format!("Download failed (HTTP {status}). Try again later."));
    }
    let file_name = destination.file_name().unwrap_or_default().to_string_lossy();
    let partial = destination.with_file_name(format!("{file_name}.part"));
    let mut file = tokio::fs::File::create(&partial)
        .await
        .map_err(|error| error.to_string())?;
    let mut written = 0u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            drop(file);
            let _ = tokio::fs::remove_file(&partial).await;
            return Err("Download cancelled".into());
        }
        let chunk = chunk.map_err(|error| format!("Download interrupted: {error}"))?;
        file.write_all(&chunk).await.map_err(|error| error.to_string())?;
        written += chunk.len() as u64;
        on_bytes(written);
    }
    file.flush().await.map_err(|error| error.to_string())?;
    drop(file);
    if written != expected_bytes {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(format!("Download incomplete ({written} of {expected_bytes} bytes). Try again."));
    }
    tokio::fs::rename(&partial, destination)
        .await
        .map_err(|error| error.to_string())
}

pub fn delete(model: &WhisperModel) -> Result<(), String> {
    std::fs::remove_file(model.path()).map_err(|error| error.to_string())
}
