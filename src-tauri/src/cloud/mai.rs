use std::time::Duration;

use reqwest::Url;
use reqwest::multipart::Form;
use serde_json::{Value, json};

use super::{CloudError, CloudRequest};
use crate::engines::{AZURE_SPEECH_KEY, Engine};
use crate::secrets;

const MODEL: &str = "MAI-Transcribe-2";
const API_VERSION: &str = "2025-10-15";
const MAX_PHRASES: usize = 50;
const TIMEOUT: Duration = Duration::from_secs(120);

pub async fn transcribe(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    let key = secrets::read(AZURE_SPEECH_KEY.id).ok_or(CloudError::MissingKey(Engine::Mai))?;
    let url = request
        .azure_endpoint
        .as_deref()
        .and_then(endpoint_url)
        .ok_or(CloudError::MissingEndpoint)?;
    let audio = super::read_audio(request.wav).await?;
    send(url, &key, audio, &request.dictionary).await
}

fn endpoint_url(raw: &str) -> Option<Url> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let base = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}.cognitiveservices.azure.com")
    };
    let trimmed = base.strip_suffix('/').unwrap_or(&base);
    Url::parse(&format!(
        "{trimmed}/speechtotext/transcriptions:transcribe?api-version={API_VERSION}"
    ))
    .ok()
}

async fn send(
    url: Url,
    key: &str,
    audio: Vec<u8>,
    dictionary: &[String],
) -> Result<String, CloudError> {
    let request = super::http()
        .post(url)
        .timeout(TIMEOUT)
        .header("Ocp-Apim-Subscription-Key", key)
        .multipart(form(audio, dictionary));
    let reply = super::fetch(Engine::Mai, request).await?;
    if reply.status != 200 {
        return Err(CloudError::Status {
            engine: Engine::Mai,
            status: reply.status,
        });
    }
    transcript(&reply.body).ok_or(CloudError::BadResponse(Engine::Mai))
}

fn form(audio: Vec<u8>, dictionary: &[String]) -> Form {
    Form::new()
        .part("audio", super::wav_part(audio))
        .text("definition", definition(dictionary).to_string())
}

fn definition(dictionary: &[String]) -> Value {
    let mut definition = json!({
        "enhancedMode": {
            "enabled": true,
            "model": MODEL,
            "modelOptions": {
                "transcribeStyle": "clean",
                "timestamps": "none",
            },
        }
    });
    let phrases: Vec<&String> = dictionary.iter().take(MAX_PHRASES).collect();
    if !phrases.is_empty() {
        definition["phraseList"] = json!({ "phrases": phrases });
    }
    definition
}

