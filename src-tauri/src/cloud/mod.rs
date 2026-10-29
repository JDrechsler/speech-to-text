mod assembly;
mod mai;
mod openai;
mod retry;
mod scribe;
mod stepfun;

use std::fmt;
use std::path::Path;
use std::sync::OnceLock;

use reqwest::multipart::Part;

use crate::engines::Engine;

const USER_AGENT: &str = concat!("SpeechToText/", env!("CARGO_PKG_VERSION"));

pub struct CloudRequest<'a> {
    pub engine: Engine,
    pub wav: &'a Path,
    pub recording_seconds: f64,
    pub dictionary: Vec<String>,
    pub azure_endpoint: Option<String>,
    pub stepfun_language: String,
}

#[derive(Debug)]
pub enum CloudError {
    LocalEngine,
    MissingKey(Engine),
    MissingEndpoint,
    UnreadableAudio(String),
    InvalidRequest(Engine),
    Network { engine: Engine, failure: NetworkFailure },
    Status { engine: Engine, status: u16 },
    AssemblyStep { step: &'static str, status: Option<u16> },
    BadResponse(Engine),
    Provider { engine: Engine, message: String },
    TimedOut(Engine),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkFailure {
    TimedOut,
    Unreachable,
    Interrupted,
}

pub async fn transcribe(
    request: &CloudRequest<'_>,
    on_attempt: impl Fn(u32),
) -> Result<String, CloudError> {
    if request.engine.is_local() {
        return Err(CloudError::LocalEngine);
    }
    retry::run(
        request.engine,
        request.recording_seconds,
        &retry::BACKOFF,
        on_attempt,
        || attempt(request),
    )
    .await
}

async fn attempt(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    match request.engine {
        Engine::Whisper => Err(CloudError::LocalEngine),
        Engine::Mai => mai::transcribe(request).await,
        Engine::Scribe => scribe::transcribe(request).await,
        Engine::Assembly => assembly::transcribe(request).await,
        Engine::Stepfun => stepfun::transcribe(request).await,
        Engine::Openai => openai::transcribe(request).await,
    }
}

impl fmt::Display for CloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for CloudError {}

impl CloudError {
    fn message(&self) -> String {
        match self {
            CloudError::LocalEngine => "Whisper runs on this computer, not in the cloud.".into(),
            CloudError::MissingKey(Engine::Scribe) => {
                "Scribe needs your ElevenLabs API key. Add it in Settings.".into()
            }
            CloudError::MissingKey(Engine::Mai) => {
                "MAI-Transcribe-2 needs your Azure Speech key. Add it in Settings.".into()
            }
            CloudError::MissingKey(engine) => {
                format!("{} needs your API key. Add it in Settings.", label(*engine))
            }
            CloudError::MissingEndpoint => {
                "MAI-Transcribe-2 needs your Azure Speech endpoint. Add it in Settings.".into()
            }
            CloudError::UnreadableAudio(reason) => {
                format!("Could not read the recording: {reason}")
            }
            CloudError::InvalidRequest(engine) => format!(
                "{} could not send the request. Check the key in Settings.",
                label(*engine)
            ),
            CloudError::Network { engine, failure } => match failure {
                NetworkFailure::TimedOut => {
                    format!("{} request timed out. Internet up?", label(*engine))
                }
                NetworkFailure::Unreachable => {
                    format!("{} could not be reached. Internet up?", label(*engine))
                }
                NetworkFailure::Interrupted => {
                    format!("{} connection dropped. Internet up?", label(*engine))
                }
            },
            CloudError::Status {
                engine: Engine::Stepfun,
                status: 401,
            } => "StepFun rejected the API key. Check it in Settings.".into(),
            CloudError::Status {
                engine: Engine::Stepfun,
                status: 402,
            } => "StepFun says your account balance is too low.".into(),
            CloudError::Status {
                engine: Engine::Mai,
                status,
            } => format!(
                "MAI-Transcribe-2 request failed ({status}). Internet up, key and endpoint right?"
            ),
            CloudError::Status { engine, status } => format!(
                "{} request failed ({status}). Internet up, key right?",
                label(*engine)
            ),
            CloudError::AssemblyStep { step, .. } => {
                format!("AssemblyAI {step} failed. Internet up, key right?")
            }
            CloudError::BadResponse(Engine::Stepfun) => {
                "StepFun returned no transcript. Audio saved to history.".into()
            }
            CloudError::BadResponse(engine) => format!(
                "{} returned an unreadable response. Audio saved to history.",
                label(*engine)
            ),
            CloudError::Provider { engine, message } => {
                format!("{} error: {message}", label(*engine))
            }
            CloudError::TimedOut(engine) => {
                format!("{} timed out. Audio saved to history.", label(*engine))
            }
        }
    }
}

fn label(engine: Engine) -> &'static str {
    match engine {
        Engine::Mai => engine.display_name(),
        _ => engine.short_name(),
    }
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .expect("the HTTP client builds with the bundled TLS roots")
    })
}

