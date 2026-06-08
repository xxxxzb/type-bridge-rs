# External Integrations

**Analysis Date:** 2026-06-08

## APIs & External Services

**None.** This application has zero external API dependencies. All services run locally on the LAN.

The application is entirely self-contained:
- HTTP server (axum) bound to `0.0.0.0:{port}` — serves the mobile web UI
- Socket.IO server (socketioxide) on the same port — bidirectional real-time communication with the mobile browser
- No cloud APIs, no SaaS integrations, no telemetry services

## Data Storage

**Databases:**
- None. No database of any kind. The application runs statelessly per session.

**File Storage:**
- Local filesystem only — no external file storage services
- Icon assets are embedded as `const` bytes in `src/tray_icons.rs` (4096-byte RGBA arrays for tray icons)
- Socket.IO client library embedded as `include_str!("socket.io.min.js")` in `src/server.rs`

**Caching:**
- None. There is no caching layer.

## Authentication & Identity

**Auth Provider:**
- Custom token-based authentication, entirely self-contained:
  - Token generated at startup via cryptographically random 128-bit value (`getrandom` in `src/main.rs`)
  - Encoded as base64url (22 characters, no padding)
  - Token is embedded in the connection URL: `http://{ip}:{port}/?token={token}`
  - Validated on every HTTP request (via axum middleware) and Socket.IO namespace connection (via `check_token` in `src/server.rs`)
  - No persistent storage, no user accounts, no session management beyond Engine.IO's internal `sid`
  - Token validation includes URL-decoding support (`urlencoding` function in `src/server.rs`)

## Monitoring & Observability

**Error Tracking:**
- None. No external error tracking service (Sentry, etc.).

**Logs:**
- Structured logging via `tracing` / `tracing-subscriber` (fmt layer, stdout)
- Log levels used: `info` (connection events), `warn` (command queue full, unrecognized keys), `error` (enigo/clipboard/softbuffer failures)
- Init: `tracing_subscriber::fmt::init()` in `src/main.rs` (line 97)
- No log aggregation, no log shipping, no external log service

## CI/CD & Deployment

**Hosting:**
- No hosting platform. Application runs as a native desktop process on the user's machine.

**CI Pipeline:**
- None detected. No `.github/` directory, no CI configuration files.

**Deployment:**
- Native binary distribution. macOS users can build a `.app` bundle via `scripts/build-mac.sh`.
- No package registry, no auto-update mechanism.

## Environment Configuration

**Required env vars:**
- None. Zero environment variables required.

**Secrets location:**
- No secrets. The auth token is generated at runtime and printed once to stdout (and embedded in the tray tooltip URL). It is never persisted or stored.

## Webhooks & Callbacks

**Incoming:**
- None. No webhook endpoints.

**Outgoing:**
- None. The application does not make outbound HTTP calls.

## System Integrations

**Keyboard Simulation (`src/keyboard.rs`):**
- Uses [enigo](https://github.com/enigo-rs/enigo) 0.5 for cross-platform keyboard input simulation
- TypeText works via clipboard paste: copies text to clipboard, presses Cmd+V (macOS) / Ctrl+V (other), restores previous clipboard content
- Backspace works via Alt+Backspace (macOS) / Ctrl+Backspace (other) for word-level deletion
- Enter and SelectAll use native key events
- macOS requires Accessibility permissions (CFBundle with `NSAccessibilityUsageDescription` in Info.plist)

**Clipboard (`src/keyboard.rs`, `src/main.rs`):**
- Uses [arboard](https://github.com/1Password/arboard) 3 for clipboard read/write
- Used for: TypeText paste method, clipboard content restoration, "Copy URL" tray action

**System Tray (`src/tray.rs`, `src/tray_icons.rs`):**
- Uses [tray-icon](https://github.com/nicoverbruggen/tray-icon) 0.19 + [muda](https://github.com/nicoverbruggen/muda) 0.15
- Menu items: URL display, Toggle Typing ON/OFF, Show QR Code, Copy URL, Quit
- Two icon states: active (colored `ICON_ON`) and paused (desaturated `make_icon_off()`)
- macOS: LSUIElement set to true (menu-bar app, no dock icon)

**QR Code Display (`src/main.rs`):**
- Uses [qrcode](https://github.com/kennytm/qrcode-rust) 0.14 to generate QR codes from the connection URL
- Uses [winit](https://github.com/rust-windowing/winit) 0.29 for window creation (always-on-top, frameless popup)
- Uses [softbuffer](https://github.com/rust-windowing/softbuffer) 0.4 for pixel buffer rendering
- Also prints QR code to terminal as Unicode block characters (`print_qr` function)

**Network Interface (`src/ip.rs`):**
- Uses [if-addrs](https://github.com/messense/if-addrs-rs) 0.13 to enumerate network interfaces
- Finds first non-loopback IPv4 address for the LAN URL
- Falls back to `127.0.0.1` if no LAN interface found

---

*Integration audit: 2026-06-08*
