<!-- refreshed: 2026-06-08 -->
# Architecture

**Analysis Date:** 2026-06-08

## System Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│                     Phone Browser (Web UI)                       │
│         Socket.IO client over Wi-Fi (polling + WebSocket)       │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                      HTTP / Socket.IO Server                     │
│                          `src/server.rs`                         │
│   axum Router + socketioxide + token auth middleware             │
│   Handles: /, /sio.min.js, /socket.io/*                         │
└───────────────────────────┬─────────────────────────────────────┘
                            │  (mpsc SyncSender<KeyCommand>)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Main Event Loop (winit)                       │
│                          `src/main.rs`                           │
│   Drains command channel, merges commands, executes OS input     │
│   Manages: QR window, system tray, menu events, auto-close      │
└────────────┬─────────────────────────┬──────────────────────────┘
             │                         │
             ▼                         ▼
┌──────────────────────────┐  ┌─────────────────────────────────────┐
│  enigo Keyboard Input    │  │  arboard Clipboard                  │
│  `src/keyboard.rs`       │  │  `src/keyboard.rs`                  │
│  Simulates Cmd+V/Ctrl+V  │  │  Reads/writes clipboard text        │
│  Backspace (Alt+BS)      │  │  Restores prior clipboard content   │
│  Enter / Select All      │  │                                     │
└──────────────────────────┘  └─────────────────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `main.rs` | Entry point, event loop, QR rendering, tray event handling, command dispatch | `src/main.rs` |
| `server.rs` | HTTP server, Socket.IO handlers, token auth, type-text/press-key/backspace event routing | `src/server.rs` |
| `keyboard.rs` | OS-level keyboard simulation, clipboard management, command queue, command merging | `src/keyboard.rs` |
| `tray.rs` | System tray icon build, menu construction, icon generation | `src/tray.rs` |
| `tray_icons.rs` | Embedded RGBA icon data for active/paused tray states | `src/tray_icons.rs` |
| `ip.rs` | Local IP address auto-detection (non-loopback IPv4) | `src/ip.rs` |
| `assets.rs` | Static HTML page for the web UI (embedded as `&str`) | `src/assets.rs` |

## Pattern Overview

**Overall:** Desktop GUI application with embedded HTTP server pattern. The application runs a local HTTP+WebSocket server on a separate OS thread (with its own Tokio runtime), while the main thread runs a winit event loop that handles the system tray, QR-code overlay window, and dispatches keyboard commands to the OS via enigo.

**Key Characteristics:**
- Communication between the server thread and the event loop thread uses a bounded `mpsc::sync_channel<KeyCommand>` (capacity 256)
- Server shutdown is coordinated via `tokio::sync::oneshot`
- Tray events are injected into the winit event loop via `EventLoopProxy`
- The HTML UI and Socket.IO client JS are embedded as `include_str!()` at compile time -- zero external files at runtime
- Token-based authentication prevents unauthorized LAN access (128-bit random token in URL)
- All OS-level input is guarded by an `AtomicBool` toggle (enabled/disabled)

## Layers

**Network / Server Layer:**
- Purpose: Accept HTTP and WebSocket connections from phone browsers
- Location: `src/server.rs`
- Contains: axum Router, socketioxide namespace handlers, token auth middleware
- Depends on: `keyboard.rs` (queue functions), `assets.rs`, embedded `socket.io.min.js`
- Used by: Spawned as a thread from `main.rs`

**GUI / Event Loop Layer:**
- Purpose: Manage system tray, QR code window, dispatch commands to OS
- Location: `src/main.rs`
- Contains: winit event loop, tray event handling, QR pixel rendering via softbuffer, clipboard copy
- Depends on: `keyboard.rs`, `tray.rs`, `tray_icons.rs`, `ip.rs`
- Used by: Entry point (calling `event_loop.run()`)

**OS Input Layer:**
- Purpose: Simulate keyboard input and manage clipboard
- Location: `src/keyboard.rs`
- Contains: enigo-based key simulation, arboard clipboard operations, command queue, command merging logic
- Depends on: enigo, arboard
- Used by: Both server layer (queue) and event loop layer (execute/merge)

## Data Flow

### Primary Request Path -- Phone to PC key input

1. **Phone browser** sends Socket.IO `type_text` event to `/socket.io/` (`src/server.rs:106-122`)
2. **server thread** validates token, calls `keyboard::queue_type_text()` (`src/keyboard.rs:119-131`)
3. **Queued** as `KeyCommand::TypeText(text)` on the `mpsc::SyncSender<KeyCommand>` channel (`src/keyboard.rs:107-117`)
4. **Main event loop** drains channel in `event_loop.run()` (`src/main.rs:164-179`), limited to 32 commands per tick (`MAX_COMMANDS_PER_TICK`)
5. **Merge optimization** groups consecutive `TypeText` commands (`src/keyboard.rs:158-173`, called at `src/main.rs:177`)
6. **Execution** via `keyboard::execute()` (`src/main.rs:178`, defined `src/keyboard.rs:177-184`)
7. **Clipboard path** for `TypeText`: sets clipboard text, simulates Cmd+V/Ctrl+V paste, restores original clipboard (`src/keyboard.rs:186-228`)
8. **Direct key path** for `Backspace`/`Enter`/`SelectAll`: simulates keystrokes via enigo (`src/keyboard.rs:230-273`)

### Tray Interaction Flow

1. User clicks tray icon or menu item
2. `tray_icon::TrayIconEvent` / `muda::MenuEvent` fires
3. Event handlers forward to `EventLoopProxy` (`src/main.rs:142-151`)
4. Main event loop receives as `Event::UserEvent(TrayEvent::...)` (`src/main.rs:192-221`)
5. Handles: toggle typing, show QR, copy URL, quit

### QR Window Flow

1. User triggers "Show QR Code" via tray icon click or menu
2. `open_qr_window()` creates a `winit::window::Window` with softbuffer surface (`src/main.rs:344-376`)
3. QR code rendered as RGBA pixels from `qrcode` crate (`src/main.rs:259-298`)
4. Window auto-closes after 5 seconds of inactivity (`src/main.rs:182-184`)

**State Management:**
- **Enabled/disabled toggle**: Global `AtomicBool` in `src/keyboard.rs:10` (`ENABLED`)
- **Command queue**: Global `Mutex<Option<mpsc::SyncSender<KeyCommand>>>` in `src/keyboard.rs:12-13` (`COMMAND_TX`)
- **enigo instance**: `OnceLock<Mutex<Enigo>>` in `src/keyboard.rs:11` (`ENIGO`) -- lazy-initialized singleton
- **Tray state**: `Rc<RefCell<TrayState>>` in `src/main.rs:135` -- reference-counted, interior mutability
- **QR window state**: `Rc<RefCell<Option<QrWindow>>>` in `src/main.rs:153`
- **History**: `Arc<Mutex<VecDeque<String>>>` in `src/server.rs:59-60` -- shared between socket handlers
- **Shutdown**: `Cell<Option<oneshot::Sender<()>>>` in `src/main.rs:109-110`

## Key Abstractions

**KeyCommand Enum:**
- Purpose: Represents all possible keyboard actions that can be queued from network and executed on the event loop
- Variants: `TypeText(String)`, `Backspace`, `Enter`, `SelectAll`
- Defined at: `src/keyboard.rs:16-21`
- Pattern: Tagged enum with command pattern (queue/execute split)

**TrayState Struct:**
- Purpose: Holds references to system tray icon, menu items, and their IDs for stateful tray updates
- Defined at: `src/tray.rs:7-14`
- Pattern: Builder pattern (built by `build_tray()`), consumed via `Rc<RefCell<>>`

**QrWindow Struct:**
- Purpose: Wraps a winit window with cached QR pixel data for efficient re-rendering
- Fields: `window`, `qr_pixels` (cached RGBA), `qr_cached_scale` (cache key)
- Defined at: `src/main.rs:46-52`
- Pattern: Cache-aside pattern (regenerates QR pixels only when display scale changes)

**SyncProxy Struct:**
- Purpose: Safe wrapper around `EventLoopProxy` for cross-thread access
- SAFETY: Unsafe impl Sync because `EventLoopProxy::send_event()` is documented as thread-safe
- Defined at: `src/main.rs:42-44`

**TestGuard Struct:**
- Purpose: Test-only RAII guard that saves/restores global keyboard state (ENABLED, COMMAND_TX) and serializes E2E tests
- Defined at: `src/keyboard.rs:45-86`
- Pattern: RAII guard with exclusive mutex lock

## Entry Points

**CLI startup:**
- Location: `src/main.rs:96-254`
- Triggers: User runs `type-bridge-rs` binary (with optional `--port` flag via clap)
- Responsibilities: Parse CLI args, detect local IP, generate auth token, spawn server thread, run event loop, manage tray and QR window

**HTTP Server:**
- Location: `src/server.rs:200-211` (`run()`) and `src/server.rs:215-227` (`serve()`)
- Triggers: Spawned as `std::thread` from `main()`
- Responsibilities: Bind TCP listener, serve axum Router with graceful shutdown

**Test Entry Points:**
- `spawn_server()` at `src/server.rs:595-610` -- E2E test helper that pre-binds a 127.0.0.1 listener
- `test_router()` at `src/server.rs:238-240` -- builds router without binding for unit tests

## Architectural Constraints

- **Threading:** Two-thread architecture. Main thread runs the winit event loop (single-threaded, UI-driven). A second OS thread runs the Tokio async runtime for the HTTP/Socket.IO server. Communication via bounded `mpsc::sync_channel`. Tray events injected into event loop via `Arc<SyncProxy(EventLoopProxy)>`.
- **Global state:** Several module-level statics in `src/keyboard.rs`: `ENABLED` (AtomicBool), `ENIGO` (OnceLock<Mutex<Enigo>>), `COMMAND_TX` (Mutex<Option<SyncSender>>)). These are the primary mutable global state -- all guarded by appropriate synchronization primitives.
- **Token-based auth:** The same random token is embedded in the printed URL and validated on every HTTP request and Socket.IO connection. The token is NOT embedded in the HTML page -- the client reads it from `location.search`.
- **No circular imports:** The module dependency chain is strictly linear: `main.rs` depends on all other modules. `server.rs` depends on `keyboard.rs` and `assets.rs`. `keyboard.rs`, `tray.rs`, `tray_icons.rs`, `ip.rs` are leaf modules.

## Error Handling

**Strategy:** Fail-loud with `expect()` for critical initialization (bind, window creation, tray build). Non-critical errors (keyboard simulation failures, clipboard issues) are logged via `tracing::error!()` / `tracing::warn!()` and do not crash the application.

**Patterns:**
- `expect()` with descriptive messages for one-time initialization: QR generation, window creation, tray icon creation, event loop run
- `tracing::error!()` for runtime OS input failures: key presses, clipboard operations
- `tracing::warn!()` for backpressure: command queue full, text too long
- `unwrap_or_else(|e| tracing::error!(...))` for tray operations

## Cross-Cutting Concerns

**Logging:** `tracing` + `tracing-subscriber` crate, initialized at `src/main.rs:98` with `tracing_subscriber::fmt::init()`. Warning-level and error-level logs only -- no debug/info in production paths.

**Validation:** Token validation via `check_token()` in `src/server.rs:44-52` -- a pure function that parses query string and matches token. URL decoding handled by `urlencoding()` (`src/server.rs:177-198`). Socket.IO namespace also re-validates token on connection.

**Authentication:** Single static token approach. Generated once at startup with 128 bits of cryptographically random entropy (via `getrandom`), encoded in base64url. Validated at three points: HTTP middleware for `/socket.io/*`, handler for `/`, and Socket.IO namespace `on("connection")`.

---

*Architecture analysis: 2026-06-08*
