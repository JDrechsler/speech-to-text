use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const OVERLAY: &str = "overlay";
pub const SETTINGS: &str = "settings";
pub const HISTORY: &str = "history";

const OVERLAY_WIDTH: f64 = 560.0;
const OVERLAY_HEIGHT: f64 = 106.0;
const OVERLAY_BOTTOM_MARGIN: f64 = 80.0;

pub fn create_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("overlay".into()))
        .title("Speech to Text")
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    #[cfg(target_os = "macos")]
    drag_on_click_without_activating(&window);
    Ok(window)
}

pub fn show_overlay(app: &AppHandle) {
    let Some(window) = app.get_webview_window(OVERLAY) else {
        return;
    };
    place_at_bottom_of_active_screen(app, &window);
    #[cfg(target_os = "macos")]
    {
        let overlay = window.clone();
        let _ = app.run_on_main_thread(move || order_front_without_focus(&overlay));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window.show();
}

pub fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY) {
        let _ = window.hide();
    }
}

fn place_at_bottom_of_active_screen(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|cursor| app.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let width = OVERLAY_WIDTH * scale;
    let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
    let y = area.position.y as f64 + area.size.height as f64
        - (OVERLAY_HEIGHT + OVERLAY_BOTTOM_MARGIN) * scale;
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

#[cfg(target_os = "macos")]
fn drag_on_click_without_activating(window: &WebviewWindow) {
    use objc2::runtime::{Bool, NSObjectProtocol};
    use objc2::{msg_send, sel};
    use objc2_app_kit::NSWindow;
    let Ok(pointer) = window.ns_window() else {
        return;
    };
    let ns_window = unsafe { &*(pointer as *const NSWindow) };
    ns_window.setMovableByWindowBackground(true);
    if ns_window.respondsToSelector(sel!(_setPreventsActivation:)) {
        let _: () = unsafe { msg_send![ns_window, _setPreventsActivation: Bool::YES] };
    }
}

#[cfg(target_os = "macos")]
fn order_front_without_focus(window: &WebviewWindow) {
    use objc2_app_kit::NSWindow;
    if let Ok(pointer) = window.ns_window() {
        let ns_window = unsafe { &*(pointer as *const NSWindow) };
        ns_window.orderFrontRegardless();
    }
}

pub fn open_settings(app: &AppHandle) {
    open(app, SETTINGS, "Speech to Text Settings", (640.0, 720.0));
}

pub fn open_history(app: &AppHandle) {
    open(app, HISTORY, "Recording History", (720.0, 480.0));
}

fn open(app: &AppHandle, label: &str, title: &str, size: (f64, f64)) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App(label.into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(520.0, 360.0)
        .center()
        .focused(true)
        .build();
    if let Ok(window) = built {
        let _ = window.set_focus();
    }
}