struct Reply {
    status: u16,
    body: Vec<u8>,
}

async fn fetch(engine: Engine, request: reqwest::RequestBuilder) -> Result<Reply, CloudError> {
    let response = request
        .send()
        .await
        .map_err(|error| network_error(engine, error))?;
    let status = response.status().as_u16();
    let body = response
        .bytes()
        .await
        .map_err(|error| network_error(engine, error))?;
    Ok(Reply {
        status,
        body: body.to_vec(),
    })
}

fn network_error(engine: Engine, error: reqwest::Error) -> CloudError {
    if error.is_builder() {
        return CloudError::InvalidRequest(engine);
    }
    let failure = if error.is_timeout() {
        NetworkFailure::TimedOut
    } else if error.is_connect() {
        NetworkFailure::Unreachable
    } else {
        NetworkFailure::Interrupted
    };
    CloudError::Network { engine, failure }
}

async fn read_audio(path: &Path) -> Result<Vec<u8>, CloudError> {
    tokio::fs::read(path)
        .await
        .map_err(|error| CloudError::UnreadableAudio(error.to_string()))
}

fn wav_part(audio: Vec<u8>) -> Part {
    Part::bytes(audio)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .expect("audio/wav is a valid MIME type")
}

#[cfg(test)]
mod test_server {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::thread;

    pub struct Received {
        pub method: String,
        pub path: String,
        pub headers: Vec<(String, String)>,
        pub body: Vec<u8>,
    }

    impl Received {
        pub fn header(&self, name: &str) -> Option<&str> {
            header_value(&self.headers, name)
        }

        pub fn body_text(&self) -> String {
            String::from_utf8_lossy(&self.body).into_owned()
        }
    }

    pub struct TestServer {
        pub url: String,
        received: Arc<Mutex<Vec<Received>>>,
    }

