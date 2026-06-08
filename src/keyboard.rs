use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

/// Maximum characters per single TypeText event.
const MAX_TEXT_LEN: usize = 10_000;

static ENABLED: AtomicBool = AtomicBool::new(true);
static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();
pub(crate) static COMMAND_TX: std::sync::Mutex<Option<mpsc::SyncSender<KeyCommand>>> =
    std::sync::Mutex::new(None);

#[derive(Debug)]
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

pub fn init_command_queue(tx: mpsc::SyncSender<KeyCommand>) {
    *COMMAND_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
}

/// Global test mutex — ensures only one `TestGuard` is alive at a time
/// so parallel tests cannot race on `ENABLED` or `COMMAND_TX`.
#[cfg(test)]
static TEST_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test-only guard: saves global `ENABLED` and (optionally) `COMMAND_TX` on
/// creation, restores both on drop — even after a panic.  Holding
/// `TEST_STATE_LOCK` guarantees exclusive access, so parallel tests that use
/// this guard are serialised with respect to the guarded globals.
///
/// ```ignore
/// let mut guard = TestGuard::new();
/// set_enabled(false);
/// guard.replace_command_tx(my_tx);
/// // … test body …
/// // guard drops here, releasing the lock and restoring state
/// ```
#[cfg(test)]
pub struct TestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    initial_enabled: bool,
    initial_tx: Option<mpsc::SyncSender<KeyCommand>>,
    replaced_tx: bool,
}

#[cfg(test)]
impl TestGuard {
    pub fn new() -> Self {
        let lock = TEST_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Self {
            _lock: lock,
            initial_enabled: ENABLED.load(Ordering::SeqCst),
            initial_tx: None,
            replaced_tx: false,
        }
    }

    /// Swap the global `COMMAND_TX` so tests can observe queued commands
    /// without hitting the real OS input path.  The original sender (if any)
    /// is restored automatically when the guard goes out of scope.
    pub fn replace_command_tx(&mut self, tx: mpsc::SyncSender<KeyCommand>) {
        let mut lock = COMMAND_TX.lock().unwrap_or_else(|e| e.into_inner());
        if !self.replaced_tx {
            self.initial_tx = lock.take();
            self.replaced_tx = true;
        }
        *lock = Some(tx);
    }
}

#[cfg(test)]
impl Drop for TestGuard {
    fn drop(&mut self) {
        ENABLED.store(self.initial_enabled, Ordering::SeqCst);
        if self.replaced_tx {
            let mut lock = COMMAND_TX.lock().unwrap_or_else(|e| e.into_inner());
            *lock = self.initial_tx.take();
        }
    }
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
    if text.is_empty() {
        return CommandResult::Paused;
    }
    if text.len() > MAX_TEXT_LEN {
        tracing::warn!(
            "TypeText too long ({} chars), max {MAX_TEXT_LEN}, rejecting",
            text.len()
        );
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

// ── Merging: consecutive TypeText are joined before execution ──────

/// Merge consecutive `TypeText` commands into one, respecting
/// Backspace/Enter as unmergeable boundaries.
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
            other => out.push(other), // Backspace, Enter, SelectAll — unmergeable
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
        KeyCommand::SelectAll => execute_select_all(),
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

    // ── enable/disable ─────────────────────────────────────────

    #[test]
    fn test_enabled_default() {
        let _guard = TestGuard::new();
        ENABLED.store(true, Ordering::SeqCst);
        assert!(is_enabled());
    }

    #[test]
    fn test_set_enabled_false() {
        let _guard = TestGuard::new();
        set_enabled(false);
        assert!(!is_enabled());
    }

    #[test]
    fn test_set_enabled_toggle() {
        let _guard = TestGuard::new();
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    // ── disabled returns Paused ─────────────────────────────────

    #[test]
    fn test_queue_type_text_returns_paused_when_disabled() {
        let _guard = TestGuard::new();
        set_enabled(false);
        assert_eq!(queue_type_text("hello".into()), CommandResult::Paused);
    }

    #[test]
    fn test_queue_backspace_returns_paused_when_disabled() {
        let _guard = TestGuard::new();
        set_enabled(false);
        assert_eq!(queue_backspace(), CommandResult::Paused);
    }

    #[test]
    fn test_queue_enter_returns_paused_when_disabled() {
        let _guard = TestGuard::new();
        set_enabled(false);
        assert_eq!(queue_enter(), CommandResult::Paused);
    }

    // ── overlong text returns TooLong ──────────────────────────

    #[test]
    fn test_queue_type_text_returns_too_long() {
        let _guard = TestGuard::new();
        set_enabled(true);
        let long = "x".repeat(MAX_TEXT_LEN + 1);
        assert_eq!(queue_type_text(long), CommandResult::TooLong);
    }

    // ── full channel returns Full ──────────────────────────────

    #[test]
    fn test_queue_returns_full_when_channel_full() {
        let mut guard = TestGuard::new();
        set_enabled(true);
        let (test_tx, _test_rx) = mpsc::sync_channel::<KeyCommand>(1);
        guard.replace_command_tx(test_tx);
        assert_eq!(queue_type_text("first".into()), CommandResult::Queued);
        assert_eq!(queue_type_text("second".into()), CommandResult::Full);
    }

    // ── queued on success ──────────────────────────────────────

    #[test]
    fn test_queue_type_text_returns_queued() {
        let mut guard = TestGuard::new();
        set_enabled(true);
        let (test_tx, _test_rx) = mpsc::sync_channel::<KeyCommand>(8);
        guard.replace_command_tx(test_tx);
        assert_eq!(queue_type_text("hello".into()), CommandResult::Queued);
    }

    // ── bounded channel backpressure (direct channel, no global) ──

    #[test]
    fn test_sync_channel_rejects_when_full() {
        let (tx, rx) = mpsc::sync_channel::<KeyCommand>(2);

        assert!(tx.send(KeyCommand::TypeText("a".into())).is_ok());
        assert!(tx.send(KeyCommand::TypeText("b".into())).is_ok());
        // Third send should block or fail on bounded channel
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
        match &merged[0] {
            KeyCommand::TypeText(t) => assert_eq!(t, "a"),
            _ => panic!("expected TypeText a"),
        }
        match &merged[2] {
            KeyCommand::TypeText(t) => assert_eq!(t, "b"),
            _ => panic!("expected TypeText b"),
        }
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
    fn test_merge_respects_select_all_boundary() {
        let cmds = vec![
            KeyCommand::TypeText("before".into()),
            KeyCommand::SelectAll,
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
