mod server;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::models::fetch_file;
use crate::paths;

pub use server::NemotronServer;

const MODEL_REVISION_URL: &str = "https://huggingface.co/AtomicChat/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-GGUF/resolve/dbd06c5cad7a2f33c8e06be2fb3ef814dbd9aa59";

const RUNTIME_VERSION: &str = "66d50d9";
const RUNTIME_URL: &str = "https://github.com/JDrechsler/speech-to-text/releases/download/nemo-speech-66d50d9/nemo-speech-nightly-macos-aarch64-metal.tar.gz";
const RUNTIME_ARCHIVE: &str = "nemo-speech-nightly-macos-aarch64-metal.tar.gz";
const RUNTIME_FOLDER: &str = "nemo-speech-nightly-macos-aarch64-metal";
const RUNTIME_BYTES: u64 = 7_234_179;
const RUNTIME_SHA256: &str = "4621515b0050d283d3a2a3f0382631ee7f876eea46608fdb52f7a05f50c958fc";

const ENCODER_FILE: &str = "NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-Encoder-F16.gguf";
const ENCODER_BYTES: u64 = 1_670_410_080;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NemotronModel {
    pub id: &'static str,
    pub name: &'static str,
    pub detail: &'static str,
    pub bytes: u64,
    pub is_recommended: bool,
    #[serde(skip)]
    decoder_file: &'static str,
    #[serde(skip)]
    decoder_bytes: u64,
}

pub const CATALOG: [NemotronModel; 2] = [
    NemotronModel {
        id: "nemotron-iq4_nl",
        name: "Compact (IQ4_NL)",
        detail: "Uses about 20 GB of memory while it is loaded. Can leave out punctuation in recordings longer than a minute.",
        bytes: RUNTIME_BYTES + ENCODER_BYTES + 19_678_656_672,
        is_recommended: true,
        decoder_file: "NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-AD-IQ4_NL.gguf",
        decoder_bytes: 19_678_656_672,
    },
    NemotronModel {
        id: "nemotron-q8_0",
        name: "Full (Q8_0)",
        detail: "Closest to NVIDIA's original and punctuates long recordings reliably. Uses about 34 GB of memory while it is loaded, so it needs a Mac with 64 GB.",
        bytes: RUNTIME_BYTES + ENCODER_BYTES + 35_004_643_232,
        is_recommended: false,
        decoder_file: "NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-Q8_0.gguf",
        decoder_bytes: 35_004_643_232,
    },
];

struct Download {
    url: String,
    path: PathBuf,
    bytes: u64,
}

impl Download {
    fn of_model_file(name: &str, bytes: u64) -> Download {
        Download { url: format!("{MODEL_REVISION_URL}/{name}"), path: model_dir().join(name), bytes }
    }

    fn is_done(&self) -> bool {
        std::fs::metadata(&self.path).is_ok_and(|metadata| metadata.len() == self.bytes)
    }
}

impl NemotronModel {
    pub fn decoder_path(&self) -> PathBuf {
        model_dir().join(self.decoder_file)
    }

    fn files(&self) -> [Download; 2] {
        [
            Download::of_model_file(ENCODER_FILE, ENCODER_BYTES),
            Download::of_model_file(self.decoder_file, self.decoder_bytes),
        ]
    }

    pub fn is_installed(&self) -> bool {
        binary().exists() && self.files().iter().all(Download::is_done)
    }
}

pub fn find(id: &str) -> Option<&'static NemotronModel> {
    CATALOG.iter().find(|model| model.id == id)
}

fn recommended() -> &'static NemotronModel {
    CATALOG.iter().find(|model| model.is_recommended).unwrap_or(&CATALOG[0])
}

pub fn selected(stored: Option<&str>) -> &'static NemotronModel {
    if let Some(model) = stored.and_then(find) {
        return model;
    }
    if recommended().is_installed() {
        return recommended();
    }
    CATALOG.iter().find(|model| model.is_installed()).unwrap_or_else(recommended)
}

fn model_dir() -> PathBuf {
    paths::models_dir().join("nemotron")
}

fn runtimes_dir() -> PathBuf {
    paths::support_dir().join("runtimes")
}

fn runtime_dir() -> PathBuf {
    runtimes_dir().join(format!("nemo-speech-{RUNTIME_VERSION}"))
}

fn binary() -> PathBuf {
    runtime_dir().join("bin").join("nemo-speech")
}

