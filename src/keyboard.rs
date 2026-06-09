use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static ENABLED: AtomicBool = AtomicBool::new(true);
static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();

#[derive(Debug)]
pub enum KeyCommand {
    TypeText(String),
    Backspace,
    Enter,
    ClearPcField,
}

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

fn enigo() -> std::sync::MutexGuard<'static, Enigo> {
    ENIGO
        .get_or_init(|| {
            Mutex::new(Enigo::new(&Settings::default()).expect("Failed to initialize enigo"))
        })
        .lock()
        .expect("enigo mutex poisoned")
}

// ── Merging: consecutive TypeText are joined before execution ──────

/// Merge consecutive `TypeText` commands into one, respecting
/// Backspace/Enter/ClearPcField as unmergeable boundaries.
pub fn merge_commands(cmds: Vec<KeyCommand>) -> Vec<KeyCommand> {
    let mut out: Vec<KeyCommand> = Vec::with_capacity(cmds.len());
    for cmd in cmds {
        match cmd {
            KeyCommand::TypeText(t) => {
                if let Some(KeyCommand::TypeText(last)) = out.last_mut() {
                    last.push_str(&t);
                } else {
                    out.push(KeyCommand::TypeText(t));
                }
            }
            other => out.push(other),
        }
    }
    out
}

// ── Execute API (called from main thread event loop) ───────────────

pub fn execute(cmd: KeyCommand) {
    match cmd {
        KeyCommand::TypeText(text) => execute_type_text(&text),
        KeyCommand::Backspace => execute_backspace(),
        KeyCommand::Enter => execute_enter(),
        KeyCommand::ClearPcField => {
            execute_select_all();
            execute_backspace();
        }
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

fn execute_select_all() {
    let mut enigo = enigo();

    #[cfg(target_os = "macos")]
    let mod_key = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let mod_key = Key::Control;

    if let Err(e) = enigo.key(mod_key, Direction::Press) {
        tracing::error!("SelectAll: failed to press modifier: {e}");
    }
    if let Err(e) = enigo.key(Key::Unicode('a'), Direction::Click) {
        tracing::error!("SelectAll: failed to press 'a': {e}");
    }
    if let Err(e) = enigo.key(mod_key, Direction::Release) {
        tracing::error!("SelectAll: failed to release modifier: {e}");
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // ── enable/disable ─────────────────────────────────────────

    #[test]
    fn test_enabled_default() {
        ENABLED.store(true, Ordering::SeqCst);
        assert!(is_enabled());
    }

    #[test]
    #[serial_test::serial]
    fn test_set_enabled_false() {
        set_enabled(true);
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
    }

    #[test]
    #[serial_test::serial]
    fn test_set_enabled_toggle() {
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    // ── bounded channel backpressure ───────────────────────────

    #[test]
    fn test_sync_channel_rejects_when_full() {
        let (tx, rx) = mpsc::sync_channel::<KeyCommand>(2);

        assert!(tx.send(KeyCommand::TypeText("a".into())).is_ok());
        assert!(tx.send(KeyCommand::TypeText("b".into())).is_ok());
        assert!(tx.try_send(KeyCommand::TypeText("c".into())).is_err());

        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    // ── text merging ───────────────────────────────────────────

    #[test]
    fn test_merge_consecutive_text() {
        let cmds = vec![
            KeyCommand::TypeText("a".into()),
            KeyCommand::TypeText("b".into()),
        ];
        let merged = merge_commands(cmds);
        assert_eq!(merged.len(), 1);
        match &merged[0] {
            KeyCommand::TypeText(t) => assert_eq!(t, "ab"),
            _ => panic!("expected TypeText"),
        }
    }

    #[test]
    fn test_merge_respects_enter_boundary() {
        let cmds = vec![
            KeyCommand::TypeText("a".into()),
            KeyCommand::Enter,
            KeyCommand::TypeText("b".into()),
        ];
        let merged = merge_commands(cmds);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_respects_backspace_boundary() {
        let cmds = vec![
            KeyCommand::TypeText("x".into()),
            KeyCommand::Backspace,
            KeyCommand::TypeText("y".into()),
        ];
        let merged = merge_commands(cmds);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_respects_clear_pc_field_boundary() {
        let cmds = vec![
            KeyCommand::TypeText("before".into()),
            KeyCommand::ClearPcField,
            KeyCommand::TypeText("after".into()),
        ];
        let merged = merge_commands(cmds);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn test_merge_empty_input() {
        let merged = merge_commands(vec![]);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_single_command() {
        let merged = merge_commands(vec![KeyCommand::TypeText("only".into())]);
        assert_eq!(merged.len(), 1);
    }

    // ── command queue end-to-end ────────────────────────────────

    #[test]
    fn test_command_queue_preserves_unicode() {
        let (tx, rx) = mpsc::sync_channel::<KeyCommand>(8);
        tx.send(KeyCommand::TypeText("emoji 😀 🚀".into())).unwrap();
        drop(tx);

        let commands: Vec<_> = rx.iter().collect();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            KeyCommand::TypeText(t) => assert_eq!(t, "emoji 😀 🚀"),
            _ => panic!("expected TypeText"),
        }
    }
}
