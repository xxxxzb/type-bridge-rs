# Technology Stack

**Analysis Date:** 2026-06-08

## Languages

**Primary:**
- Rust 2021 Edition (edition = "2021") — Entire application, all source files in `src/`

**Secondary:**
- JavaScript (bundled as embedded string) — Socket.IO client frontend, inline in `src/assets.rs` the HTML page served to mobile browsers

## Runtime

**Environment:**
- Native binary compiled via `rustc`. No external runtime required.
- Minimum macOS version: 12.0 (`scripts/Info.plist` — `LSMinimumSystemVersion`)
- Supports macOS, Windows, Linux

**Package Manager:**
- Cargo (Rust official package manager)
- Lockfile: `Cargo.lock` (version 4) — present, should be committed

## Frameworks

**Core:**
- [axum](https://github.com/tokio-rs/axum) 0.8 — HTTP server framework, request routing, middleware (`src/server.rs`)
- [socketioxide](https://github.com/Totodore/socketioxide) 0.16 — Socket.IO server implementation on top of axum, WebSocket + long-polling real-time communication (`src/server.rs`)

**Testing:**
- Rust built-in `#[test]` / `#[tokio::test]` — Unit and integration tests
- [tower](https://github.com/tower-rs/tower) 0.5 (dev-dependency) — Service trait for HTTP test client (`src/server.rs` tests use `.oneshot()`)

**Build/Dev:**
- Cargo — Build system
- `scripts/build-mac.sh` — macOS `.app` bundle creation script (builds release binary, creates app bundle, ad-hoc codesigns, optionally installs to `/Applications`)
- `scripts/Info.plist` — macOS bundle metadata (bundle ID `com.typebridge.rs`, LSUIElement for menu-bar app)

## Key Dependencies

**Critical:**
- [tokio](https://tokio.rs) 1 — Async runtime (multi-threaded, net, sync, macros, time features). One runtime per server thread, main thread uses `winit` event loop
- [serde](https://serde.rs) 1 + serde_json 1 — JSON serialization/deserialization for Socket.IO payloads and token parsing
- [enigo](https://github.com/enigo-rs/enigo) 0.5 — Cross-platform keyboard simulation (TypeText, Backspace, Enter, SelectAll via clipboard paste + key events) (`src/keyboard.rs`)
- [arboard](https://github.com/1Password/arboard) 3 — Clipboard read/write for Unicode text transfer (`src/keyboard.rs`, `src/main.rs`)
- [clap](https://github.com/clap-rs/clap) 4 with derive — CLI argument parsing (`--port` flag, default 12345) (`src/main.rs`)

**Infrastructure:**
- [tray-icon](https://github.com/nicoverbruggen/tray-icon) 0.19 + [muda](https://github.com/nicoverbruggen/muda) 0.15 — System tray icon and context menu (`src/tray.rs`, `src/tray_icons.rs`)
- [winit](https://github.com/rust-windowing/winit) 0.29 — Windowing library for the QR code popup window (`src/main.rs`)
- [softbuffer](https://github.com/rust-windowing/softbuffer) 0.4 — Software-rendered surface for displaying QR code pixels in the window (`src/main.rs`)
- [qrcode](https://github.com/kennytm/qrcode-rust) 0.14 — QR code generation, rendered in both terminal (Unicode) and pixel buffer for window (`src/main.rs`)
- [tracing](https://github.com/tokio-rs/tracing) 0.1 + tracing-subscriber 0.3 — Structured logging (initialized at startup via `tracing_subscriber::fmt::init()`) (`src/main.rs`)
- [if-addrs](https://github.com/messense/if-addrs-rs) 0.13 — Network interface enumeration to find the local LAN IP address (`src/ip.rs`)
- [getrandom](https://github.com/rust-random/getrandom) 0.2 — Cryptographically random bytes for token generation (`src/main.rs`)

## Configuration

**Environment:**
- No `.env` files. No environment variables used.
- All configuration is via CLI flags (currently only `--port`)

**Build:**
- `Cargo.toml` — Package manifest, dependencies, dev-dependencies
- `scripts/build-mac.sh` — macOS bundle build script
- `scripts/Info.plist` — macOS bundle Info.plist template

## Platform Requirements

**Development:**
- Rust toolchain 1.75+ (per README badge; edition 2021 implies stable >= 1.56)
- macOS: For local development and `.app` bundle building
- On macOS, enigo requires Accessibility permissions for keyboard simulation

**Production:**
- Single static binary (after compilation, zero runtime dependencies)
- Requires network access on the configured port (default 12345) — user may need to configure firewall rules
- macOS: Accessibility permission for keyboard simulation
- Linux: `wl-clipboard` for Wayland clipboard support

---

*Stack analysis: 2026-06-08*
