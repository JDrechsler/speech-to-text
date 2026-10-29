use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

use crate::cloud::{self, CloudRequest};
use crate::engines::Engine;
use crate::whisper::{self, LocalWhisper};
use crate::{dictionary, models, punctuation, settings};

struct Engines {
    whisper: LocalWhisper,
    runtime: tokio::runtime::Runtime,
    settings: settings::Settings,
    dictionary: Vec<String>,
    without_dictionary: Vec<Engine>,
}

impl Engines {
    fn transcribe(&self, engine: Engine, wav: &Path, seconds: f64) -> Result<String, String> {
        match engine {
            Engine::Whisper => {
                let model = models::selected(self.settings.whisper_model.as_deref()).path();
                let prompt = whisper::vocabulary_prompt(&self.dictionary);
                self.whisper.transcribe(wav, &model, &self.settings.whisper_language, prompt.as_deref())
            }
            _ => {
                let request = CloudRequest {
                    engine,
                    wav,
                    recording_seconds: seconds,
                    dictionary: if self.without_dictionary.contains(&engine) { Vec::new() } else { self.dictionary.clone() },
                    azure_endpoint: self.settings.azure_endpoint.clone(),
                    stepfun_language: self.settings.stepfun_language.clone(),
                };
                self.runtime.block_on(cloud::transcribe(&request, |_| {})).map_err(|error| error.to_string())
            }
        }
    }
}

fn engines_from_env(name: &str) -> Vec<Engine> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| Engine::from_id(id).unwrap_or_else(|| panic!("unknown engine {id}")))
        .collect()
}

fn recordings(dir: &Path) -> Vec<PathBuf> {
    let mut wavs: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("BENCH_DIR is readable")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
        .collect();
    wavs.sort();
    wavs
}

fn seconds_of(wav: &Path) -> f64 {
    let reader = hound::WavReader::open(wav).expect("a readable WAV");
    reader.duration() as f64 / reader.spec().sample_rate as f64
}

fn finished(out: &Path) -> HashSet<(String, String)> {
    std::fs::read_to_string(out)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|row| row["error"].is_null())
        .map(|row| (row["recording"].as_str().unwrap_or_default().into(), row["engine"].as_str().unwrap_or_default().into()))
        .collect()
}

#[test]
#[ignore = "benchmark: set BENCH_DIR, BENCH_ENGINES (comma-separated ids) and BENCH_OUT"]
fn benchmark_engines_on_recordings() {
    let dir = PathBuf::from(std::env::var("BENCH_DIR").expect("set BENCH_DIR"));
    let out = PathBuf::from(std::env::var("BENCH_OUT").expect("set BENCH_OUT"));
    let wanted = engines_from_env("BENCH_ENGINES");
    assert!(!wanted.is_empty(), "set BENCH_ENGINES");
    let engines = Engines {
        whisper: LocalWhisper::default(),
        runtime: tokio::runtime::Runtime::new().unwrap(),
        settings: settings::load(),
        dictionary: dictionary::terms(),
        without_dictionary: engines_from_env("BENCH_WITHOUT_DICTIONARY"),
    };
    let done = finished(&out);
    let wavs = recordings(&dir);
    let mut log = OpenOptions::new().create(true).append(true).open(&out).unwrap();

    for engine in wanted {
        if engine.is_local() {
            let _ = engines.transcribe(engine, &wavs[0], seconds_of(&wavs[0]));
        }
        for wav in &wavs {
            let recording = wav.file_stem().unwrap().to_string_lossy().to_string();
            if done.contains(&(recording.clone(), engine.id().to_string())) {
                continue;
            }
            let seconds = seconds_of(wav);
            let started = Instant::now();
            let result = engines.transcribe(engine, wav, seconds).map(|text| punctuation::with_final_period(&text));
            let elapsed = started.elapsed().as_secs_f64();
            let row = json!({
                "recording": recording,
                "engine": engine.id(),
                "audio_seconds": seconds,
                "elapsed_seconds": elapsed,
                "text": result.as_ref().ok(),
                "error": result.as_ref().err(),
            });
            writeln!(log, "{row}").unwrap();
            println!("{} {recording} {seconds:.1}s -> {elapsed:.2}s {}", engine.id(), if result.is_ok() { "ok" } else { "ERROR" });
        }
        engines.whisper.unload();
    }
}