fn transcript(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    let phrases = json.get("combinedPhrases")?.as_array()?;
    let phrases = phrases
        .iter()
        .map(Value::as_object)
        .collect::<Option<Vec<_>>>()?;
    let texts: Vec<&str> = phrases
        .iter()
        .filter_map(|phrase| phrase.get("text")?.as_str())
        .collect();
    Some(texts.join(" ").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::test_server;

    const TRANSCRIBE_PATH: &str =
        "/speechtotext/transcriptions:transcribe?api-version=2025-10-15";

    fn endpoint(raw: &str) -> Option<String> {
        endpoint_url(raw).map(String::from)
    }

    #[test]
    fn bare_resource_name_becomes_the_azure_host() {
        let expected = format!("https://my-speech.cognitiveservices.azure.com{TRANSCRIBE_PATH}");
        assert_eq!(endpoint("my-speech"), Some(expected));
    }

    #[test]
    fn full_endpoint_keeps_its_host_and_loses_one_trailing_slash() {
        let expected = format!("https://eastus.api.cognitive.microsoft.com{TRANSCRIBE_PATH}");
        let with_slash = endpoint("https://eastus.api.cognitive.microsoft.com/");
        let with_spaces = endpoint("  https://eastus.api.cognitive.microsoft.com \n");
        assert_eq!(with_slash.as_ref(), Some(&expected));
        assert_eq!(with_spaces.as_ref(), Some(&expected));
    }

    #[test]
    fn blank_or_malformed_endpoint_is_missing() {
        assert_eq!(endpoint(""), None);
        assert_eq!(endpoint("   "), None);
        assert_eq!(endpoint("my speech resource"), None);
        assert_eq!(
            CloudError::MissingEndpoint.to_string(),
            "MAI-Transcribe-2 needs your Azure Speech endpoint. Add it in Settings."
        );
    }

    #[test]
    fn definition_without_dictionary_has_no_phrase_list() {
        assert_eq!(
            definition(&[]),
            json!({
                "enhancedMode": {
                    "enabled": true,
                    "model": "MAI-Transcribe-2",
                    "modelOptions": { "transcribeStyle": "clean", "timestamps": "none" },
                }
            })
        );
    }

    #[test]
    fn phrase_list_is_capped_at_fifty() {
        let dictionary: Vec<String> = (0..60).map(|index| format!("term{index}")).collect();
        let phrases = definition(&dictionary)["phraseList"]["phrases"].clone();
        assert_eq!(phrases, json!(dictionary[..50].to_vec()));
    }

    #[test]
    fn combined_phrases_are_joined_with_spaces() {
        let body = br#"{
            "durationMilliseconds": 3000,
            "combinedPhrases": [
                {"text": "Hello there."},
                {"channel": 1},
                {"text": "How are you? "}
            ],
            "phrases": []
        }"#;
        assert_eq!(transcript(body), Some("Hello there. How are you?".into()));
        assert_eq!(transcript(br#"{"combinedPhrases":[]}"#), Some(String::new()));
    }

    #[test]
    fn missing_or_malformed_combined_phrases_is_unreadable() {
        assert_eq!(transcript(br#"{"phrases":[]}"#), None);
        assert_eq!(transcript(br#"{"combinedPhrases":"hello"}"#), None);
        assert_eq!(transcript(br#"{"combinedPhrases":[{"text":"a"},"b"]}"#), None);
        assert_eq!(transcript(b"not json"), None);
    }

    #[tokio::test]
    async fn sends_audio_and_definition_parts_with_the_subscription_key() {
        let reply = r#"{"combinedPhrases":[{"text":"Hi Tauri"}]}"#;
        let server = test_server::serve(vec![(200, reply.into())]);
        let url = endpoint_url(&format!("{}/", server.url)).unwrap();

        let text = send(url, "test-key", b"RIFF-audio".to_vec(), &["Tauri".into()]).await;

        assert_eq!(text.unwrap(), "Hi Tauri");
        let received = server.received();
        let request = &received[0];
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, TRANSCRIBE_PATH);
        assert_eq!(request.header("ocp-apim-subscription-key"), Some("test-key"));
        let content_type = request.header("content-type").unwrap();
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        let body = request.body_text();
        let audio = "Content-Disposition: form-data; name=\"audio\"; filename=\"audio.wav\"\r\n\
                     Content-Type: audio/wav\r\n\r\nRIFF-audio\r\n";
        assert!(body.contains(audio));
        let marker = "Content-Disposition: form-data; name=\"definition\"\r\n\r\n";
        let start = body.find(marker).unwrap() + marker.len();
        let end = start + body[start..].find("\r\n").unwrap();
        let sent: Value = serde_json::from_str(&body[start..end]).unwrap();
        assert_eq!(sent, definition(&["Tauri".into()]));
        assert_eq!(sent["phraseList"]["phrases"], json!(["Tauri"]));
    }

    #[tokio::test]
    async fn non_200_status_mentions_the_endpoint() {
        let server = test_server::serve(vec![(404, "{}".into())]);
        let url = endpoint_url(&server.url).unwrap();
        let error = send(url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(
            error,
            CloudError::Status {
                engine: Engine::Mai,
                status: 404
            }
        ));
        assert_eq!(
            error.to_string(),
            "MAI-Transcribe-2 request failed (404). Internet up, key and endpoint right?"
        );
    }

    #[tokio::test]
    async fn unreadable_body_is_a_bad_response() {
        let server = test_server::serve(vec![(200, r#"{"phrases":[]}"#.into())]);
        let url = endpoint_url(&server.url).unwrap();
        let error = send(url, "test-key", b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "MAI-Transcribe-2 returned an unreadable response. Audio saved to history."
        );
    }
}
