# Codebase Concerns

**Analysis Date:** 2026-06-08

## Tech Debt

### Global Mutable State in `keyboard.rs`

- Issue: Three static globals (`ENIGO`, `COMMAND_TX`, `ENABLED`) are shared across threads via `OnceLock<Mutex<...>>`, `Mutex<Option<...>>`, and `AtomicBool`. This creates implicit coupling between the server thread and the main event loop thread, and makes reasoning about state harder.
- Files: `src/keyboard.rs` (lines 10-13)
- Impact: Test isolation requires a `TestGuard` pattern with a global `TEST_STATE_LOCK` mutex that serialises all tests using these globals. Concurrency bugs in production (e.g., a poisoned mutex) are silently recovered via `.lock().unwrap_or_else(|e| e.into_inner())`, hiding real issues.
- Fix approach: Replace static globals with dependency-injected state. Pass an `Arc<KeyboardState>` through the event loop and server setup instead of relying on module-level statics.

### Poisoned Mutex Handling Hides Bugs

- Issue: Many places use `.lock().unwrap_or_else(|e| e.into_inner())` to silently recover from poisoned mutexes. This means if a panic occurs while holding one of these locks, all subsequent operations proceed on potentially corrupted state.
- Files: `src/keyboard.rs` (lines 24, 55, 68, 82, 110), `src/server.rs`
- Impact: Silent data corruption. A panic in `enigo()` while the `ENIGO` mutex is held poisons the mutex; subsequent calls silently recover but the underlying `Enigo` instance may be in an inconsistent state.
- Fix approach: Use `.lock().expect("...")` with meaningful messages so poisoned mutexes are surfaced immediately, or restructure to avoid holding locks across fallible operations.

### `unsafe impl Sync for SyncProxy`

- Issue: `EventLoopProxy<TrayEvent>` does not implement `Sync` in winit, but the code needs to share it across threads. A manual `unsafe impl Sync` wrapper is used.
- Files: `src/main.rs` (lines 42-44)
- Impact: Any future thread-safety mistake in how the proxy is used would be unsound, and the compiler would not catch it.
- Fix approach: Keep the wrapper but add a clippy `deny(unsafe_code)` lint elsewhere to ensure the `unsafe` is isolated and auditable. Consider if `EventLoopProxy` upstream has added `Sync` support in newer winit versions.

### `server.rs` Excessive Size

- Issue: `src/server.rs` is 878 lines, making it the largest file in the codebase. This is driven by ~380 lines of E2E integration tests embedded directly in the same file as production code.
- Files: `src/server.rs`
- Impact: Difficult to navigate. Production logic is mixed with test infrastructure (helper functions, mock server, TCP-level parsing). The `e2e_tests` module alone is ~300 lines.
- Fix approach: Extract E2E test helpers into a separate test support module (e.g., `tests/e2e/mod.rs`) or into `tests/integration.rs`.

### Hardcoded Sleep Timings

- Issue: Keyboard execution uses hardcoded `std::thread::sleep(Duration::from_millis(30))` and `50ms` waits for clipboard operations and modifier key sequences.
- Files: `src/keyboard.rs` (lines 202, 221)
- Impact: Fragile across platforms. On slower machines the delays may be insufficient; on faster machines they waste time. No adaptive timing or polling-based synchronisation.
- Fix approach: For clipboard operations, poll for clipboard availability instead of sleeping. For key sequences, use platform-specific key event acknowledgment where possible.

### Socket.IO Client JavaScript Embedded as Binary

- Issue: A 49KB minified `socket.io.min.js` is embedded via `include_str!` in `src/server.rs` and served as a static asset. This increases binary size and makes updating the Socket.IO protocol version a manual process.
- Files: `src/server.rs` (line 14), `src/socket.io.min.js`
- Impact: Binary size increase of ~49KB. The JavaScript is version-locked to whatever was downloaded at the time it was placed in the repo.
- Fix approach: Keep the embedding (it aligns with the "single binary, zero dependencies at runtime" goal), but add a build script that fetches and verifies the specific Socket.IO client version matching the `socketioxide` server crate version.

### Missing `rust-toolchain.toml` / Pinned Toolchain

