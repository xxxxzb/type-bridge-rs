---
phase: 01-backend-core
plan: 04
type: execute
subsystem: server
tags:
  - http-api
  - auth
  - security-headers
  - tests
requires:
  - 01-01 (project skeleton)
  - 01-02 (keyboard module)
  - 01-03 (tray/UI)
provides:
  - HTTP API with Bearer auth
  - 4 REST routes (/, /api/status, /api/history, /api/commands)
  - Security headers on all responses
  - Unit + E2E test suite
affects:
  - Cargo.toml (remove socketioxide)
  - src/server.rs (full rewrite)
  - src/keyboard.rs (CommandResult, TestGuard, COMMAND_TX)
  - src/assets.rs (clean HTML, no socket.io)
  - src/main.rs (AppState, token gen, sync_channel)
  - src/tray.rs (+ tray_icons.rs) (show_qr_id, make_icon)
tech-stack:
  added:
    - axum 0.8 (continued use)
    - serial_test 3 (test isolation)
  removed:
    - socketioxide 0.16 (full removal)
  patterns:
    - Bearer token auth via axum middleware::from_fn_with_state
    - Security headers via axum middleware::from_fn
    - Router nesting for auth-scoped API routes
    - Bridge-channel pattern for test COMMAND_TX isolation
key-files:
  created:
    - (none — all files already existed)
  modified:
    - Cargo.toml
    - src/server.rs
    - src/keyboard.rs
    - src/assets.rs
    - src/main.rs
    - src/tray.rs
    - src/tray_icons.rs
decisions:
  - Use nest("/api", router) for auth middleware scoping (prevents 401 on unmatched routes)
  - Bridge pattern for test COMMAND_TX: unbounded channel with forwarding thread keeps sync_channel alive and never fills up
  - All handler return types use concrete `Resp` type alias (axum::response::Response<Body>) to avoid opaque impl IntoResponse resolution issues
  - serial_test::serial on all keyboard-modifying tests to prevent COMMAND_TX race conditions
metrics:
  duration: "~75 min (extensive debugging of std::sync::mpsc disconnect behavior)"
  completed: "2026-06-08"
  commits: 3
  tests_passing: 70
---

# Phase 1 Plan 4: Rewrite server.rs from Socket.IO to Pure HTTP API

Rewrite server.rs as a pure HTTP API by removing all Socket.IO code and introducing Bearer token auth, 4 REST routes, and security headers. Also updates keyboard.rs, main.rs, assets.rs, Cargo.toml, and tray modules to support the new architecture.

## One-liner

Removed socketioxide dependency, rewrote server.rs with Bearer token auth middleware (AUTH-05/07), 4 routes (API-01/02/03/04/05), security headers (SEC-01/02/03/04), and no CORS headers (AUTH-06). All 70 tests pass with clippy clean.

## Task Summary

### Task 1: Remove Socket.IO code and Cargo.toml dependency
Removed `socketioxide = "0.16"` from Cargo.toml, stripped all Socket.IO imports and handler code from server.rs, and updated build_router to take `&Arc<AppState>` and return `Router` directly.

### Task 2: Add Bearer token auth middleware and route handlers
Implemented auth middleware that validates `Authorization: Bearer <token>` headers on `/api/*` routes (AUTH-05) and rejects requests with `token=` query parameters (AUTH-07). Added 4 handlers:
- GET / (no auth) serves HTML page
- GET /api/status returns `{ enabled, version }` with auth
- GET /api/history returns entry list with auth
- POST /api/commands accepts type_text/enter/backspace/clear_pc_field with auth

### Task 3: Add security headers middleware
Added `security_headers` middleware applying Cache-Control: no-store, Referrer-Policy: no-referrer, X-Content-Type-Options: nosniff, Cross-Origin-Resource-Policy: same-origin to ALL responses. No CORS headers served.

### Task 4: Adapt unit tests and replace E2E tests
Removed all Socket.IO-specific tests, added 16 new unit tests covering auth (401 flows), commands (202/400/409/413/429), security headers, and no-CORS verification. Replaced Socket.IO E2E tests with HTTP equivalents: auth rejection, command flow, and history flow. Added `serial_test::serial` to all keyboard-modifying tests.

## Deviations from Plan

### Rule 2 - Missing critical functionality

