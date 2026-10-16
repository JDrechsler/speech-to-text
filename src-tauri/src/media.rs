use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const COMMAND_PLAY: &str = "0";
const COMMAND_PAUSE: &str = "1";
const ADAPTER_TIMEOUT: Duration = Duration::from_secs(3);
const SCRIPT: &str = "mediaremote-adapter.pl";
const FRAMEWORK: &str = "MediaRemoteAdapter.framework";

#[derive(Default)]
pub struct Media {
    adapter_dir: Option<PathBuf>,
    was_playing: AtomicBool,
}

impl Media {
    pub fn new(resource_dir: Option<PathBuf>) -> Media {
        let adapter_dir = resource_dir
            .map(|dir| dir.join("mediaremote-adapter"))
            .filter(|dir| dir.join(SCRIPT).exists() && dir.join(FRAMEWORK).exists());
        Media { adapter_dir, was_playing: AtomicBool::new(false) }
    }
}

pub fn pause(media: &Media) -> bool {
    media.was_playing.store(false, Ordering::SeqCst);
    let Some(dir) = media.adapter_dir.as_deref() else {
        return false;
    };
    let playing = run_adapter(dir, &["get", "--no-artwork"])
        .as_deref()
        .is_some_and(reports_playing);
    media.was_playing.store(playing, Ordering::SeqCst);
    run_adapter(dir, &["send", COMMAND_PAUSE]);
    playing
}

pub fn resume(media: &Media) {
    if !media.was_playing.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(dir) = media.adapter_dir.clone() {
        std::thread::spawn(move || run_adapter(&dir, &["send", COMMAND_PLAY]));
    }
}

fn reports_playing(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|info| info.get("playing")?.as_bool())
        .unwrap_or(false)
}

fn run_adapter(dir: &Path, arguments: &[&str]) -> Option<String> {
    let mut child = Command::new("/usr/bin/perl")
        .arg(dir.join(SCRIPT))
        .arg(dir.join(FRAMEWORK))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < ADAPTER_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_playing_flag_from_adapter_output() {
        assert!(reports_playing(r#"{"playing":true,"title":"Song"}"#));
        assert!(!reports_playing(r#"{"playing":false}"#));
        assert!(!reports_playing("null"));
        assert!(!reports_playing(""));
    }

    #[test]
    fn without_an_adapter_nothing_is_paused_or_resumed() {
        let media = Media::new(None);
        assert!(!pause(&media));
        resume(&media);
    }
}
