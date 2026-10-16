use std::future::Future;
use std::time::{Duration, Instant};

use super::CloudError;
use crate::engines::Engine;

pub const ATTEMPTS: u32 = 3;
pub const BACKOFF: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(2)];

pub async fn run<Call, Attempt>(
    engine: Engine,
    recording_seconds: f64,
    backoff: &[Duration],
    on_attempt: impl Fn(u32),
    mut call: Call,
) -> Result<String, CloudError>
where
    Call: FnMut() -> Attempt,
    Attempt: Future<Output = Result<String, CloudError>>,
{
    let mut attempt = 1;
    loop {
        on_attempt(attempt);
        let started = Instant::now();
        match call().await {
            Ok(text) => {
                if attempt > 1 {
                    eprintln!("{}", recovered_line(engine, attempt, started));
                }
                return Ok(text);
            }
            Err(error) => {
                eprintln!(
                    "{}",
                    failure_line(engine, attempt, recording_seconds, started, &error)
                );
                if !is_transient(&error) || attempt >= ATTEMPTS {
                    return Err(error);
                }
                tokio::time::sleep(pause_after(backoff, attempt)).await;
                attempt += 1;
            }
        }
    }
}

pub fn is_transient(error: &CloudError) -> bool {
    match error {
        CloudError::Network { .. } => true,
        CloudError::Status { status, .. } => is_transient_status(*status),
        CloudError::AssemblyStep { status, .. } => status.is_none_or(is_transient_status),
        _ => false,
    }
}

fn is_transient_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500..=599)
}

fn pause_after(backoff: &[Duration], attempt: u32) -> Duration {
    let index = (attempt as usize - 1).min(backoff.len().saturating_sub(1));
    backoff.get(index).copied().unwrap_or_default()
}

fn recovered_line(engine: Engine, attempt: u32, started: Instant) -> String {
    format!(
        "cloud_recovered engine={} attempt={attempt}/{ATTEMPTS} took={}",
        engine_name(engine),
        seconds(started.elapsed())
    )
}

fn failure_line(
    engine: Engine,
    attempt: u32,
    recording_seconds: f64,
    started: Instant,
    error: &CloudError,
) -> String {
    let transient = if is_transient(error) { "yes" } else { "no" };
    format!(
        "cloud_error engine={} attempt={attempt}/{ATTEMPTS} recording={recording_seconds:.1}s \
         took={} transient={transient} code={} error={error}",
        engine_name(engine),
        seconds(started.elapsed()),
        code(error)
    )
}

fn engine_name(engine: Engine) -> String {
    format!("{engine:?}").to_lowercase()
}

fn seconds(elapsed: Duration) -> String {
    format!("{:.1}s", elapsed.as_secs_f64())
}

