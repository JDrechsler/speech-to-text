use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIdentity {
    pub id: String,
    pub name: String,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::AppIdentity;
    use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

    fn identity(app: &NSRunningApplication) -> Option<AppIdentity> {
        let id = app.bundleIdentifier()?.to_string();
        let name = app.localizedName().map(|name| name.to_string()).unwrap_or_else(|| id.clone());
        Some(AppIdentity { id, name })
    }

    pub fn frontmost() -> Option<AppIdentity> {
        let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        identity(&app)
    }

    pub fn running() -> Vec<AppIdentity> {
        NSWorkspace::sharedWorkspace()
            .runningApplications()
            .iter()
            .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
            .filter_map(|app| identity(&app))
            .collect()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::AppIdentity;
    use std::path::Path;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    use windows::core::PWSTR;

    fn executable_of(window: HWND) -> Option<String> {
        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
        if process_id == 0 {
            return None;
        }
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let queried = unsafe {
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length)
        };
        unsafe {
            let _ = CloseHandle(process);
        }
        queried.ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }

    pub fn frontmost() -> Option<AppIdentity> {
        let path = executable_of(unsafe { GetForegroundWindow() })?;
        let file = Path::new(&path);
        let id = file.file_name()?.to_string_lossy().to_lowercase();
        let name = file.file_stem()?.to_string_lossy().into_owned();
        Some(AppIdentity { id, name })
    }

    pub fn running() -> Vec<AppIdentity> {
        Vec::new()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::AppIdentity;

    pub fn frontmost() -> Option<AppIdentity> {
        None
    }

    pub fn running() -> Vec<AppIdentity> {
        Vec::new()
    }
}

pub fn frontmost() -> Option<AppIdentity> {
    platform::frontmost()
}

pub fn running_apps(own_id: &str) -> Vec<AppIdentity> {
    let mut apps: Vec<AppIdentity> = platform::running()
        .into_iter()
        .filter(|app| app.id != own_id)
        .collect();
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.id == b.id);
    apps
}

pub fn identity_of_app_file(path: &std::path::Path) -> Option<AppIdentity> {
    let name = path.file_stem()?.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    let id = plist::Value::from_file(path.join("Contents/Info.plist"))
        .ok()?
        .as_dictionary()?
        .get("CFBundleIdentifier")?
        .as_string()?
        .to_string();
    #[cfg(not(target_os = "macos"))]
    let id = path.file_name()?.to_string_lossy().to_lowercase();
    Some(AppIdentity { id, name })
}
