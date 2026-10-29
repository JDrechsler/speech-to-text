use std::time::Duration;

use reqwest::multipart::Form;
use serde_json::Value;

use super::{CloudError, CloudRequest};
use crate::engines::{Engine, OPENAI_KEY};
use crate::secrets;

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const MODEL: &str = "gpt-transcribe";
const TIMEOUT: Duration = Duration::from_secs(120);

pub async fn transcribe(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    let key = secrets::read(OPENAI_KEY.id).ok_or(CloudError::MissingKey(Engine::Openai))?;
    let audio = super::read_audio(request.wav).await?;
    send(ENDPOINT, &key, audio, &request.dictionary).await
}

async fn send(endpoint: &str, key: &str, audio: Vec<u8>, dictionary: &[String]) -> Result<String, CloudError> {
    let request = super::http()
        .post(endpoint)
        .timeout(TIMEOUT)
        .bearer_auth(key)
        .multipart(form(audio, dictionary));
    let reply = super::fetch(Engine::Openai, request).await?;
    if reply.status != 200 {
        return Err(CloudError::Status { engine: Engine::Openai, status: reply.status });
    }
    transcript(&reply.body).ok_or(CloudError::BadResponse(Engine::Openai))
}

fn form(audio: Vec<u8>, dictionary: &[String]) -> Form {
    let form = Form::new().part("file", super::wav_part(audio)).text("model", MODEL);
    dictionary
        .iter()
        .filter(|term| is_allowed_keyword(term))
        .fold(form, |form, term| form.text("keywords[]", term.clone()))
}

fn is_allowed_keyword(term: &str) -> bool {
    !term.contains(['<', '>', '\r', '\n'])
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

    #[tokio::test]
    async fn sends_the_recording_model_and_keywords() {
        let server = test_server::serve(vec![(200, r#"{"text":" Hello Kubernetes ","languages":[{"code":"en"}]}"#.into())]);
        let endpoint = format!("{}/v1/audio/transcriptions", server.url);
        let terms = vec!["Kubernetes".into(), "Drechsler".into(), "<tag>".into()];

        let text = send(&endpoint, "test-key", b"RIFF-audio".to_vec(), &terms).await;

        assert_eq!(text.unwrap(), "Hello Kubernetes");
        let received = server.received();
        let request = &received[0];
        assert_eq!(request.path, "/v1/audio/transcriptions");
        assert_eq!(request.header("authorization"), Some("Bearer test-key"));
        let body = request.body_text();
        let file = "Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n\
                    Content-Type: audio/wav\r\n\r\nRIFF-audio\r\n";
        let model = "Content-Disposition: form-data; name=\"model\"\r\n\r\ngpt-transcribe\r\n";
        let first = "Content-Disposition: form-data; name=\"keywords[]\"\r\n\r\nKubernetes\r\n";
        let second = "Content-Disposition: form-data; name=\"keywords[]\"\r\n\r\nDrechsler\r\n";
        let positions: Vec<usize> = [file, model, first, second]
            .iter()
            .map(|part| body.find(part).unwrap_or_else(|| panic!("missing part {part:?}")))
            .collect();
        assert!(positions.is_sorted());
        assert!(!body.contains("<tag>"));
        assert!(!body.contains("languages"));
    }

    #[tokio::test]
    async fn non_200_status_is_reported_with_its_code() {
        let server = test_server::serve(vec![(401, r#"{"error":{"message":"bad key"}}"#.into())]);
        let error = send(&server.url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::Status { engine: Engine::Openai, status: 401 }));
        assert_eq!(error.to_string(), "GPT Transcribe request failed (401). Internet up, key right?");
    }

    #[test]
    fn transcript_needs_a_text_string() {
        assert_eq!(transcript(br#"{"text":" ok ","languages":[]}"#), Some("ok".into()));
        assert_eq!(transcript(br#"{"languages":[]}"#), None);
    }
}
