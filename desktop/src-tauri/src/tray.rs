use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::MenuEvent;
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tray_icon::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
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

enum Entry {
    Item { id: &'static str, title: String, enabled: bool },
    Check { id: String, title: String, checked: bool },
    Header(String),
    Note(String),
    Separator,
    Submenu { title: String, entries: Vec<Entry> },
}

impl Entry {
    fn item(id: &'static str, title: impl Into<String>) -> Entry {
        Entry::Item { id, title: title.into(), enabled: true }
    }

    fn check(id: impl Into<String>, title: impl Into<String>, checked: bool) -> Entry {
        Entry::Check { id: id.into(), title: title.into(), checked }
    }

    fn rendered(&self) -> Box<dyn IsMenuItem> {
        match self {
            Entry::Item { id, title, enabled } => Box::new(MenuItem::with_id(*id, title, *enabled, None)),
            Entry::Check { id, title, checked } => {
                Box::new(CheckMenuItem::with_id(id.as_str(), title, true, *checked, None))
            }
            Entry::Header(title) => Box::new(PredefinedMenuItem::section_header(title)),
            Entry::Note(title) => Box::new(MenuItem::new(title, false, None)),
            Entry::Separator => Box::new(PredefinedMenuItem::separator()),
            Entry::Submenu { title, entries } => {
                let submenu = Submenu::new(title, true);
                for entry in entries {
                    let _ = submenu.append(entry.rendered().as_ref());
                }
                Box::new(submenu)
            }
        }
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(IDLE_ICON)?)
        .icon_as_template(true)
        .tooltip("Speech to Text")
        .show_menu_on_left_click(true)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Enter { .. }) {
                rebuild_menu(tray.app_handle());
            }
        })
        .build(app)?;
    rebuild_menu(app);
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
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let entries = menu_entries(app);
    let _ = tray.with_inner_tray_icon(move |inner| {
        let menu = Menu::new();
        for entry in &entries {
            let _ = menu.append(entry.rendered().as_ref());
        }
        inner.set_menu(Some(Box::new(menu)));
    });
}

fn toggle_title(phase: Phase) -> String {
    match phase {
        Phase::Idle => format!("Start Recording ({SHORTCUT_LABEL})"),
        Phase::Recording => format!("Stop Recording ({SHORTCUT_LABEL})"),
        Phase::Finishing => "Finishing…".into(),
    }
}

fn menu_entries(app: &AppHandle) -> Vec<Entry> {
    let state = state::shared(app);
    let settings = state.settings();
    let phase = state.phase();
    let mut entries = vec![
        Entry::Item { id: "toggle", title: toggle_title(phase), enabled: phase != Phase::Finishing },
        Entry::Separator,
        Entry::item("history", "History…"),
        Entry::item("recordings", "Open Recordings Folder"),
        Entry::item("dictionary", "Edit Dictionary…"),
        Entry::Separator,
        engine_menu(&state),
        microphone_menu(&settings),
    ];
    if cfg!(target_os = "macos") {
        entries.push(Entry::check("pause-media", "Pause Media While Recording", settings.pause_media));
    }
    if !paste::accessibility::is_trusted(false) {
        entries.push(Entry::item("accessibility", "Grant Accessibility (needed for auto-paste)…"));
    }
    let launches_at_login = app.autolaunch().is_enabled().unwrap_or(false);
    entries.extend([
        Entry::check("launch-at-login", "Launch at Login", launches_at_login),
        Entry::Separator,
        Entry::item("settings", "Settings…"),
        Entry::Separator,
        Entry::item("quit", "Quit Speech to Text"),
    ]);
    entries
}

fn engine_menu(state: &Shared) -> Entry {
    let current = state.settings().engine;
    let place = if current.is_local() { format!("on {THIS_DEVICE}") } else { "cloud".into() };
    let groups = [(format!("On {THIS_DEVICE} · private, offline"), true), ("Cloud · uploads your audio".into(), false)];
    let mut entries = Vec::new();
    for (header, local) in groups {
        entries.push(Entry::Header(header));
        for engine in Engine::ALL.into_iter().filter(|engine| engine.is_local() == local) {
            let ready = state::engine_setup_hint(engine, state).is_none();
            let title =
                if ready { engine.menu_title() } else { format!("{} (set up…)", engine.menu_title()) };
            entries.push(Entry::check(format!("{ENGINE_PREFIX}{}", engine.id()), title, engine == current));
        }
    }
    Entry::Submenu { title: format!("Engine: {} ({place})", current.display_name()), entries }
}

fn microphone_menu(settings: &Settings) -> Entry {
    let default_preference = if cfg!(target_os = "macos") { BUILT_IN } else { audio::SYSTEM_DEFAULT };
    let wanted = settings.microphone.clone().unwrap_or_else(|| default_preference.into());
    let system_default = audio::default_device_name();
    let in_use = audio::resolved_device_name(Some(&wanted)).or_else(|| system_default.clone());

    let default_title = match &system_default {
        Some(name) => format!("System Default ({name})"),
        None => "System Default".into(),
    };
    let default_id = format!("{MICROPHONE_PREFIX}{}", audio::SYSTEM_DEFAULT);
    let mut entries = vec![Entry::check(default_id, default_title, wanted == audio::SYSTEM_DEFAULT), Entry::Separator];
    for device in audio::input_devices() {
        let preferred_built_in = cfg!(target_os = "macos") && device.is_built_in;
        let (value, title) = if preferred_built_in {
            (BUILT_IN.to_string(), format!("{} (preferred)", device.name))
        } else {
            (device.name.clone(), device.name.clone())
        };
        let checked = wanted == value;
        entries.push(Entry::check(format!("{MICROPHONE_PREFIX}{value}"), title, checked));
    }
    if audio::unavailable_preference_notice(Some(&wanted)).is_some() {
        entries.push(Entry::Separator);
        entries.push(Entry::Note("Preferred microphone not connected, using system default".into()));
    }
    Entry::Submenu { title: format!("Microphone: {}", in_use.as_deref().unwrap_or("none")), entries }
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
