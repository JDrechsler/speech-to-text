use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Value, json};

use super::{CloudError, CloudRequest};
use crate::engines::{Engine, STEPFUN_KEY};
use crate::secrets;

const ENDPOINT: &str = "https://api.stepfun.ai/v1/audio/asr/sse";
const MODEL: &str = "stepaudio-3-asr-max";
const TIMEOUT: Duration = Duration::from_secs(120);

pub async fn transcribe(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    let key = secrets::read(STEPFUN_KEY.id).ok_or(CloudError::MissingKey(Engine::Stepfun))?;
    let audio = super::read_audio(request.wav).await?;
    send(ENDPOINT, &key, &audio, &request.stepfun_language).await
}

async fn send(
    endpoint: &str,
    key: &str,
    audio: &[u8],
    language: &str,
) -> Result<String, CloudError> {
    let request = super::http()
        .post(endpoint)
        .timeout(TIMEOUT)
        .bearer_auth(key)
        .header("Accept", "text/event-stream")
        .json(&body(audio, language));
    let reply = super::fetch(Engine::Stepfun, request).await?;
    if reply.status != 200 {
        return Err(CloudError::Status {
            engine: Engine::Stepfun,
            status: reply.status,
        });
    }
    transcript(&reply.body)
}

fn body(audio: &[u8], language: &str) -> Value {
    json!({
        "audio": {
            "data": BASE64.encode(audio),
            "input": {
                "transcription": {
                    "model": MODEL,
                    "language": language,
                    "enable_itn": true,
                },
                "format": { "type": "wav" },
            },
        }
    })
}

fn transcript(event_stream: &[u8]) -> Result<String, CloudError> {
    let text = String::from_utf8_lossy(event_stream);
    for event in text.split(['\n', '\r']).filter_map(event) {
        match event.get("type").and_then(Value::as_str) {
            Some("transcript.text.done") => {
                return Ok(string_field(&event, "text", "").trim().to_string());
            }
            Some("error") => {
                return Err(CloudError::Provider {
                    engine: Engine::Stepfun,
                    message: string_field(&event, "message", "unknown error").to_string(),
                });
            }
            _ => continue,
        }
    }
    Err(CloudError::BadResponse(Engine::Stepfun))
}

fn event(line: &str) -> Option<Value> {
    let payload = line.strip_prefix("data:")?.trim();
    serde_json::from_str::<Value>(payload)
        .ok()
        .filter(Value::is_object)
}

fn string_field<'a>(event: &'a Value, name: &str, fallback: &'a str) -> &'a str {
    event.get(name).and_then(Value::as_str).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::test_server;

    const STREAM: &str = "event: message\n\
        data: {\"type\":\"transcript.text.delta\",\"delta\":\"Hola\"}\n\n\
        data: {\"type\":\"transcript.text.done\",\"text\":\" Hola, ¿qué tal? \"}\n\n\
        data: [DONE]\n\n";

    #[test]
    fn body_carries_base64_audio_model_language_and_format() {
        assert_eq!(
            body(b"RIFF", "es"),
            json!({
                "audio": {
                    "data": "UklGRg==",
                    "input": {
                        "transcription": {
                            "model": "stepaudio-3-asr-max",
                            "language": "es",
                            "enable_itn": true,
                        },
                        "format": { "type": "wav" },
                    },
                }
            })
        );
    }

    #[test]
    fn done_event_yields_the_trimmed_transcript() {
        assert_eq!(transcript(STREAM.as_bytes()).unwrap(), "Hola, ¿qué tal?");
    }

    #[test]
    fn crlf_lines_and_missing_space_after_data_are_accepted() {
        let stream = "data:{\"type\":\"transcript.text.delta\"}\r\n\r\n\
                      data:{\"type\":\"transcript.text.done\",\"text\":\"Hi\"}\r\n\r\n";
        assert_eq!(transcript(stream.as_bytes()).unwrap(), "Hi");
    }

    #[test]
    fn error_event_becomes_a_provider_error() {
        let stream = "data: {\"type\":\"error\",\"message\":\"audio too short\"}\n\n";
        let error = transcript(stream.as_bytes()).unwrap_err();
        assert_eq!(error.to_string(), "StepFun error: audio too short");
        let silent = transcript(b"data: {\"type\":\"error\"}\n\n").unwrap_err();
        assert_eq!(silent.to_string(), "StepFun error: unknown error");
    }

    #[test]
    fn first_decisive_event_wins() {
        let stream = "data: {\"type\":\"transcript.text.done\",\"text\":\"first\"}\n\
                      data: {\"type\":\"error\",\"message\":\"late\"}\n";
        assert_eq!(transcript(stream.as_bytes()).unwrap(), "first");
    }

    #[test]
    fn stream_without_done_event_has_no_transcript() {
        let stream = "data: {\"type\":\"transcript.text.delta\",\"delta\":\"Hi\"}\n\n\
                      data: [DONE]\n";
        let error = transcript(stream.as_bytes()).unwrap_err();
        assert!(matches!(error, CloudError::BadResponse(Engine::Stepfun)));
        assert_eq!(
            error.to_string(),
            "StepFun returned no transcript. Audio saved to history."
        );
        assert!(matches!(transcript(b""), Err(CloudError::BadResponse(_))));
    }

    #[tokio::test]
    async fn posts_json_with_bearer_key_and_reads_the_event_stream() {
        let server = test_server::serve(vec![(200, STREAM.into())]);
        let endpoint = format!("{}/v1/audio/asr/sse", server.url);

        let text = send(&endpoint, "test-key", b"RIFF", "es").await;

        assert_eq!(text.unwrap(), "Hola, ¿qué tal?");
        let received = server.received();
        let request = &received[0];
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/audio/asr/sse");
        assert_eq!(request.header("authorization"), Some("Bearer test-key"));
        assert_eq!(request.header("accept"), Some("text/event-stream"));
        assert_eq!(request.header("content-type"), Some("application/json"));
        let sent: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(sent, body(b"RIFF", "es"));
    }

    #[tokio::test]
    async fn rejected_key_and_low_balance_have_their_own_messages() {
        let replies = [401, 402, 503].map(|status| (status, "{}".to_string()));
        let server = test_server::serve(replies.into());
        let mut messages = Vec::new();
        for _ in 0..3 {
            let error = send(&server.url, "test-key", b"RIFF", "en").await.unwrap_err();
            messages.push(error.to_string());
        }
        assert_eq!(
            messages,
            [
                "StepFun rejected the API key. Check it in Settings.",
                "StepFun says your account balance is too low.",
                "StepFun request failed (503). Internet up, key right?",
            ]
        );
    }
}