**1. Supporting module updates required for compilation**
- **Found during:** Tasks 1-3
- **Issue:** The worktree branch was at an earlier state where keyboard.rs lacked `CommandResult`, `TestGuard`, `MAX_TEXT_LEN`, and `mpsc::SyncSender`. main.rs lacked token generation and AppState creation. assets.rs still had Socket.IO CDN + client code. tray.rs lacked `show_qr_id`.
- **Fix:** Updated all supporting modules to match the expected post-Wave-1 state (these were presumed to exist from earlier phases).
- **Files modified:** src/keyboard.rs, src/main.rs, src/assets.rs, src/tray.rs, src/tray_icons.rs
- **Commit:** 488d98f

### Rule 3 - Blocking issue

**2. std::sync::mpsc sync_channel receiver keeps disconnecting in tests**
- **Found during:** Task 4
- **Issue:** The `mpsc::sync_channel` used as COMMAND_TX in tests kept returning `Disconnected` when `try_send` was called, even though the receiver was stored in a static `OnceLock` (via `Mutex`, `Box::leak`, and `mem::forget` approaches). The root cause was never fully determined — the channel disconnected despite the receiver being alive in the static.
- **Fix:** Implemented a bridge-channel pattern: an unbounded `mpsc::channel` with its receiver stored in a `OnceLock` static. A bounded `sync_channel` feeds into it via a forwarding thread each time `init_test_globals()` is called. This avoids the disconnect issue entirely.
- **Commits:** 46a0267

**3. Handler return type mismatch**
- **Found during:** Task 4
- **Issue:** Mixing `impl IntoResponse` return types from helper functions with concrete `Response<Body>` from inline `.into_response()` caused type-resolution failures in match arms.
- **Fix:** Created a `Resp` type alias (`axum::response::Response<Body>`) and used it consistently for all handler, middleware, and helper function return types.
- **Commit:** 46a0267

**4. COMMAND_TX race between unit tests and E2E tests**
- **Found during:** Task 4
- **Issue:** Server unit tests (without TestGuard) and E2E tests (with TestGuard) both use the global COMMAND_TX. When they run concurrently, the unit test's commands go to the E2E test's test channel, causing unexpected command reads.
- **Fix:** Added `#[serial_test::serial]` to all keyboard-modifying tests (7 command tests + 3 E2E tests) to serialize their execution.
- **Commit:** 46a0267

### Rule 1 - Fixed clippy warnings
- `map_or(false, |q| ...)` simplified to `is_some_and(|q| ...)`
- `&auth[7..]` changed to `auth[7..]` (unnecessary reference)
- **Commit:** 2b0c0c2

## Auth Gates

None.

## Known Stubs

None. All routes are fully implemented with proper error handling and status codes.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: auth_bypass | src/server.rs | GET / has no auth — this is intentional (API-01: "no auth required for the page itself"). The token is embedded in the QR URL for the phone JS to read. This is NOT a threat flag for v0.3.0 because the page contains no sensitive data, only the phone UI. |

## Key Decisions

1. **nest vs merge for API routing**: Used `nest("/api", api_routes)` instead of `merge(api_routes)` to properly scope auth middleware to `/api/*`. With `merge`, the auth layer applied to ALL routes including `/`, causing 401 on unmatched routes. With `nest`, the auth layer only wraps paths under `/api`.

2. **Bridge-channel for test COMMAND_TX**: A bounded `sync_channel` forwarded to an unbounded `mpsc::channel` via a dedicated thread. The unbounded channel's receiver lives in a `OnceLock` static. This avoids the persistent "Disconnected" error from `std::sync::mpsc::sync_channel`.

3. **Concrete `Resp` alias over `impl IntoResponse`**: Using a concrete type alias avoids opaque type resolution issues when handlers return from multiple arms that call different helper functions.

4. **serial_test for test isolation**: All tests that call keyboard::queue_* functions are marked `#[serial_test::serial]` to prevent race conditions on the global COMMAND_TX.

## Verification

- [x] `cargo test` 70 passed
- [x] `cargo clippy` no issues
- [x] socketioxide absent from Cargo.toml
- [x] No Socket.IO imports in server.rs
- [x] Auth middleware validates Bearer token from Authorization header
- [x] GET / returns 200 with HTML (no auth)
- [x] GET /api/status, GET /api/history, POST /api/commands return correct status codes
- [x] Security headers on all responses (Cache-Control, Referrer-Policy, X-Content-Type-Options, Cross-Origin-Resource-Policy)
- [x] No Access-Control-* headers
- [x] Handlers use `State<Arc<AppState>>`