    impl TestServer {
        pub fn received(&self) -> MutexGuard<'_, Vec<Received>> {
            self.received.lock().unwrap()
        }
    }

    pub fn serve(replies: Vec<(u16, String)>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        thread::spawn(move || {
            for (status, body) in replies {
                let (stream, _) = listener.accept().unwrap();
                answer(stream, status, &body, &log);
            }
        });
        TestServer { url, received }
    }

    fn answer(mut stream: TcpStream, status: u16, body: &str, log: &Mutex<Vec<Received>>) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        log.lock().unwrap().push(read_request(&mut reader));
        let head = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body.as_bytes()).unwrap();
    }

    fn read_request(reader: &mut impl BufRead) -> Received {
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_string();
        let path = parts.next().unwrap_or_default().to_string();
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let Some((name, value)) = line.trim_end().split_once(':') else {
                break;
            };
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
        let length = header_value(&headers, "content-length")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        Received {
            method,
            path,
            headers,
            body,
        }
    }

    fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn request(engine: Engine) -> CloudRequest<'static> {
        CloudRequest {
            engine,
            wav: Path::new("/nonexistent/recording.wav"),
            recording_seconds: 1.0,
            dictionary: Vec::new(),
            azure_endpoint: None,
            stepfun_language: "en".into(),
        }
    }

    #[tokio::test]
    async fn whisper_is_refused_before_any_attempt() {
        let attempts = Cell::new(0);
        let count = |_| attempts.set(attempts.get() + 1);
        let result = transcribe(&request(Engine::Whisper), count).await;
        assert!(matches!(result, Err(CloudError::LocalEngine)));
        assert_eq!(attempts.get(), 0);
    }

    #[test]
    fn transcribe_can_run_on_a_multi_threaded_runtime() {
        fn assert_send<T: Send>(_: &T) {}
        let request = request(Engine::Scribe);
        let future = transcribe(&request, |_| {});
        assert_send(&future);
    }

    #[test]
    fn missing_key_messages_point_to_settings() {
        let engines = [Engine::Scribe, Engine::Assembly, Engine::Mai, Engine::Stepfun];
        let messages = engines.map(|engine| CloudError::MissingKey(engine).to_string());
        assert_eq!(
            messages,
            [
                "Scribe needs your ElevenLabs API key. Add it in Settings.",
                "AssemblyAI needs your API key. Add it in Settings.",
                "MAI-Transcribe-2 needs your Azure Speech key. Add it in Settings.",
                "StepFun needs your API key. Add it in Settings.",
            ]
        );
    }

    #[test]
    fn network_failures_read_as_connection_problems() {
        let failure = |failure| CloudError::Network {
            engine: Engine::Assembly,
            failure,
        };
        assert_eq!(
            failure(NetworkFailure::TimedOut).to_string(),
            "AssemblyAI request timed out. Internet up?"
        );
        assert_eq!(
            failure(NetworkFailure::Unreachable).to_string(),
            "AssemblyAI could not be reached. Internet up?"
        );
        assert_eq!(
            failure(NetworkFailure::Interrupted).to_string(),
            "AssemblyAI connection dropped. Internet up?"
        );
    }

    #[test]
    fn messages_avoid_em_dashes() {
        let errors = [
            CloudError::LocalEngine,
            CloudError::MissingEndpoint,
            CloudError::UnreadableAudio("gone".into()),
            CloudError::InvalidRequest(Engine::Scribe),
            CloudError::Status {
                engine: Engine::Stepfun,
                status: 402,
            },
            CloudError::AssemblyStep {
                step: "upload",
                status: None,
            },
            CloudError::BadResponse(Engine::Mai),
            CloudError::Provider {
                engine: Engine::Assembly,
                message: "x".into(),
            },
            CloudError::TimedOut(Engine::Assembly),
        ];
        assert!(errors.iter().all(|error| !error.to_string().contains('\u{2014}')));
    }

    #[tokio::test]
    async fn unreadable_recording_is_reported() {
        let result = read_audio(Path::new("/nonexistent/recording.wav")).await;
        assert!(matches!(result, Err(CloudError::UnreadableAudio(_))));
    }

    #[tokio::test]
    async fn refused_connection_counts_as_unreachable() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let result = fetch(Engine::Scribe, http().get(url)).await;
        assert!(matches!(
            result,
            Err(CloudError::Network {
                engine: Engine::Scribe,
                failure: NetworkFailure::Unreachable
            })
        ));
    }

    #[tokio::test]
    async fn key_with_a_line_break_is_an_invalid_request() {
        let request = http().get("http://127.0.0.1:9").header("xi-api-key", "bad\nkey");
        let error = fetch(Engine::Scribe, request).await.err().unwrap();
        assert!(matches!(error, CloudError::InvalidRequest(Engine::Scribe)));
        assert_eq!(
            error.to_string(),
            "Scribe could not send the request. Check the key in Settings."
        );
    }
}
