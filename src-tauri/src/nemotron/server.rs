use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use super::{binary, log_file};

const READY_TIMEOUT: Duration = Duration::from_secs(180);
const READY_POLL: Duration = Duration::from_millis(250);
const TRANSCRIBE_TIMEOUT: Duration = Duration::from_secs(600);

struct Running {
    child: Child,
    port: u16,
    decoder: PathBuf,
}

pub struct NemotronServer {
    running: Mutex<Option<Running>>,
    last_used: Mutex<Instant>,
}

impl Default for NemotronServer {
    fn default() -> Self {
        NemotronServer { running: Mutex::new(None), last_used: Mutex::new(Instant::now()) }
    }
}

#[derive(Deserialize)]
struct Ready {
    ready: bool,
}

#[derive(Deserialize)]
struct Transcript {
    text: String,
}

#[derive(Deserialize)]
struct Failure {
    error: FailureDetail,
}

#[derive(Deserialize)]
struct FailureDetail {
    message: String,
}

impl NemotronServer {
    pub fn start(&self, decoder: &Path) -> Result<u16, String> {
        *self.last_used.lock().unwrap() = Instant::now();
        let mut running = self.running.lock().unwrap();
        if let Some(current) = running.as_mut() {
            if current.decoder == decoder && matches!(current.child.try_wait(), Ok(None)) {
                return Ok(current.port);
            }
            let _ = current.child.kill();
            let _ = current.child.wait();
        }
        *running = None;
        if !decoder.exists() || !binary().exists() {
            return Err("Nemotron 3.5 Transcribe is not downloaded. Open Settings to download it.".into());
        }
        let port = free_port()?;
        let log = File::create(log_file()).map_err(|error| error.to_string())?;
        let child = Command::new(binary())
            .args(["serve", "--host", "127.0.0.1", "--no-ui", "--device", "metal"])
            .arg("--port")
            .arg(port.to_string())
            .arg("--asr-model")
            .arg(decoder)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|error| format!("Could not start Nemotron ({error})."))?;
        *running = Some(Running { child, port, decoder: decoder.to_path_buf() });
        Ok(port)
    }

    pub async fn transcribe(&self, wav: &Path, decoder: &Path) -> Result<String, String> {
        let port = self.start(decoder)?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|error| error.to_string())?;
        self.wait_until_ready(&client, port).await?;
        let audio = tokio::fs::read(wav).await.map_err(|error| error.to_string())?;
        let file = Part::bytes(audio)
            .file_name("recording.wav")
            .mime_str("audio/wav")
            .map_err(|error| error.to_string())?;
        let form = Form::new().part("file", file);
        *self.last_used.lock().unwrap() = Instant::now();
        let response = client
            .post(format!("http://127.0.0.1:{port}/v1/audio/transcriptions"))
            .multipart(form)
            .timeout(TRANSCRIBE_TIMEOUT)
            .send()
            .await
            .map_err(|error| format!("Nemotron did not answer ({error}). Audio saved to history."))?;
        *self.last_used.lock().unwrap() = Instant::now();
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            let detail = serde_json::from_str::<Failure>(&body).map_or(body, |failure| failure.error.message);
            return Err(format!("Nemotron failed ({detail}). Audio saved to history."));
        }
        serde_json::from_str::<Transcript>(&body)
            .map(|transcript| transcript.text)
            .map_err(|error| format!("Nemotron sent an unexpected answer ({error})."))
    }

    async fn wait_until_ready(&self, client: &reqwest::Client, port: u16) -> Result<(), String> {
        let started = Instant::now();
        let url = format!("http://127.0.0.1:{port}/ready");
        while started.elapsed() < READY_TIMEOUT {
            let ready = client.get(&url).timeout(READY_POLL * 4).send().await;
            if let Ok(response) = ready {
                if response.json::<Ready>().await.is_ok_and(|ready| ready.ready) {
                    return Ok(());
                }
            }
            if self.has_exited() {
                return Err(format!("Nemotron stopped while loading ({}).", last_log_line()));
            }
            tokio::time::sleep(READY_POLL).await;
        }
        self.stop();
        Err("Nemotron took too long to load. Audio saved to history.".into())
    }

    fn has_exited(&self) -> bool {
        let mut running = self.running.lock().unwrap();
        running.as_mut().is_none_or(|current| !matches!(current.child.try_wait(), Ok(None)))
    }

    pub fn stop(&self) {
        if let Some(mut current) = self.running.lock().unwrap().take() {
            let _ = current.child.kill();
            let _ = current.child.wait();
        }
    }

    pub fn stop_if_idle_for(&self, idle: Duration) -> bool {
        if self.last_used.lock().unwrap().elapsed() < idle {
            return false;
        }
        let was_running = self.running.lock().unwrap().is_some();
        self.stop();
        was_running
    }
}

fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("Could not find a free port for Nemotron ({error})."))
}

fn last_log_line() -> String {
    std::fs::read_to_string(log_file())
        .ok()
        .and_then(|log| log.lines().rev().find(|line| !line.trim().is_empty()).map(str::to_string))
        .unwrap_or_else(|| "no details".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs the downloaded Nemotron model and SPEECH_TO_TEXT_TEST_WAV"]
    fn transcribes_a_spoken_clip_and_stops() {
        let wav = std::env::var("SPEECH_TO_TEXT_TEST_WAV").expect("set SPEECH_TO_TEXT_TEST_WAV");
        let server = NemotronServer::default();
        let decoder = crate::nemotron::selected(std::env::var("NEMOTRON_TEST_MODEL").ok().as_deref()).decoder_path();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let cold = Instant::now();
        let text = runtime.block_on(server.transcribe(Path::new(&wav), &decoder)).unwrap();
        let cold = cold.elapsed();
        let warm = Instant::now();
        runtime.block_on(server.transcribe(Path::new(&wav), &decoder)).unwrap();
        let warm = warm.elapsed();
        println!("TRANSCRIPT {text:?} first {cold:?} warm {warm:?}");

        assert!(!text.trim().is_empty());
        assert!(server.stop_if_idle_for(Duration::ZERO));
        assert!(server.has_exited());
    }
}
