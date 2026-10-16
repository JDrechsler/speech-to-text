use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const THREADS: i32 = 4;

struct Loaded {
    path: PathBuf,
    context: WhisperContext,
}

pub struct LocalWhisper {
    loaded: Mutex<Option<Loaded>>,
    last_used: Mutex<Instant>,
}

impl Default for LocalWhisper {
    fn default() -> Self {
        LocalWhisper { loaded: Mutex::new(None), last_used: Mutex::new(Instant::now()) }
    }
}

impl LocalWhisper {
    #[cfg(test)]
    pub fn is_loaded(&self) -> bool {
        self.loaded.lock().unwrap().is_some()
    }

    pub fn load(&self, model: &Path) -> Result<(), String> {
        self.with_context(model, |_| ())
    }

    pub fn transcribe(
        &self,
        wav: &Path,
        model: &Path,
        language: &str,
        prompt: Option<&str>,
    ) -> Result<String, String> {
        let samples = read_samples(wav)?;
        if samples.is_empty() {
            return Ok(String::new());
        }
        self.with_context(model, |context| run(context, &samples, language, prompt))?
    }

    pub fn unload(&self) {
        *self.loaded.lock().unwrap() = None;
    }

    pub fn unload_if_idle_for(&self, idle: Duration) -> bool {
        if self.last_used.lock().unwrap().elapsed() < idle {
            return false;
        }
        let mut loaded = self.loaded.lock().unwrap();
        let was_loaded = loaded.is_some();
        *loaded = None;
        was_loaded
    }

    fn with_context<R>(&self, model: &Path, work: impl FnOnce(&WhisperContext) -> R) -> Result<R, String> {
        let mut loaded = self.loaded.lock().unwrap();
        if loaded.as_ref().is_none_or(|current| current.path != model) {
            *loaded = None;
            *loaded = Some(Loaded { path: model.to_path_buf(), context: load_context(model)? });
        }
        *self.last_used.lock().unwrap() = Instant::now();
        let context = &loaded.as_ref().expect("context was just loaded").context;
        let result = work(context);
        *self.last_used.lock().unwrap() = Instant::now();
        Ok(result)
    }
}

fn load_context(model: &Path) -> Result<WhisperContext, String> {
    if !model.exists() {
        return Err("The Whisper model is not downloaded. Open Settings to download it.".into());
    }
    whisper_rs::install_logging_hooks();
    let mut parameters = WhisperContextParameters::default();
    parameters.use_gpu(true).flash_attn(false);
    WhisperContext::new_with_params(model, parameters).map_err(|error| {
        format!("Could not load the Whisper model ({error}). Delete and download it again in Settings.")
    })
}

fn run(
    context: &WhisperContext,
    samples: &[f32],
    language: &str,
    prompt: Option<&str>,
) -> Result<String, String> {
    let mut state = context.create_state().map_err(|error| error.to_string())?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(THREADS);
    params.set_translate(false);
    params.set_no_timestamps(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    params.set_temperature(0.0);
    params.set_language(Some(language));
    if let Some(prompt) = prompt {
        params.set_initial_prompt(prompt);
    }
    state
        .full(params, samples)
        .map_err(|error| format!("Whisper failed to transcribe ({error}). Audio saved to history."))?;
    let text: Vec<String> = state
        .as_iter()
        .filter_map(|segment| segment.to_str_lossy().ok().map(|text| text.into_owned()))
        .collect();
    Ok(text.join(" ").split_whitespace().collect::<Vec<_>>().join(" "))
}

fn read_samples(wav: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(wav).map_err(|error| error.to_string())?;
    let spec = reader.spec();
    if spec.sample_rate != crate::audio::SAMPLE_RATE || spec.channels != 1 {
        return Err("Whisper needs a 16 kHz mono recording made by this app.".into());
    }
    match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| value as f32 / 32_768.0))
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string()),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string()),
    }
}

pub fn vocabulary_prompt(terms: &[String]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    Some(format!("Note to self. I sometimes mention {}.", terms.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a downloaded model and SPEECH_TO_TEXT_TEST_WAV"]
    fn transcribes_a_spoken_clip_and_frees_the_model() {
        let wav = std::env::var("SPEECH_TO_TEXT_TEST_WAV").expect("set SPEECH_TO_TEXT_TEST_WAV");
        let model = crate::models::selected(None).path();
        let whisper = LocalWhisper::default();

        let cold = Instant::now();
        let text = whisper.transcribe(Path::new(&wav), &model, "en", None).unwrap();
        let cold = cold.elapsed();
        let warm = Instant::now();
        whisper.transcribe(Path::new(&wav), &model, "en", None).unwrap();
        let warm = warm.elapsed();
        println!("TRANSCRIPT {text:?} first {cold:?} warm {warm:?}");

        assert!(text.to_lowercase().contains("dictation"), "{text}");
        assert!(whisper.is_loaded());
        assert!(whisper.unload_if_idle_for(Duration::ZERO));
        assert!(!whisper.is_loaded());
    }

    #[test]
    fn vocabulary_prompt_reads_like_dictation() {
        assert_eq!(vocabulary_prompt(&[]), None);
        assert_eq!(
            vocabulary_prompt(&["Kubernetes".into(), "Tauri".into()]).as_deref(),
            Some("Note to self. I sometimes mention Kubernetes, Tauri.")
        );
    }
}
