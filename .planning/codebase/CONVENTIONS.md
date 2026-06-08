# Coding Conventions

**Analysis Date:** 2026-06-08

## Naming Patterns

**Files:**
- `snake_case.rs` — Every source file matches this: `main.rs`, `server.rs`, `keyboard.rs`, `tray.rs`, `tray_icons.rs`, `ip.rs`, `assets.rs`
- Test modules are co-located: `#[cfg(test)] mod tests` at end of each file

**Functions:**
- `snake_case` for all functions, both public and private — `generate_token()`, `physical_scale()`, `get_local_ip()`, `build_tray()`, `make_icon()`, `queue_type_text()`, `merge_commands()`, `execute()`, `check_token()`, `urlencoding()`, `add_to_history()`
- Descriptive names carrying intent — `queue_type_text()` not `qtt()`, `merge_commands()` not `merge()`

**Variables:**
- `snake_case` for local variables — `shutdown_tx`, `kb_tx`, `tray_state`, `token_clone`, `last_click`
- Single uppercase letter `T` for generic type params

**Constants:**
- `SCREAMING_SNAKE_CASE` — `MAX_COMMANDS_PER_TICK`, `KB_CHANNEL_BOUND`, `QR_SCALE`, `QR_PAD`, `MAX_TEXT_LEN`, `HISTORY_MAX`, `CMD_TIMEOUT`, `E2E_TOKEN`, `TEST_TOKEN`

**Statics:**
- `SCREAMING_SNAKE_CASE` — `ENABLED`, `ENIGO`, `COMMAND_TX`, `TEST_STATE_LOCK`, `ICON_OFF`, `SOCKET_IO_JS`

**Types:**
- `PascalCase` for enums, structs, and type aliases — `KeyCommand`, `TrayEvent`, `TypeTextPayload`, `PressKeyPayload`, `QrWindow`, `Cli`, `TrayState`, `SyncProxy`, `TestGuard`
- Enum variants: `PascalCase` — `KeyCommand::TypeText(String)`, `KeyCommand::Backspace`, `KeyCommand::Enter`, `KeyCommand::SelectAll`

## Code Style

**Formatting:**
- Rust standard formatting (rustfmt defaults, no `.rustfmt.toml` found)
- Edition 2021
- 4-space indentation

**Linting:**
- Clippy used with selective `#[allow(...)]` attributes
- One explicit suppression in the codebase: `#[allow(clippy::needless_range_loop)]` in `src/tray.rs` test module
- No separate `clippy.toml` found

## Import Organization

**Order:**
1. External crate imports — `use axum::...`, `use tokio::...`, `use serde::...`, `use socketioxide::...`
2. Standard library imports — `use std::sync::...`, `use std::collections::...`
3. Crate-internal references — `use crate::tray_icons::...`, `crate::keyboard::...`
4. Platform-conditional imports via `#[cfg(target_os = "...")]`
5. Test-only imports behind `#[cfg(test)]`

**Path Aliases:**
- No path aliases configured; all imports use full crate paths

## Error Handling

**Patterns:**
```rust
// Preferred: unwrap_or_else with tracing error
.unwrap_or_else(|e| tracing::error!("Description: {e}"));

// Critical failures: expect
.expect("Descriptive message about what failed and why");

// Warnings for recoverable issues
tracing::warn!("Recoverable situation description");

// Guard-and-return for non-critical fallible ops
if let Err(e) = clipboard.set_text(text) {
    tracing::error!("Failed to set clipboard text: {e}");
    return;
}
```

**Error Handling Principles:**
- Use `.expect()` for initialization failures that should never happen in normal operation (e.g., `Icon::from_rgba`, `softbuffer::Context::new`, `EventLoop::build`)
- Use `.unwrap_or_else(|e| tracing::error!(...))` for operations where failure is acceptable and execution can continue (e.g., tray icon updates, menu appends)
- Use `Result` return types for fallible operations at function boundaries
- Discard errors with `let _ = ...` when the caller explicitly does not care about the result (e.g., `let _ = tx.send(())`, `let _ = c.set_text(url)`)
- Use `match` on `Err(e)` variants for operations needing cleanup or different response paths

## Logging

**Framework:** `tracing` crate (v0.1) + `tracing-subscriber` (v0.3)

**Patterns:**
```rust
tracing::info!("Descriptive message with {vars}");
tracing::warn!("Warning message: {e}");
tracing::error!("Error message: {e}");
```

- `tracing_subscriber::fmt::init()` called once in `main()` before any other operations
- Log messages are lowercase, no trailing punctuation
- Include relevant context in messages (e.g., `"Socket.IO connection rejected — invalid or missing token (id: {})"`)
- Server-side connection/disconnection events logged at `info` level
- Backpressure warnings at `warn` level

## Comments

**When to Comment:**
- Section comments use `// ── Section Name ──` visual separators (e.g., `// ── QR pixel generation ──`, `// ── Tests ──`)
- Safety invariants documented with `// SAFETY:` comments (e.g., `unsafe impl Sync for SyncProxy` has a safety comment)
- Architectural notes documented in block comments (e.g., E2E test module has a multi-line comment explaining the approach)
- `#[derive(Debug)]` types that appear in public APIs
- Magic numbers have inline comments explaining their provenance
- Test helper types have doc comments with usage examples

**No JSDoc/TSDoc equivalent:**
- Rust doc comments (`///`) used sparingly; only on `TrayState` struct and some test helpers
- Most functions rely on descriptive naming rather than doc comments

## Function Design

**Size:** Functions are small and focused. Average 15-30 lines. Largest is `main()` at ~160 lines.

**Parameters:**
- Prefer owned values over references where the function consumes data
- Use `&str` for borrowed string data
- Maximum 3-4 parameters; grouped into structs when more needed (e.g., `QrWindow`)

**Return Values:**
- Simple values returned directly (e.g., `String`, `bool`, `u32`)
- Complex results use enums or tuples (e.g., `(u32, u32, Vec<u32>)` for QR pixel data)
- Fallible operations return nothing (`()`) with errors logged internally rather than propagated

## Module Design

**Exports:** Minimal public API per module — most functions are private, only exposing what other modules need:
- `keyboard.rs`: `KeyCommand`, `init_command_queue`, `queue_type_text`, `queue_backspace`, `queue_enter`, `queue_select_all`, `merge_commands`, `execute`, `set_enabled`, `is_enabled`, `TestGuard` (test only)
- `server.rs`: `run`, `serve`
- `tray.rs`: `TrayState`, `make_icon`, `build_tray`
- `ip.rs`: `get_local_ip`
- `tray_icons.rs`: `ICON_ON`, `make_icon_off`
- `assets.rs`: `HTML`

**Barrel Files:** Not used. Each module is imported directly.

## Conditional Compilation

**Pattern:**
```rust
#[cfg(target_os = "macos")]
let mod_key = Key::Meta;
#[cfg(not(target_os = "macos"))]
let mod_key = Key::Control;
```

- Used for macOS/not-macOS key bindings (Cmd vs Ctrl, Alt vs Ctrl)
- Used for macOS-only activation policy via `EventLoopBuilderExtMacOS`
- Test code gated behind `#[cfg(test)]`

---

*Convention analysis: 2026-06-08*