fn code(error: &CloudError) -> String {
    match error {
        CloudError::Status { status, .. }
        | CloudError::AssemblyStep {
            status: Some(status),
            ..
        } => format!("HTTP{status}"),
        CloudError::Network { failure, .. } => format!("Network{failure:?}"),
        CloudError::AssemblyStep { status: None, .. } => "UnreadableReply".into(),
        CloudError::LocalEngine => "LocalEngine".into(),
        CloudError::MissingKey(_) => "MissingKey".into(),
        CloudError::MissingEndpoint => "MissingEndpoint".into(),
        CloudError::UnreadableAudio(_) => "UnreadableAudio".into(),
        CloudError::InvalidRequest(_) => "InvalidRequest".into(),
        CloudError::BadResponse(_) => "BadResponse".into(),
        CloudError::Provider { .. } => "ProviderError".into(),
        CloudError::TimedOut(_) => "TimedOut".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::NetworkFailure;
    use std::cell::RefCell;

    type Outcome = Result<String, CloudError>;

    fn status(status: u16) -> CloudError {
        CloudError::Status {
            engine: Engine::Scribe,
            status,
        }
    }

    fn network() -> CloudError {
        CloudError::Network {
            engine: Engine::Scribe,
            failure: NetworkFailure::Interrupted,
        }
    }

    fn assembly(step: &'static str, status: Option<u16>) -> CloudError {
        CloudError::AssemblyStep { step, status }
    }

    async fn run_outcomes(outcomes: Vec<Outcome>) -> (Outcome, Vec<u32>) {
        let attempts = RefCell::new(Vec::new());
        let mut outcomes = outcomes.into_iter();
        let result = run(
            Engine::Scribe,
            2.5,
            &[],
            |attempt| attempts.borrow_mut().push(attempt),
            || {
                let outcome = outcomes.next().expect("no more outcomes");
                async move { outcome }
            },
        )
        .await;
        (result, attempts.into_inner())
    }

    #[test]
    fn timeouts_rate_limits_and_server_errors_are_transient() {
        for code in [408, 429, 500, 502, 503, 599] {
            assert!(is_transient(&status(code)), "{code}");
        }
        for code in [200, 400, 401, 402, 403, 404, 413, 422] {
            assert!(!is_transient(&status(code)), "{code}");
        }
    }

    #[test]
    fn network_failures_and_assembly_hiccups_are_transient() {
        assert!(is_transient(&network()));
        assert!(is_transient(&assembly("upload", None)));
        assert!(is_transient(&assembly("create", Some(503))));
        assert!(!is_transient(&assembly("upload", Some(401))));
    }

    #[test]
    fn configuration_and_content_errors_are_permanent() {
        let permanent = [
            CloudError::MissingKey(Engine::Scribe),
            CloudError::MissingEndpoint,
            CloudError::UnreadableAudio("gone".into()),
            CloudError::InvalidRequest(Engine::Mai),
            CloudError::BadResponse(Engine::Mai),
            CloudError::Provider {
                engine: Engine::Stepfun,
                message: "bad audio".into(),
            },
            CloudError::TimedOut(Engine::Assembly),
        ];
        assert!(permanent.iter().all(|error| !is_transient(error)));
    }

    #[test]
    fn backoff_waits_one_then_two_seconds() {
        assert_eq!(pause_after(&BACKOFF, 1), Duration::from_secs(1));
        assert_eq!(pause_after(&BACKOFF, 2), Duration::from_secs(2));
        assert_eq!(pause_after(&BACKOFF, 3), Duration::from_secs(2));
        assert_eq!(pause_after(&[], 1), Duration::ZERO);
    }

    #[tokio::test]
    async fn transient_failures_are_retried_until_success() {
        let outcomes = vec![Err(status(503)), Err(network()), Ok("hello".into())];
        let (result, attempts) = run_outcomes(outcomes).await;
        assert_eq!(result.unwrap(), "hello");
        assert_eq!(attempts, [1, 2, 3]);
    }

    #[tokio::test]
    async fn gives_up_after_three_attempts() {
        let outcomes = vec![Err(status(500)), Err(status(502)), Err(status(503))];
        let (result, attempts) = run_outcomes(outcomes).await;
        assert!(matches!(result, Err(CloudError::Status { status: 503, .. })));
        assert_eq!(attempts, [1, 2, 3]);
    }

    #[tokio::test]
    async fn permanent_failure_is_not_retried() {
        let (result, attempts) = run_outcomes(vec![Err(status(401))]).await;
        assert!(matches!(result, Err(CloudError::Status { status: 401, .. })));
        assert_eq!(attempts, [1]);
    }

    #[test]
    fn failure_line_carries_the_diagnostics() {
        let error = CloudError::Status {
            engine: Engine::Stepfun,
            status: 429,
        };
        let line = failure_line(Engine::Stepfun, 2, 4.2, Instant::now(), &error);
        let expected_start = "cloud_error engine=stepfun attempt=2/3 recording=4.2s took=0.0s";
        assert!(line.starts_with(expected_start));
        assert!(line.contains("transient=yes code=HTTP429 error=StepFun request failed (429)."));
    }

    #[test]
    fn codes_name_the_failure() {
        assert_eq!(code(&status(504)), "HTTP504");
        assert_eq!(code(&assembly("upload", Some(401))), "HTTP401");
        assert_eq!(code(&assembly("upload", None)), "UnreadableReply");
        assert_eq!(code(&network()), "NetworkInterrupted");
        assert_eq!(code(&CloudError::MissingKey(Engine::Mai)), "MissingKey");
    }
}