- Issue: No `rust-toolchain.toml` file, so builds use whatever Rust toolchain is installed locally. The `Cargo.toml` specifies `edition = "2021"` but not a minimum Rust version.
- Files: `Cargo.toml`
- Impact: Builds may fail or produce different results depending on the user's installed Rust version. CI and local development can diverge.
- Fix approach: Add a `rust-toolchain.toml` with the minimum supported Rust version (e.g., 1.75 as documented in README).

---

## Known Bugs

### Clipboard Content Lost on Execution Failure

- Symptoms: If `execute_type_text` successfully saves the previous clipboard content, then fails during the paste operation (e.g., `enigo.key()` errors), the original clipboard content is NOT restored because the restore code only runs after the paste sequence, not in error paths.
- Files: `src/keyboard.rs` (lines 186-228)
- Trigger: An enigo keystroke error between clipboard save and clipboard restore.
- Workaround: None. The user would need to manually re-copy their original clipboard content.

### Backspace Deletes Word Instead of Character on macOS

- Symptoms: `execute_backspace` presses Alt+Backspace on macOS, which deletes the entire previous word rather than a single character. This does not match the button behavior expected by desktop users who want character-by-character deletion.
- Files: `src/keyboard.rs` (lines 230-247)
- Trigger: Clicking the "backspace" button on the phone UI.
- Workaround: None from the UI side. The backspace behavior is intentionally word-level but not documented in the UI button label.

### History Duplication with Multiple Clients

- Symptoms: If two mobile browsers connect simultaneously, both receive history events. When client A sends `type_text`, the updated history is broadcast to all connected clients. Client B will see client A's text in its history, which may be unexpected.
- Files: `src/server.rs` (lines 106-118)
- Trigger: Multiple concurrent Socket.IO connections.
- Workaround: Only one client should be used at a time.

---

## Security Considerations

### Token in URL Query String

- Risk: The authentication token is passed exclusively as a URL query parameter (`?token=...`). It is visible in browser history, browser autocomplete, server logs, the OS process list (`ps aux`), and any intermediate HTTP proxies. The token also appears in the tray tooltip and URL menu item.
- Files: `src/main.rs` (line 103), `src/server.rs` (lines 64-83, 86-95, 147-159), `assets.rs` (JS reads from `location.search`)
- Current mitigation: Tokens are 128-bit random (22 base64url chars) — computationally infeasible to guess. The server only listens on local network interfaces, not the public internet. The HTML page never embeds the token in the DOM.
- Recommendations: Use HTTP-only cookies for the initial page request and WebSocket handshake. Alternatively, use a session-based approach where the initial request sets a cookie and subsequent Socket.IO connections authenticate via the cookie. At minimum, log the warning "Token in URL is visible in browser history and server logs".

### No HTTPS/TLS

- Risk: All traffic is plain HTTP. Any device on the same network can sniff the token and keystroke data using ARP spoofing or passive packet capture.
- Files: `src/server.rs` (line 203: binds to `0.0.0.0`)
- Current mitigation: The project is designed exclusively for local LAN use ("runs entirely on your home network"). Self-signed HTTPS would require a CA certificate installation on the phone.
- Recommendations: Document the risk prominently in the README so users understand that an attacker on the same network can intercept typing. Consider adding optional mTLS or a simple shared-secret-based encryption layer for sensitive environments.

### No CSRF Protection on Token-Protected Routes

- Risk: The `GET /?token=...` route returns the HTML page. A malicious site could embed `<img src="http://<victim-ip>:12345/?token=<leaked-token>">` to confirm the server exists and the token is valid (timing side-channel or 200 vs 403 status observed via JavaScript).
- Files: `src/server.rs` (lines 147-159)
- Current mitigation: The token is 128-bit random and changes on every server restart. No side effects occur on GET requests.
- Recommendations: Add a `Origin` or `Referer` header check on the auth middleware. This prevents cross-origin requests from loading the page, even on LAN.

### No Input Rate Limiting

