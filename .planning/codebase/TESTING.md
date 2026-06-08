# Testing Patterns

**Analysis Date:** 2026-06-08

## Test Framework

**Runner:**
- Built-in Rust `#[test]` framework (standard library)
- No external test runner or harness configured

**Assertion Library:**
- Built-in Rust macros: `assert!()`, `assert_eq!()`, `assert_ne!()`
- Custom panic messages in match arms (e.g., `panic!("expected TypeText, got {other:?}")`)

**Async Runtime:**
- `#[tokio::test]` for async tests (tokio runtime)
- Tokio configured via `[dev-dependencies]` in `Cargo.toml` — only `tower = "0.5"` listed as a dev-dep

**Run Commands:**
```bash
cargo test                    # Run all tests
cargo test -- --nocapture     # Show stdout/tracing output during tests
cargo test <test_name>        # Run specific test by name filter
cargo test e2e                # Run all E2E tests (matches "e2e" in name)
```

**Config:**
- No test config file (no `.cargo/config.toml` for test settings)
- No code coverage configuration found

## Test File Organization

**Location:**
- **Co-located** — Every test is inside `#[cfg(test)] mod tests { ... }` at the bottom of the source file it tests
- No separate `tests/` directory
- Files containing tests:
  - `src/main.rs` — Unit tests for token generation (lines 402-439)
  - `src/keyboard.rs` — Unit tests for keyboard commands, merging, queuing (lines 277-456)
  - `src/server.rs` — Unit + integration + E2E tests (lines 229-878)
  - `src/tray.rs` — Unit tests for QR generation and icons (lines 81-278)
  - `src/ip.rs` — Unit tests for local IP detection (lines 21-49)
  - `src/assets.rs` — Unit tests for HTML content assertions (lines 394-485)

**Naming:**
```rust
#[test]
fn test_<feature>_<condition>() { ... }
```

**Structure:**
```
src/
├── main.rs          # tests at EOF (line 402)
├── keyboard.rs      # tests at EOF (line 277)
├── server.rs        # unit tests (line 229) + e2e_tests module (line 579)
├── tray.rs          # tests at EOF (line 81)
├── ip.rs            # tests at EOF (line 21)
└── assets.rs        # tests at EOF (line 394)
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ── section comment headers ──

    #[test]
    fn test_...() {
        // arrange
        // act
        // assert
    }
}
```

**Patterns:**
- **Section comments** group related tests (e.g., `// ── enable/disable ──`, `// ── text merging ──`, `// ── check_token unit tests ──`)
- **TestGuard** for global state isolation (detailed below)
- E2E tests in a separate module: `#[cfg(test)] mod e2e_tests { ... }` within `src/server.rs`

## Mocking

**Framework:** None — no mocking crate used. Tests use real objects and manual channel interception.

**Patterns:**

**1. Global State Guard (`src/keyboard.rs`):**
```rust
// TestGuard saves and restores ENABLED and COMMAND_TX on drop
// Uses TEST_STATE_LOCK to serialize parallel tests touching globals
#[cfg(test)]
pub struct TestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    initial_enabled: bool,
    initial_tx: Option<mpsc::SyncSender<KeyCommand>>,
    replaced_tx: bool,
}
```
Usage:
```rust
let mut guard = TestGuard::new();
crate::keyboard::set_enabled(true);
let (test_tx, test_rx) = mpsc::sync_channel::<KeyCommand>(16);
guard.replace_command_tx(test_tx);
// ... test body ...
// guard drops, restoring original state
```

**2. Channel-based Command Interception:**
- Tests replace the global `COMMAND_TX` with a test channel to observe queued commands without hitting the real OS input path
- `recv_cmd()` helper polls channel with timeout in E2E tests

**3. Direct Router Testing (HTTP Integration):**
```rust
fn test_router() -> Router {
    build_router(TEST_TOKEN).0
}
```
- Tests call `app.oneshot(request)` directly via `tower::ServiceExt` — no network needed
- `uriencoding()` tested as pure function

**What to Mock:**
- Global mutable state (`ENABLED`, `COMMAND_TX`)
- OS-level side effects (clipboard, keyboard input) — intercepted via channel replacement

