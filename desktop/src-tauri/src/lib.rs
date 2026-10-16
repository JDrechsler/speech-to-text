mod audio;
#[cfg(target_os = "macos")]
mod audio_devices;
mod cloud;
mod commands;
mod dictionary;
mod engines;
mod frontmost;
mod history;
mod media;
mod microphone;
mod models;
mod paste;
mod paths;
mod profiles;
mod rewrite;
mod secrets;
mod session;
mod settings;
mod state;
mod tray;
mod whisper;
mod windows;

use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState};

const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);

fn toggle_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::Space)
}

fn cancel_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::Escape)
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    if *shortcut == toggle_shortcut() {
        session::toggle(app);
    } else if *shortcut == cancel_shortcut() {
        session::cancel(app);
    }
}

fn on_command_line(app: &AppHandle, args: &[String]) {
    if args.iter().any(|arg| arg == "--toggle") {
        session::toggle(app);
    } else if args.iter().any(|arg| arg == "--cancel") {
        session::cancel(app);
    } else if args.iter().any(|arg| arg == "--history") {
        windows::open_history(app);
    } else {
        windows::open_settings(app);
    }
}

fn register_shortcuts(app: &AppHandle) {
    let shortcuts = app.global_shortcut();
    for shortcut in [toggle_shortcut(), cancel_shortcut()] {
        if let Err(error) = shortcuts.register(shortcut) {
            eprintln!("could not register {shortcut:?}: {error}");
        }
    }
}

fn start_hourly_cleanup() {
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(CLEANUP_INTERVAL).await;
            history::cleanup_expired();
        }
    });
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| on_command_line(app, &args)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(on_shortcut).build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            dictionary::ensure_file_exists();
            history::cleanup_expired();
            app.manage(state::AppState::new(media::Media::new(app.path().resource_dir().ok())));
            tray::create(app.handle())?;
            windows::create_overlay(app.handle())?;
            register_shortcuts(app.handle());
            start_hourly_cleanup();
            let state = state::shared(app.handle());
            if state::engine_setup_hint(state.settings().engine, &state).is_some() {
                windows::open_settings(app.handle());
            }
            Ok(())
        })
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())
        .expect("error while building Speech to Text");

    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
        RunEvent::Exit => state::shared(app).whisper.unload(),
        _ => {}
    });
}
