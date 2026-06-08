# Codebase Structure

**Analysis Date:** 2026-06-08

## Directory Layout

```
type-bridge-rs/
├── .gitignore                     # Ignores /target and /.omc/
├── Cargo.toml                     # Rust project manifest (v0.3.0, edition 2021)
├── Cargo.lock                     # Dependency lockfile
├── LICENSE                        # MIT License
├── README.md                      # Project documentation (Chinese)
│
├── assets/
│   └── icons/                     # App and tray icon assets
│       ├── icon-on.svg            # SVG icon for active state
│       ├── icon-off.svg           # SVG icon for paused state
│       └── icon_*.png             # PNG icons at various sizes (16 to 512 px)
│
├── scripts/
│   ├── build-mac.sh               # macOS .app bundle build script
│   └── Info.plist                 # macOS bundle Info.plist template
│
└── src/
    ├── main.rs                    # Entry point, winit event loop, QR rendering
    ├── server.rs                  # HTTP server (axum), Socket.IO, auth, E2E tests
    ├── keyboard.rs                # OS keyboard simulation, clipboard, command queue
    ├── tray.rs                    # System tray icon and menu
    ├── tray_icons.rs              # Embedded RGBA icon pixel data
    ├── ip.rs                      # LAN IP auto-detection
    ├── assets.rs                  # Static HTML page (embedded &str)
    └── socket.io.min.js           # Embedded Socket.IO client library
```

## Directory Purposes

**`src/`:**
- Purpose: All application source code
- Contains: 7 Rust modules + 1 embedded JS file (994 lines total across Rust files, 288 lines for the icon data file)
- Key files: `main.rs` (439 lines), `server.rs` (878 lines), `keyboard.rs` (456 lines)

**`assets/icons/`:**
- Purpose: Application and system tray icon assets
- Contains: SVG source files and rasterized PNGs at multiple resolutions (16x16 through 512x512)
- Note: The actual tray icon used at runtime is embedded directly in `src/tray_icons.rs` as raw RGBA byte arrays

**`scripts/`:**
- Purpose: Build and packaging scripts
- Contains: `build-mac.sh` for creating a macOS `.app` bundle, `Info.plist` template with version-substitution variables

**Root directory:**
- `Cargo.toml` -- project manifest declaring 22 dependencies
- `Cargo.lock` -- locked dependency versions
- `.gitignore` -- ignores compiled output and orchestrator metadata

## Key File Locations

**Entry Points:**
- `src/main.rs`: Application entry point. Parses CLI args, spawns server thread, runs winit event loop.

**Configuration:**
- `Cargo.toml`: All dependency and build configuration.
- `src/main.rs:54-59`: CLI argument definition (port via `clap`).
- `src/main.rs:24-26`: Tuning constants (max commands per tick, channel capacity).

**Core Logic:**
- `src/server.rs:200-227`: Server startup and graceful shutdown (`run()`, `serve()` functions).
- `src/server.rs:54-174`: Router and Socket.IO handler construction (`build_router()`).
- `src/keyboard.rs:177-273`: OS input execution functions (TypeText, Backspace, Enter, SelectAll).
- `src/keyboard.rs:158-173`: Command merging optimization (`merge_commands()`).
- `src/main.rs:257-376`: QR code generation and window management.
- `src/main.rs:96-254`: Main event loop (entry point).

**Testing:**
- `src/server.rs:229-878`: Unit tests + E2E integration tests (co-located in `server.rs`).
- `src/keyboard.rs:277-456`: Unit tests for merge, enable/disable, backpressure (co-located in `keyboard.rs`).
- `src/ip.rs:22-49`: Unit tests for IP detection (co-located in `ip.rs`).
- `src/assets.rs:394-485`: Unit tests for HTML page content (co-located in `assets.rs`).
- `src/tray.rs:81-277`: QR structure verification tests (co-located in `tray.rs`).
- `src/main.rs:403-439`: Token generation tests (co-located in `main.rs`).

## Naming Conventions

**Files:**
- `snake_case.rs` for all Rust source files.
- `CamelCase` for asset files (`icon-on.svg`, `TypeBridge.png`).
- `kebab-case` for shell scripts (`build-mac.sh`).
- `Cargo.toml` and `Cargo.lock` follow Rust conventions.

**Rust identifiers:**
- `snake_case` for functions, variables, module names: `get_local_ip()`, `build_tray()`, `queue_type_text()`
- `CamelCase` for types and enums: `KeyCommand`, `TrayState`, `QrWindow`, `TrayEvent`
- `SCREAMING_SNAKE_CASE` for constants and statics: `MAX_COMMANDS_PER_TICK`, `KB_CHANNEL_BOUND`, `ENABLED`, `HISTORY_MAX`
- Module-level `static` and `OnceLock` globals use all-caps naming: `ENIGO`, `COMMAND_TX`, `ICON_ON`

**Directories:**
- All lowercase: `src/`, `assets/`, `scripts/`

## Where to Add New Code

**New Feature -- e.g., new keyboard command:**
1. Add new variant to `KeyCommand` enum in `src/keyboard.rs:16-21`
2. Add queue function in `src/keyboard.rs` (following pattern of `queue_type_text`)
3. Add execute function in `src/keyboard.rs` (following pattern of `execute_type_text`)
4. Handle in `execute()` match in `src/keyboard.rs:177-184`
5. Add Socket.IO event handler in `src/server.rs` (inside the `io.ns("/", ...)` closure)
6. Add web UI button/event in `src/assets.rs` HTML/JS
7. Update `merge_commands()` if the new command should be unmergeable

**New Feature -- e.g., additional HTTP route:**
1. Add route in `build_router()` at `src/server.rs:54-174` using `.route()`
2. Add handler function inline or as a standalone async fn
3. Tests go into the `mod tests` block at `src/server.rs:229-571`

**New Feature -- e.g., additional tray menu item:**
1. Add `MenuItem` in `build_tray()` at `src/tray.rs:29-78`
2. Clone the `.id()` for matching
3. Add match arm in `main.rs` `Event::UserEvent(TrayEvent::Menu(id))` handler at `src/main.rs:192-221`

**Utilities:**
- Pure helper functions: add to relevant module (e.g., `src/keyboard.rs` for input-related, `src/ip.rs` for network-related). Keep functions small and focused.

## Special Directories

**`scripts/`:**
- Purpose: Build and packaging scripts (currently macOS-only)
- Contains: `build-mac.sh` (bash), `Info.plist`
- Generated: No
- Committed: Yes

**`assets/icons/`:**
- Purpose: Source icon files for app bundling and documentation
- Contains: SVGs and multi-resolution PNGs
- Generated: Yes (icons are embedded as RGBA byte arrays in `src/tray_icons.rs` for the runtime, the PNGs are used by the macOS bundle script)
- Committed: Yes

---

*Structure analysis: 2026-06-08*
