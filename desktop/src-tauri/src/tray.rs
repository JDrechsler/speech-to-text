use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::audio;
use crate::dictionary;
use crate::engines::{Engine, THIS_DEVICE};
use crate::paste;
use crate::paths;
use crate::session;
use crate::settings::Settings;
use crate::state::{self, Phase, Shared};
use crate::windows;

const TRAY_ID: &str = "main";
const IDLE_ICON: &[u8] = include_bytes!("../icons/tray-idle.png");
const RECORDING_ICON: &[u8] = include_bytes!("../icons/tray-recording.png");
const ENGINE_PREFIX: &str = "engine:";
const MICROPHONE_PREFIX: &str = "mic:";
const BUILT_IN: &str = "builtin";

#[cfg(target_os = "macos")]
const SHORTCUT_LABEL: &str = "⌥Space";
#[cfg(not(target_os = "macos"))]
const SHORTCUT_LABEL: &str = "Alt+Space";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .icon_as_template(true)
        .tooltip("Speech to Text")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Enter { .. }) {
                rebuild_menu(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let recording = state::shared(app).phase() == Phase::Recording;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let icon = if recording { RECORDING_ICON } else { IDLE_ICON };
        if let Ok(image) = Image::from_bytes(icon) {
            let _ = tray.set_icon(Some(image));
            let _ = tray.set_icon_as_template(!recording);
        }
    }
    rebuild_menu(app);
}

fn rebuild_menu(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn toggle_title(phase: Phase) -> String {
    match phase {
        Phase::Idle => format!("Start Recording ({SHORTCUT_LABEL})"),
        Phase::Recording => format!("Stop Recording ({SHORTCUT_LABEL})"),
        Phase::Finishing => "Finishing…".into(),
    }
}

fn item(app: &AppHandle, id: &str, title: &str) -> tauri::Result<MenuItem<Wry>> {
    MenuItem::with_id(app, id, title, true, None::<&str>)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = state::shared(app);
    let settings = state.settings();
    let phase = state.phase();
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "toggle", toggle_title(phase), phase != Phase::Finishing, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(app, "history", "History…")?)?;
    menu.append(&item(app, "recordings", "Open Recordings Folder")?)?;
    menu.append(&item(app, "dictionary", "Edit Dictionary…")?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&engine_menu(app, &state)?)?;
    menu.append(&microphone_menu(app, &settings)?)?;
    if cfg!(target_os = "macos") {
        menu.append(&CheckMenuItem::with_id(
            app,
            "pause-media",
            "Pause Media While Recording",
            true,
            settings.pause_media,
            None::<&str>,
        )?)?;
    }
    if !paste::accessibility::is_trusted(false) {
        menu.append(&item(app, "accessibility", "Grant Accessibility (needed for auto-paste)…")?)?;
    }
    let launches_at_login = app.autolaunch().is_enabled().unwrap_or(false);
    menu.append(&CheckMenuItem::with_id(
        app,
        "launch-at-login",
        "Launch at Login",
        true,
        launches_at_login,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(app, "settings", "Settings…")?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(app, "quit", "Quit Speech to Text")?)?;
    Ok(menu)
}

fn engine_menu(app: &AppHandle, state: &Shared) -> tauri::Result<Submenu<Wry>> {
    let current = state.settings().engine;
    let place = if current.is_local() { format!("on {THIS_DEVICE}") } else { "cloud".into() };
    let submenu = Submenu::new(app, format!("Engine: {} ({place})", current.display_name()), true)?;
    let groups = [(format!("On {THIS_DEVICE} · private, offline"), true), ("Cloud · uploads your audio".into(), false)];
    for (header, local) in groups {
        submenu.append(&MenuItem::new(app, header, false, None::<&str>)?)?;
        for engine in Engine::ALL.into_iter().filter(|engine| engine.is_local() == local) {
            let ready = state::engine_setup_hint(engine, state).is_none();
            let title =
                if ready { engine.menu_title() } else { format!("{} (set up…)", engine.menu_title()) };
            let id = format!("{ENGINE_PREFIX}{}", engine.id());
            submenu.append(&CheckMenuItem::with_id(app, id, title, true, engine == current, None::<&str>)?)?;
        }
    }
    Ok(submenu)
}

fn microphone_menu(app: &AppHandle, settings: &Settings) -> tauri::Result<Submenu<Wry>> {
    let default_preference = if cfg!(target_os = "macos") { BUILT_IN } else { audio::SYSTEM_DEFAULT };
    let wanted = settings.microphone.clone().unwrap_or_else(|| default_preference.into());
    let system_default = audio::default_device_name();
    let in_use = audio::resolved_device_name(Some(&wanted)).or_else(|| system_default.clone());
    let submenu = Submenu::new(app, format!("Microphone: {}", in_use.as_deref().unwrap_or("none")), true)?;

    let default_title = match &system_default {
        Some(name) => format!("System Default ({name})"),
        None => "System Default".into(),
    };
    let default_id = format!("{MICROPHONE_PREFIX}{}", audio::SYSTEM_DEFAULT);
    let is_default = wanted == audio::SYSTEM_DEFAULT;
    submenu.append(&CheckMenuItem::with_id(app, default_id, default_title, true, is_default, None::<&str>)?)?;
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    for device in audio::input_devices() {
        let preferred_built_in = cfg!(target_os = "macos") && device.is_built_in;
        let (value, title) = if preferred_built_in {
            (BUILT_IN.to_string(), format!("{} (preferred)", device.name))
        } else {
            (device.name.clone(), device.name.clone())
        };
        let checked = wanted == value;
        let id = format!("{MICROPHONE_PREFIX}{value}");
        submenu.append(&CheckMenuItem::with_id(app, id, title, true, checked, None::<&str>)?)?;
    }
    if audio::unavailable_preference_notice(Some(&wanted)).is_some() {
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
        let notice = "Preferred microphone not connected, using system default";
        submenu.append(&MenuItem::new(app, notice, false, None::<&str>)?)?;
    }
    Ok(submenu)
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id.as_ref();
    let state = state::shared(app);
    if let Some(engine) = id.strip_prefix(ENGINE_PREFIX).and_then(Engine::from_id) {
        state::choose_engine(&state, engine);
        if state::engine_setup_hint(engine, &state).is_some() {
            windows::open_settings(app);
        }
    } else if let Some(microphone) = id.strip_prefix(MICROPHONE_PREFIX) {
        state.update_settings(|settings| settings.microphone = Some(microphone.to_string()));
    } else {
        match id {
            "toggle" => session::toggle(app),
            "history" => windows::open_history(app),
            "recordings" => {
                let _ = app.opener().open_path(paths::recordings_dir().to_string_lossy(), None::<&str>);
            }
            "dictionary" => {
                dictionary::ensure_file_exists();
                let _ = app.opener().open_path(paths::dictionary_file().to_string_lossy(), None::<&str>);
            }
            "pause-media" => {
                state.update_settings(|settings| settings.pause_media = !settings.pause_media);
            }
            "accessibility" => {
                paste::accessibility::is_trusted(true);
            }
            "launch-at-login" => {
                let launcher = app.autolaunch();
                let _ = if launcher.is_enabled().unwrap_or(false) { launcher.disable() } else { launcher.enable() };
            }
            "settings" => windows::open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        }
    }
    rebuild_menu(app);
}
