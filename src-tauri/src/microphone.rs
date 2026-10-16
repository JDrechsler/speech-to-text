pub const PRIVACY_SETTINGS_URL: Option<&str> = if cfg!(target_os = "macos") {
    Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
} else {
    None
};

const DENIED: &str = "Microphone access denied. Allow Speech to Text under Privacy & Security, Microphone.";

#[cfg(target_os = "macos")]
pub fn ensure_access() -> Result<(), String> {
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

    const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return Ok(());
    };
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
    if status == AVAuthorizationStatus::Authorized {
        return Ok(());
    }
    if status == AVAuthorizationStatus::NotDetermined {
        let (sender, receiver) = mpsc::channel();
        let handler = RcBlock::new(move |granted: Bool| {
            let _ = sender.send(granted.as_bool());
        });
        unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
        if receiver.recv_timeout(PROMPT_TIMEOUT).unwrap_or(false) {
            return Ok(());
        }
    }
    Err(DENIED.into())
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_access() -> Result<(), String> {
    let _ = DENIED;
    Ok(())
}
