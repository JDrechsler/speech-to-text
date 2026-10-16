use std::time::Duration;

use serde_json::{Value, json};

use super::{CloudError, CloudRequest, Reply};
use crate::engines::{ASSEMBLYAI_KEY, Engine};
use crate::secrets;

const BASE_URL: &str = "https://api.assemblyai.com/v2";
const SPEECH_MODEL: &str = "universal-3-5-pro";
const MAX_KEYTERMS: usize = 100;
const MAX_POLLS: usize = 120;
const QUICK_POLLS: usize = 8;
const QUICK_POLL_DELAY: Duration = Duration::from_millis(350);
const SLOW_POLL_DELAY: Duration = Duration::from_millis(1500);
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn transcribe(request: &CloudRequest<'_>) -> Result<String, CloudError> {
    let key = secrets::read(ASSEMBLYAI_KEY.id).ok_or(CloudError::MissingKey(Engine::Assembly))?;
    let audio = super::read_audio(request.wav).await?;
    let api = Api {
        base_url: BASE_URL,
        key: &key,
        poll_delay,
    };
    api.transcribe(audio, &request.dictionary).await
}

fn poll_delay(poll: usize) -> Duration {
    if poll < QUICK_POLLS {
        QUICK_POLL_DELAY
    } else {
        SLOW_POLL_DELAY
    }
}

struct Api<'a> {
    base_url: &'a str,
    key: &'a str,
    poll_delay: fn(usize) -> Duration,
}

impl Api<'_> {
    async fn transcribe(
        &self,
        audio: Vec<u8>,
        dictionary: &[String],
    ) -> Result<String, CloudError> {
        let audio_url = self.upload(audio).await?;
        let job_id = self.create(&audio_url, dictionary).await?;
        self.poll(&job_id).await
    }

    async fn upload(&self, audio: Vec<u8>) -> Result<String, CloudError> {
        let request = super::http()
            .post(format!("{}/upload", self.base_url))
            .timeout(UPLOAD_TIMEOUT)
            .header("authorization", self.key)
            .body(audio);
        let reply = super::fetch(Engine::Assembly, request).await?;
        string_field(&reply, "upload_url", "upload")
    }

    async fn create(&self, audio_url: &str, dictionary: &[String]) -> Result<String, CloudError> {
        let request = super::http()
            .post(format!("{}/transcript", self.base_url))
            .timeout(REQUEST_TIMEOUT)
            .header("authorization", self.key)
            .json(&job_body(audio_url, dictionary));
        let reply = super::fetch(Engine::Assembly, request).await?;
        string_field(&reply, "id", "create")
    }

    async fn poll(&self, job_id: &str) -> Result<String, CloudError> {
        let status_url = format!("{}/transcript/{job_id}", self.base_url);
        for poll in 0..MAX_POLLS {
            tokio::time::sleep((self.poll_delay)(poll)).await;
            let request = super::http()
                .get(&status_url)
                .timeout(REQUEST_TIMEOUT)
                .header("authorization", self.key);
            let reply = super::fetch(Engine::Assembly, request).await?;
            match job_state(&reply.body) {
                JobState::Completed(text) => return Ok(text),
                JobState::Failed(message) => {
                    return Err(CloudError::Provider {
                        engine: Engine::Assembly,
                        message,
                    });
                }
                JobState::Pending => continue,
            }
        }
        Err(CloudError::TimedOut(Engine::Assembly))
    }
}

fn string_field(reply: &Reply, field: &str, step: &'static str) -> Result<String, CloudError> {
    if reply.status != 200 {
        return Err(CloudError::AssemblyStep {
            step,
            status: Some(reply.status),
        });
    }
    serde_json::from_slice::<Value>(&reply.body)
        .ok()
        .and_then(|json| Some(json.get(field)?.as_str()?.to_string()))
        .ok_or(CloudError::AssemblyStep { step, status: None })
}

fn job_body(audio_url: &str, dictionary: &[String]) -> Value {
    let mut body = json!({
        "audio_url": audio_url,
        "speech_models": [SPEECH_MODEL],
        "language_detection": true,
    });
    let terms: Vec<&String> = dictionary.iter().take(MAX_KEYTERMS).collect();
    if !terms.is_empty() {
        body["keyterms_prompt"] = json!(terms);
    }
    body
}

#[derive(Debug, PartialEq)]
enum JobState {
    Pending,
    Completed(String),
    Failed(String),
}