**What NOT to Mock:**
- Pure functions tested directly (token generation, text merging, URL decoding, history dedup)
- Data structures tested with real instances (VecDeque, channels, enums)
- HTML content verified as string matching on the compiled-in `HTML` constant

## Fixtures and Factories

**Test Data:**
- Constants at module level within test block:
  ```rust
  const TEST_TOKEN: &str = "abc123";
  const E2E_TOKEN: &str = "e2e-test-token-42";
  ```
- Inline data constructed per test:
  ```rust
  let cmds = vec![
      KeyCommand::TypeText("a".into()),
      KeyCommand::TypeText("b".into()),
  ];
  ```

**Helper Functions:**
- `test_router()` — Returns a `Router` instance for HTTP integration tests
- `spawn_server()` — Starts full server on pre-bound `127.0.0.1:0` for E2E tests
- `wait_for_server()` — Polls TCP until server is reachable
- `http_get()` / `http_post()` — Raw HTTP requests via TCP stream
- `parse_http()` — Minimal HTTP response parser
- `eio_handshake()` — Engine.IO v4 handshake
- `sio_connect()` — Socket.IO CONNECT
- `sio_emit()` — Socket.IO event emission
- `recv_cmd()` — Poll test channel with timeout
- `qr_lines()` — Generates QR code as Unicode art lines
- `qr_to_modules()` — Converts QR lines to boolean grid
- `assert_finder()` — Asserts QR finder pattern structure

## Coverage

**Requirements:** Not enforced — no coverage tool or threshold configured.

**View Coverage:**
```bash
cargo tarpaulin                 # If installed
cargo llvm-cov                  # If installed
```

**Observed Coverage:**
- Pure functions have thorough unit tests (token generation: 4 tests, URL encoding: 1 test, text merging: 7 tests, history dedup: 5 tests)
- HTTP auth has comprehensive integration tests (6 tests covering token valid/invalid/encoded/sid-bypass)
- E2E tests cover the full command flow but are limited to basic scenarios
- No tests for the QR GUI rendering path or the clipboard clipboard-path (no enigo usage in tests)
- `ip.rs` tests verify non-empty IP and correct format
- `tray.rs` tests cover QR logic and icon generation but not the OS tray API

## Test Types

**Unit Tests:**
- Scope: Pure functions, data structure operations, enum matching logic
- No side effects, no async
- Examples: token generation, text merging, history dedup, URL decoding, HTML string assertions, channel backpressure

**Integration Tests:**
- Scope: HTTP routing, auth middleware, token validation
- Use `build_router()` + `tower::ServiceExt::oneshot()` — no network or real server
- Test HTTP status codes, response bodies, content-type headers
- Configured with `TEST_TOKEN` constant

**E2E Tests:**
- Scope: Full server lifecycle with Socket.IO protocol over real TCP
- Use pre-bound `127.0.0.1:0` listener (no TOCTOU race on port allocation)
- Implement raw Engine.IO v4 polling protocol manually — no external client crate
- Tests: auth rejection, handshake, namespace connect, event emit (type_text, backspace, press_key), history flow
- Keyboard state isolated with `TestGuard` per E2E test

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn test_index_rejected_without_token() {
    let app = test_router();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}
```

**Error Testing:**
```rust
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
```

**Token Uniqueness:**
```rust
#[test]
fn test_generate_token_unique() {
    let a = generate_token();
    let b = generate_token();
    assert_ne!(a, b);
}
```

**Chained Assertions:**
```rust
// test_index_page_does_not_embed_token
let body_str = String::from_utf8(body.to_vec()).unwrap();
assert!(!body_str.contains(TEST_TOKEN));
```

**Negative Testing:**
```rust
#[test]
fn test_check_token_invalid() {
    assert!(!check_token("token=wrong", TEST_TOKEN));
    assert!(!check_token("token=", TEST_TOKEN));
    assert!(!check_token("", TEST_TOKEN));
    assert!(!check_token("other=abc123", TEST_TOKEN));
}
```

**Timeout-protected TCP operations (E2E):**
```rust
tokio::time::timeout(Duration::from_secs(2), async {
    // potentially-blocking IO
})
.await
.expect("operation timed out");
```

---

*Testing analysis: 2026-06-08*
