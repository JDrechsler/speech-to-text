use std::time::Duration;

use reqwest::multipart::Form;
use serde_json::Value;

use super::{CloudError, CloudRequest};
use crate::engines::{ELEVENLABS_KEY, Engine};
use crate::secrets;

const ENDPOINT: &str = "https://api.elevenlabs.io/v1/speech-to-text";
const MODEL: &str = "scribe_v2";
const TIMEOUT: Duration = Duration::from_secs(120);

pub async fn transcribe(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    let key = secrets::read(ELEVENLABS_KEY.id).ok_or(CloudError::MissingKey(Engine::Scribe))?;
    let audio = super::read_audio(request.wav).await?;
    send(ENDPOINT, &key, audio, &request.dictionary).await
}

async fn send(
    endpoint: &str,
    key: &str,
    audio: Vec<u8>,
    dictionary: &[String],
) -> Result<String, CloudError> {
    let request = super::http()
        .post(endpoint)
        .timeout(TIMEOUT)
        .header("xi-api-key", key)
        .multipart(form(audio, dictionary));
    let reply = super::fetch(Engine::Scribe, request).await?;
    if reply.status != 200 {
        return Err(CloudError::Status {
            engine: Engine::Scribe,
            status: reply.status,
        });
    }
    transcript(&reply.body).ok_or(CloudError::BadResponse(Engine::Scribe))
}

fn form(audio: Vec<u8>, dictionary: &[String]) -> Form {
    let form = Form::new()
        .part("file", super::wav_part(audio))
        .text("model_id", MODEL);
    dictionary
        .iter()
        .fold(form, |form, term| form.text("keyterms", term.clone()))
}

fn transcript(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    let text = json.get("text")?.as_str()?;
    Some(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::test_server;

    fn terms() -> Vec<String> {
        vec!["Kubernetes".into(), "Drechsler".into()]
    }

    #[tokio::test]
    async fn sends_the_recording_model_and_keyterms() {
        let server = test_server::serve(vec![(200, r#"{"text":"  Hello Kubernetes \n"}"#.into())]);
        let endpoint = format!("{}/v1/speech-to-text", server.url);

        let text = send(&endpoint, "test-key", b"RIFF-audio".to_vec(), &terms()).await;

        assert_eq!(text.unwrap(), "Hello Kubernetes");
        let received = server.received();
        let request = &received[0];
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/v1/speech-to-text");
        assert_eq!(request.header("xi-api-key"), Some("test-key"));
        assert!(request.header("user-agent").unwrap().starts_with("SpeechToText/"));
        let content_type = request.header("content-type").unwrap();
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        let body = request.body_text();
        let file = "Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n\
                    Content-Type: audio/wav\r\n\r\nRIFF-audio\r\n";
        let model = "Content-Disposition: form-data; name=\"model_id\"\r\n\r\nscribe_v2\r\n";
        let first = "Content-Disposition: form-data; name=\"keyterms\"\r\n\r\nKubernetes\r\n";
        let second = "Content-Disposition: form-data; name=\"keyterms\"\r\n\r\nDrechsler\r\n";
        let positions: Vec<usize> = [file, model, first, second]
            .iter()
            .map(|part| body.find(part).unwrap_or_else(|| panic!("missing part {part:?}")))
            .collect();
        assert!(positions.is_sorted());
    }

    #[tokio::test]
    async fn empty_dictionary_sends_no_keyterms() {
        let server = test_server::serve(vec![(200, r#"{"text":"hi"}"#.into())]);
        send(&server.url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap();
        assert!(!server.received()[0].body_text().contains("keyterms"));
    }

    #[tokio::test]
    async fn non_200_status_is_reported_with_its_code() {
        let server = test_server::serve(vec![(401, r#"{"detail":"invalid key"}"#.into())]);
        let error = send(&server.url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::Status { engine: Engine::Scribe, status: 401 }));
        assert_eq!(error.to_string(), "Scribe request failed (401). Internet up, key right?");
    }

    #[tokio::test]
    async fn unreadable_body_is_a_bad_response() {
        let server = test_server::serve(vec![(200, "<html>oops</html>".into())]);
        let error = send(&server.url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::BadResponse(Engine::Scribe)));
        assert_eq!(
            error.to_string(),
            "Scribe returned an unreadable response. Audio saved to history."
        );
    }

    #[test]
    fn transcript_needs_a_text_string() {
        assert_eq!(transcript(br#"{"text":" ok "}"#), Some("ok".into()));
        assert_eq!(transcript(br#"{"text":42}"#), None);
        assert_eq!(transcript(br#"{"words":[]}"#), None);
        assert_eq!(transcript(br#"["text"]"#), None);
    }
}
