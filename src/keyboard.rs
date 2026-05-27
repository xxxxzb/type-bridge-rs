use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(true);
static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();

fn enigo() -> std::sync::MutexGuard<'static, Enigo> {
    ENIGO
        .get_or_init(|| {
            Mutex::new(Enigo::new(&Settings::default()).expect("Failed to initialize enigo"))
        })
        .lock()
        .expect("enigo mutex poisoned")
}

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub fn type_text(text: &str) {
    if !is_enabled() || text.is_empty() {
        return;
    }

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to open clipboard: {e}");
            return;
        }
    };

    let previous = clipboard.get_text().ok();

    if let Err(e) = clipboard.set_text(text) {
        tracing::error!("Failed to set clipboard text: {e}");
        return;
    }

    std::thread::sleep(Duration::from_millis(30));

    let mut enigo = enigo();

    #[cfg(target_os = "macos")]
    let mod_key = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let mod_key = Key::Control;

    if let Err(e) = enigo.key(mod_key, Direction::Press) {
        tracing::error!("Failed to press modifier key: {e}");
    }
    if let Err(e) = enigo.key(Key::Unicode('v'), Direction::Click) {
        tracing::error!("Failed to press paste key: {e}");
    }
    if let Err(e) = enigo.key(mod_key, Direction::Release) {
        tracing::error!("Failed to release modifier key: {e}");
    }

    std::thread::sleep(Duration::from_millis(50));

    if let Some(prev) = previous {
        if let Err(e) = clipboard.set_text(&prev) {
            tracing::warn!("Failed to restore clipboard: {e}");
        }
    }
}

pub fn press_backspace() {
    if !is_enabled() {
        return;
    }
    let mut enigo = enigo();
    if let Err(e) = enigo.key(Key::Backspace, Direction::Click) {
        tracing::error!("Backspace keystroke failed: {e}");
    }
}

pub fn press_enter() {
    if !is_enabled() {
        return;
    }
    let mut enigo = enigo();
    if let Err(e) = enigo.key(Key::Return, Direction::Click) {
        tracing::error!("Enter keystroke failed: {e}");
    }
}