fn job_state(body: &[u8]) -> JobState {
    let Ok(json) = serde_json::from_slice::<Value>(body) else {
        return JobState::Pending;
    };
    let text_of = |field: &str, fallback: &str| {
        json.get(field)
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_string()
    };
    match json.get("status").and_then(Value::as_str) {
        Some("completed") => JobState::Completed(text_of("text", "").trim().to_string()),
        Some("error") => JobState::Failed(text_of("error", "unknown")),
        _ => JobState::Pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::test_server;

    fn api(base_url: &str) -> Api<'_> {
        Api {
            base_url,
            key: "test-key",
            poll_delay: |_| Duration::ZERO,
        }
    }

    fn job_replies(polls: &[&str]) -> Vec<(u16, String)> {
        let mut replies = vec![
            (200, r#"{"upload_url":"https://cdn.assemblyai.com/upload/abc"}"#.to_string()),
            (200, r#"{"id":"job-1","status":"queued"}"#.to_string()),
        ];
        replies.extend(polls.iter().map(|poll| (200, poll.to_string())));
        replies
    }

    #[tokio::test]
    async fn uploads_creates_the_job_and_polls_until_completed() {
        let server = test_server::serve(job_replies(&[
            r#"{"status":"processing"}"#,
            "not json",
            r#"{"status":"completed","text":"  Hallo Welt, hello world. "}"#,
        ]));

        let text = api(&server.url).transcribe(b"RIFF-audio".to_vec(), &["Tauri".into()]).await;

        assert_eq!(text.unwrap(), "Hallo Welt, hello world.");
        let received = server.received();
        let paths: Vec<(&str, &str)> = received
            .iter()
            .map(|request| (request.method.as_str(), request.path.as_str()))
            .collect();
        assert_eq!(
            paths,
            [
                ("POST", "/upload"),
                ("POST", "/transcript"),
                ("GET", "/transcript/job-1"),
                ("GET", "/transcript/job-1"),
                ("GET", "/transcript/job-1"),
            ]
        );
        assert!(received.iter().all(|request| request.header("authorization") == Some("test-key")));
        assert_eq!(received[0].body, b"RIFF-audio");
        assert_eq!(received[1].header("content-type"), Some("application/json"));
        let job: Value = serde_json::from_slice(&received[1].body).unwrap();
        assert_eq!(job, job_body("https://cdn.assemblyai.com/upload/abc", &["Tauri".into()]));
    }

    #[test]
    fn job_body_asks_for_universal_with_language_detection() {
        assert_eq!(
            job_body("https://cdn/a", &[]),
            json!({
                "audio_url": "https://cdn/a",
                "speech_models": ["universal-3-5-pro"],
                "language_detection": true,
            })
        );
    }

    #[test]
    fn keyterms_are_capped_at_one_hundred() {
        let dictionary: Vec<String> = (0..120).map(|index| format!("term{index}")).collect();
        let body = job_body("https://cdn/a", &dictionary);
        assert_eq!(body["keyterms_prompt"], json!(dictionary[..100].to_vec()));
    }

    #[test]
    fn job_state_follows_the_status_field() {
        assert_eq!(job_state(br#"{"status":"queued"}"#), JobState::Pending);
        assert_eq!(job_state(br#"{"text":"no status"}"#), JobState::Pending);
        assert_eq!(job_state(b"<html>"), JobState::Pending);
        assert_eq!(
            job_state(br#"{"status":"completed","text":null}"#),
            JobState::Completed(String::new())
        );
        assert_eq!(
            job_state(br#"{"status":"error","error":"no audio"}"#),
            JobState::Failed("no audio".into())
        );
        assert_eq!(job_state(br#"{"status":"error"}"#), JobState::Failed("unknown".into()));
    }

    #[test]
    fn polls_quickly_at_first_then_slow_down() {
        assert_eq!(poll_delay(0), Duration::from_millis(350));
        assert_eq!(poll_delay(7), Duration::from_millis(350));
        assert_eq!(poll_delay(8), Duration::from_millis(1500));
        assert_eq!(poll_delay(119), Duration::from_millis(1500));
    }

    #[tokio::test]
    async fn failed_job_reports_assemblys_message() {
        let failed = r#"{"status":"error","error":"Audio duration is too short."}"#;
        let server = test_server::serve(job_replies(&[failed]));
        let error = api(&server.url).transcribe(b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert_eq!(error.to_string(), "AssemblyAI error: Audio duration is too short.");
    }

    #[tokio::test]
    async fn rejected_upload_names_the_step_and_keeps_the_status() {
        let server = test_server::serve(vec![(401, r#"{"error":"Invalid API key"}"#.into())]);
        let error = api(&server.url).transcribe(b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::AssemblyStep { step: "upload", status: Some(401) }));
        assert_eq!(error.to_string(), "AssemblyAI upload failed. Internet up, key right?");
    }

    #[tokio::test]
    async fn unreadable_create_reply_names_the_create_step() {
        let mut replies = job_replies(&[]);
        replies[1] = (200, "{}".into());
        let server = test_server::serve(replies);
        let error = api(&server.url).transcribe(b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::AssemblyStep { step: "create", status: None }));
        assert_eq!(error.to_string(), "AssemblyAI create failed. Internet up, key right?");
    }

    #[tokio::test]
    async fn gives_up_after_one_hundred_twenty_polls() {
        let pending = [r#"{"status":"processing"}"#; MAX_POLLS];
        let server = test_server::serve(job_replies(&pending));
        let error = api(&server.url).transcribe(b"RIFF".to_vec(), &[]).await.unwrap_err();
        assert!(matches!(error, CloudError::TimedOut(Engine::Assembly)));
        assert_eq!(error.to_string(), "AssemblyAI timed out. Audio saved to history.");
        assert_eq!(server.received().len(), 2 + MAX_POLLS);
    }
}