- Risk: There is no rate limiting on Socket.IO events. A malicious client on the local network (or a compromised device) could flood the server with `type_text` events, causing the command channel to fill up (256 entries) and the event loop to spend excessive time draining commands or dropping events.
- Files: `src/server.rs` (Socket.IO handlers), `src/main.rs` (line 165: `MAX_COMMANDS_PER_TICK`)
- Current mitigation: The synchronous channel has capacity 256 (`KB_CHANNEL_BOUND`). Overflow is logged and dropped. The event loop caps processing at `MAX_COMMANDS_PER_TICK = 32` per tick.
- Recommendations: Add per-client rate limiting (e.g., token bucket) and per-connection command rate limits. Monitor and alert on command drops.

---

## Performance Bottlenecks

### Synchronous `std::thread::sleep` in Async Context

- Problem: The keyboard execution functions use `std::thread::sleep` for 30-50ms. When called from the main event loop, this blocks the entire event loop, preventing it from processing window events, tray events, or draining commands from the channel.
- Files: `src/keyboard.rs` (lines 202, 221), `src/main.rs` (line 178)
- Cause: `execute()` is called synchronously from the winit event loop. The `execute_type_text` function blocks for ~80ms total (30ms + 50ms sleep).
- Improvement path: Delegate keyboard execution to a dedicated background thread so the event loop remains responsive. Use `tokio::spawn_blocking` or a dedicated std::thread with its own channel.

### QR Code Image Regeneration on Every Redraw

- Problem: `render_qr` recreates `softbuffer::Context` and `softbuffer::Surface` on every redraw request, rather than caching them for the lifetime of the QR window.
- Files: `src/main.rs` (lines 320-321)
- Cause: The surface/context is created inside the render function with no caching.
- Improvement path: Cache the `softbuffer::Context` and `Surface` in the `QrWindow` struct and only recreate them if the window size changes.

---

## Fragile Areas

### Keyboard Module Global State

- Files: `src/keyboard.rs`
- Why fragile: Three interacting pieces of mutable global state (`ENABLED`, `COMMAND_TX`, `ENIGO`) with implicit sharing between the server thread and the event loop thread. The `TestGuard` pattern serialises all tests, preventing parallel test execution. The `enigo()` function panics if `Enigo::new` fails at first use, with no graceful degradation.
- Safe modification: Always use `TestGuard` when modifying keyboard state in tests. Never add new static globals to this module. Prefer passing state explicitly.
- Test coverage: Good coverage for merging, queuing, and channel backpressure. Zero coverage for `execute_*` functions (they depend on OS-level enigo/arboard).

### Server Auth Middleware

- Files: `src/server.rs` (lines 63-83, 86-95)
- Why fragile: Token validation happens at two levels (HTTP middleware + Socket.IO namespace handler) with duplicated `check_token` logic. The middleware skips token check for requests that already have a `sid` parameter, trusting that the sid was obtained through a valid handshake. This works because Engine.IO sids are unguessable, but it is an implicit trust boundary.
- Safe modification: Never remove the namespace-level token check — it is defense-in-depth. Ensure the sid-based bypass only applies to engine.io polling requests, not arbitrary paths.
- Test coverage: Strong. Multiple test cases cover auth scenarios including sid-based bypass.

### Event Loop Single-Threaded Architecture

- Files: `src/main.rs`
- Why fragile: Everything runs on one thread — window events, tray events, keyboard command execution. Any blocking operation (sleep, clipboard, enigo) pauses the entire loop. Adding new features that perform I/O or blocking work requires careful scheduling or thread offloading.
- Safe modification: Keep blocking operations in the keyboard execution path. For new I/O features, use the tokio runtime in the background thread rather than the winit event loop thread.
- Test coverage: No tests exercise the event loop directly (it requires a display server). Only individual component tests exist.

---

## Scaling Limits

### Single-Client Socket.IO Model

- Current capacity: The server accepts multiple concurrent Socket.IO connections, but history is shared across all of them. Keyboard state (enabled/disabled) is global. The keyboard execution path has no concept of which client sent a command.
- Limit: Two people using the service simultaneously would interfere with each other's history and typing.
- Scaling path: This is an intentional design limitation. The project is designed for single-user usage. Multi-user support would require per-connection keyboard state, per-connection history, and potentially input queue merging.

### Command Channel Capacity