fn log_file() -> PathBuf {
    paths::support_dir().join("nemo-speech.log")
}

pub async fn download(
    model: &NemotronModel,
    cancelled: &AtomicBool,
    mut on_progress: impl FnMut(f64),
) -> Result<(), String> {
    let missing: Vec<Download> = model.files().into_iter().filter(|file| !file.is_done()).collect();
    let needs_runtime = !binary().exists();
    let total = missing.iter().map(|file| file.bytes).sum::<u64>() + if needs_runtime { RUNTIME_BYTES } else { 0 };
    let fraction = |bytes: u64| (bytes as f64 / total.max(1) as f64).min(1.0);
    let mut done = 0;
    if needs_runtime {
        install_runtime(cancelled, |bytes| on_progress(fraction(done + bytes))).await?;
        done += RUNTIME_BYTES;
    }
    tokio::fs::create_dir_all(model_dir()).await.map_err(|error| error.to_string())?;
    for file in missing {
        fetch_file(&file.url, &file.path, file.bytes, cancelled, |bytes| on_progress(fraction(done + bytes))).await?;
        done += file.bytes;
    }
    Ok(())
}

async fn install_runtime(cancelled: &AtomicBool, on_bytes: impl FnMut(u64)) -> Result<(), String> {
    tokio::fs::create_dir_all(runtimes_dir()).await.map_err(|error| error.to_string())?;
    let archive = runtimes_dir().join(RUNTIME_ARCHIVE);
    fetch_file(RUNTIME_URL, &archive, RUNTIME_BYTES, cancelled, on_bytes).await?;
    let result = tauri::async_runtime::spawn_blocking({
        let archive = archive.clone();
        move || unpack_runtime(&archive)
    })
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result);
    let _ = tokio::fs::remove_file(&archive).await;
    result
}

fn unpack_runtime(archive: &Path) -> Result<(), String> {
    let data = std::fs::read(archive).map_err(|error| error.to_string())?;
    if format!("{:x}", Sha256::digest(&data)) != RUNTIME_SHA256 {
        return Err("The NeMo-Speech runtime download was damaged. Try again.".into());
    }
    let staging = runtimes_dir().join("unpacking");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let unpacked = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .status()
        .is_ok_and(|status| status.success());
    if !unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("Could not unpack the NeMo-Speech runtime.".into());
    }
    let _ = std::fs::remove_dir_all(runtime_dir());
    let moved = std::fs::rename(staging.join(RUNTIME_FOLDER), runtime_dir()).map_err(|error| error.to_string());
    let _ = std::fs::remove_dir_all(&staging);
    moved
}

pub fn delete(model: &NemotronModel) -> Result<(), String> {
    let decoder = model.decoder_path();
    if decoder.exists() {
        std::fs::remove_file(&decoder).map_err(|error| error.to_string())?;
    }
    let others_remain = CATALOG.iter().any(|other| other.decoder_path().exists());
    if !others_remain {
        for dir in [model_dir(), runtime_dir()] {
            if dir.exists() {
                std::fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

pub fn stop_leftover_server() {
    if binary().exists() {
        let _ = Command::new("pkill").arg("-f").arg(binary()).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_files_come_from_one_pinned_revision() {
        for model in &CATALOG {
            let [encoder, decoder] = model.files();
            assert!(encoder.url.starts_with(MODEL_REVISION_URL));
            assert!(decoder.url.ends_with(model.decoder_file));
            assert_eq!(model.bytes, RUNTIME_BYTES + ENCODER_BYTES + model.decoder_bytes);
        }
        assert!(RUNTIME_URL.contains(RUNTIME_VERSION));
    }

    #[test]
    fn an_unknown_stored_model_falls_back_to_a_catalog_model() {
        assert_eq!(selected(Some("nemotron-q8_0")).id, "nemotron-q8_0");
        assert!(find(selected(Some("gone")).id).is_some());
    }

    #[test]
    #[ignore = "needs NEMO_SPEECH_TEST_ARCHIVE pointing at the runtime archive"]
    fn unpacks_the_verified_runtime_archive() {
        let archive = std::env::var("NEMO_SPEECH_TEST_ARCHIVE").expect("set NEMO_SPEECH_TEST_ARCHIVE");
        unpack_runtime(Path::new(&archive)).unwrap();
        assert!(binary().exists());
    }
}
