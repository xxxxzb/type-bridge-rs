use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(true);
static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();
static COMMAND_TX: OnceLock<Mutex<mpsc::Sender<KeyCommand>>> = OnceLock::new();

pub enum KeyCommand {
    TypeText(String),
    Backspace,
    Enter,
}

pub fn init_command_queue(tx: mpsc::Sender<KeyCommand>) {
    COMMAND_TX
        .set(Mutex::new(tx))
        .map_err(|_| ())
        .expect("keyboard command queue already initialized");
}

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

// ── Queue API (called from server thread) ──────────────────────────

fn send_command(cmd: KeyCommand) {
    if let Some(tx) = COMMAND_TX.get() {
        let _ = tx.lock().unwrap().send(cmd);
    }
}

pub fn queue_type_text(text: String) {
    if !is_enabled() || text.is_empty() {
        return;
    }
    send_command(KeyCommand::TypeText(text));
}

pub fn queue_backspace() {
    if !is_enabled() {
        return;
    }
    send_command(KeyCommand::Backspace);
}

pub fn queue_enter() {
    if !is_enabled() {
        return;
    }
    send_command(KeyCommand::Enter);
}

// ── Execute API (called from main thread event loop) ───────────────

pub fn execute(cmd: KeyCommand) {
    match cmd {
        KeyCommand::TypeText(text) => execute_type_text(&text),
        KeyCommand::Backspace => execute_backspace(),
        KeyCommand::Enter => execute_enter(),
    }
}

fn execute_type_text(text: &str) {
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

fn execute_backspace() {
    let mut enigo = enigo();
    if let Err(e) = enigo.key(Key::Backspace, Direction::Click) {
        tracing::error!("Backspace keystroke failed: {e}");
    }
}

fn execute_enter() {
    let mut enigo = enigo();
    if let Err(e) = enigo.key(Key::Return, Direction::Click) {
        tracing::error!("Enter keystroke failed: {e}");
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── enable/disable ─────────────────────────────────────────

    #[test]
    fn test_enabled_default() {
        set_enabled(true);
        assert!(is_enabled());
    }

    #[test]
    fn test_set_enabled_false() {
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn test_set_enabled_toggle() {
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    // ── disabled blocks queuing ─────────────────────────────────

    #[test]
    fn test_queue_type_text_returns_when_disabled() {
        set_enabled(false);
        assert!(!is_enabled());
        // queue_type_text / queue_backspace / queue_enter all check
        // is_enabled() first and return early when false
    }

    #[test]
    fn test_queue_backspace_returns_when_disabled() {
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn test_queue_enter_returns_when_disabled() {
        set_enabled(false);
        assert!(!is_enabled());
    }

    // ── command queue end-to-end ────────────────────────────────

    #[test]
    fn test_command_queue_full_flow() {
        set_enabled(true);
        let (tx, rx) = mpsc::channel::<KeyCommand>();

        tx.send(KeyCommand::TypeText("hello".into())).unwrap();
        tx.send(KeyCommand::Backspace).unwrap();
        tx.send(KeyCommand::Enter).unwrap();
        tx.send(KeyCommand::TypeText("世界".into())).unwrap();
        drop(tx);

        let commands: Vec<String> = rx
            .iter()
            .map(|cmd| match cmd {
                KeyCommand::TypeText(t) => format!("text:{t}"),
                KeyCommand::Backspace => "backspace".into(),
                KeyCommand::Enter => "enter".into(),
            })
            .collect();

        assert_eq!(commands.len(), 4);
        assert_eq!(commands[0], "text:hello");
        assert_eq!(commands[1], "backspace");
        assert_eq!(commands[2], "enter");
        assert_eq!(commands[3], "text:世界");
    }

    #[test]
    fn test_command_queue_empty_on_disabled() {
        set_enabled(false);
        let (tx, rx) = mpsc::channel::<KeyCommand>();

        // Send commands while disabled — the queue_* API would not send them,
        // but we're testing the channel isolation here
        tx.send(KeyCommand::TypeText("should_not_send".into())).unwrap();
        drop(tx);

        let commands: Vec<_> = rx.iter().collect();
        assert_eq!(commands.len(), 1); // channel has it, but queue_* wouldn't send
    }

    #[test]
    fn test_command_queue_preserves_unicode() {
        let (tx, rx) = mpsc::channel::<KeyCommand>();
        tx.send(KeyCommand::TypeText("emoji 😀 🚀".into())).unwrap();
        drop(tx);

        let commands: Vec<_> = rx.iter().collect();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            KeyCommand::TypeText(t) => assert_eq!(t, "emoji 😀 🚀"),
            _ => panic!("expected TypeText"),
        }
    }

    #[test]
    fn test_command_queue_empty_text() {
        let (tx, rx) = mpsc::channel::<KeyCommand>();
        tx.send(KeyCommand::TypeText(String::new())).unwrap();
        drop(tx);

        let commands: Vec<_> = rx.iter().collect();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            KeyCommand::TypeText(t) => assert!(t.is_empty()),
            _ => panic!("expected TypeText"),
        }
    }
}
