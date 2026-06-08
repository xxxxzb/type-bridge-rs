use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(true);
static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();
static COMMAND_TX: Mutex<Option<mpsc::SyncSender<KeyCommand>>> = Mutex::new(None);

pub enum KeyCommand {
    TypeText(String),
    Backspace,
    Enter,
    SelectAll,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CommandResult {
    Queued,
    Paused,
    Full,
    TooLong,
}

const MAX_TEXT_LEN: usize = 10_000;

pub fn init_command_queue(tx: mpsc::SyncSender<KeyCommand>) {
    *COMMAND_TX
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(tx);
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

fn send_command(cmd: KeyCommand) -> CommandResult {
    let guard = COMMAND_TX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    match guard.as_ref() {
        Some(tx) => match tx.try_send(cmd) {
            Ok(_) => CommandResult::Queued,
            Err(mpsc::TrySendError::Full(_)) => CommandResult::Full,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                tracing::warn!("Command channel disconnected");
                CommandResult::Full
            }
        },
        None => CommandResult::Full,
    }
}

pub fn queue_type_text(text: String) -> CommandResult {
    if !is_enabled() {
        return CommandResult::Paused;
    }
    if text.len() > MAX_TEXT_LEN {
        tracing::warn!("TypeText too long ({} chars), max {MAX_TEXT_LEN}", text.len());
        return CommandResult::TooLong;
    }
    send_command(KeyCommand::TypeText(text))
}

pub fn queue_backspace() -> CommandResult {
    if !is_enabled() {
        return CommandResult::Paused;
    }
    send_command(KeyCommand::Backspace)
}

pub fn queue_enter() -> CommandResult {
    if !is_enabled() {
        return CommandResult::Paused;
    }
    send_command(KeyCommand::Enter)
}

pub fn queue_select_all() -> CommandResult {
    if !is_enabled() {
        return CommandResult::Paused;
    }
    send_command(KeyCommand::SelectAll)
}

// ── Execute API (called from main thread event loop) ───────────────

pub fn execute(cmd: KeyCommand) {
    match cmd {
        KeyCommand::TypeText(text) => execute_type_text(&text),
        KeyCommand::Backspace => execute_backspace(),
        KeyCommand::Enter => execute_enter(),
        KeyCommand::SelectAll => tracing::warn!("SelectAll not yet implemented in execute"),
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
    use serial_test::serial;

    // ── enable/disable ─────────────────────────────────────────

    #[test]
    #[serial]
    fn test_enabled_default() {
        ENABLED.store(true, Ordering::SeqCst);
        assert!(is_enabled());
    }

    #[test]
    #[serial]
    fn test_set_enabled_false() {
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    #[serial]
    fn test_set_enabled_toggle() {
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    // ── disabled returns Paused ─────────────────────────────────

    #[test]
    #[serial]
    fn test_queue_type_text_returns_paused_when_disabled() {
        set_enabled(false);
        assert_eq!(queue_type_text("hello".into()), CommandResult::Paused);
    }

    #[test]
    #[serial]
    fn test_queue_backspace_returns_paused_when_disabled() {
        set_enabled(false);
        assert_eq!(queue_backspace(), CommandResult::Paused);
    }

    #[test]
    #[serial]
    fn test_queue_enter_returns_paused_when_disabled() {
        set_enabled(false);
        assert_eq!(queue_enter(), CommandResult::Paused);
    }

    // ── overlong text returns TooLong ──────────────────────────

    #[test]
    #[serial]
    fn test_queue_type_text_returns_too_long() {
        set_enabled(true);
        let long = "x".repeat(MAX_TEXT_LEN + 1);
        assert_eq!(queue_type_text(long), CommandResult::TooLong);
    }

    // ── full channel returns Full ──────────────────────────────

    #[test]
    #[serial]
    fn test_queue_returns_full_when_channel_full() {
        set_enabled(true);
        let (test_tx, _test_rx) = mpsc::sync_channel::<KeyCommand>(1);
        *COMMAND_TX.lock().unwrap() = Some(test_tx);
        assert_eq!(queue_type_text("first".into()), CommandResult::Queued);
        assert_eq!(queue_type_text("second".into()), CommandResult::Full);
    }

    // ── queued on success ──────────────────────────────────────

    #[test]
    #[serial]
    fn test_queue_type_text_returns_queued() {
        set_enabled(true);
        let (test_tx, _test_rx) = mpsc::sync_channel::<KeyCommand>(8);
        *COMMAND_TX.lock().unwrap() = Some(test_tx);
        assert_eq!(queue_type_text("hello".into()), CommandResult::Queued);
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
                KeyCommand::SelectAll => unreachable!(),
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
