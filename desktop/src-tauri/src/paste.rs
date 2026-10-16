use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "macos")]
const PASTE_MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const PASTE_MODIFIER: Key = Key::Control;

#[cfg(target_os = "macos")]
const V_KEY: Key = Key::Other(9);
#[cfg(not(target_os = "macos"))]
const V_KEY: Key = Key::Unicode('v');

fn clipboard() -> &'static Mutex<Option<arboard::Clipboard>> {
    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
    CLIPBOARD.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()))
}

pub fn copy(text: &str) -> Result<(), String> {
    let mut guard = clipboard().lock().unwrap();
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }
    let board = guard.as_mut().ok_or("The clipboard is not available")?;
    board.set_text(text).map_err(|error| error.to_string())
}

pub fn paste_into_focused_app() -> Result<(), String> {
    if !accessibility::is_trusted(true) {
        return Err("Allow Accessibility for auto-paste. The text is on the clipboard.".into());
    }
    thread::sleep(Duration::from_millis(50));
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(PASTE_MODIFIER, Direction::Press)
        .and_then(|_| enigo.key(V_KEY, Direction::Click))
        .and_then(|_| enigo.key(PASTE_MODIFIER, Direction::Release))
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
pub mod accessibility {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    pub fn is_trusted(prompt: bool) -> bool {
        if !prompt {
            return unsafe { AXIsProcessTrusted() };
        }
        let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
    }
}

#[cfg(not(target_os = "macos"))]
pub mod accessibility {
    pub fn is_trusted(_prompt: bool) -> bool {
        true
    }
}