- Current capacity: `KB_CHANNEL_BOUND = 256` commands in the synchronous channel.
- Limit: Rapid typing from a fast client, combined with a slow keyboard executor, will fill the channel. Events beyond 256 are dropped silently (logged as warning). The event loop is limited to 32 commands per tick, so at peak load a backlog of ~8 ticks worth of commands can accumulate before dropping.
- Scaling path: Increase channel capacity or switch to an unbounded channel with a high-water mark for monitoring. Add per-client backpressure via Socket.IO acknowledgements.

---

## Dependencies at Risk

### `enigo` 0.5

- Risk: Version 0.5 is several major versions behind the current release. The `enigo` crate has undergone significant API changes in 0.6+ and may have unpatched platform-specific bugs in 0.5.
- Impact: Platform-specific keyboard simulation bugs (especially on Wayland and newer macOS versions). The 0.5 API uses `Enigo::new(&Settings::default())` which differs from the 0.6+ builder pattern.
- Migration plan: Upgrade to `enigo` 0.6+ and adapt the API calls. The 0.6 API uses a builder pattern (`Enigo::new(&Settings::default())` -> `Enigo::new(&builder, ...)`).

### `getrandom` 0.2

- Risk: Pinned to the 0.2 line, while 0.3 is the current major version.
- Impact: Minor — 0.2 is still maintained and the API surface used here (simple call to `getrandom()`) is stable.
- Migration plan: Bump to 0.3 for latest platform support and security fixes.

### `winit` 0.29

- Risk: `winit` 0.29 is not the latest version (current is 0.30+). Newer versions have API changes that would require migration.
- Impact: The code uses `EventLoopBuilder::<T>::with_user_event()` and `WindowBuilder`, which have changed in newer winit versions.
- Migration plan: Stay on 0.29 unless upstream dependencies (tray-icon, softbuffer) require newer winit. The `SyncProxy` hack may be unnecessary in newer winit versions.

### `tray-icon` 0.19 / `muda` 0.15

- Risk: These are tightly coupled versions. Upgrading one requires upgrading the other.
- Impact: Currently functional, but these are relatively new crates with breaking API changes between minor versions.
- Migration plan: Keep version pairs in sync. Pin exact versions in Cargo.toml rather than using caret requirements.

---

## Missing Critical Features

### No Graceful Permission Denied Handling

- Problem: On macOS, if the user has not granted Accessibility permissions, `Enigo::new` will panic. The application crashes immediately without explaining why or how to fix it.
- Blocks: First-time users on macOS who skip or miss the accessibility permission prompt will get a crash instead of a helpful error message.
- Files: `src/keyboard.rs` (line 91)

### No Copy-on-Select for History Items

- Problem: Clicking a history item populates the textarea but does not automatically send the text. The user must press the Send button.
- Impact: Minor UX friction. Users expect clicking a history entry to either send immediately or offer a "send on tap" option.
- Files: `src/assets.rs` (lines 383-389)

---

## Test Coverage Gaps

### Keyboard Execution Functions Not Tested

- What's not tested: `execute_type_text`, `execute_backspace`, `execute_enter`, `execute_select_all` — all functions that interact with the OS via `enigo` and `arboard`.
- Files: `src/keyboard.rs` (lines 177-273)
- Risk: Platform-specific bugs in keyboard simulation or clipboard operations go undetected until runtime. Breaking changes in `enigo` or `arboard` APIs are not caught by tests.
- Priority: Medium. These are hard to test without OS-level mocking or integration test infrastructure.

### Window / QR Rendering Not Tested

- What's not tested: `render_qr`, `open_qr_window`, `print_qr`, `copy_to_clipboard`, `qr_pixels` — all functions that create windows, render pixels, or interact with the display server.
- Files: `src/main.rs` (lines 259-400)
- Risk: Display server integration issues (e.g., wayland vs x11, macOS Retina scaling) are not caught.
- Priority: Low. These are visual/non-critical paths and would require headless testing infrastructure.

### Tray Menu Interaction Not Tested

- What's not tested: The event loop's handling of `TrayEvent::Menu` (toggle, show QR, copy URL, quit) and `TrayEvent::TrayClick`.
- Files: `src/main.rs` (lines 188-222)
- Risk: Adding new tray menu items or changing event handling could break existing functionality without detection.
- Priority: Medium. The quit/toggle paths are critical for UX.

---

*Concerns audit: 2026-06-08*
