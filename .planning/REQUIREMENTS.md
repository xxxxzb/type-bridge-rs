# Requirements: TypeBridge

**Defined:** 2026-06-08
**Core Value:** 手机打字 → PC 输入，零摩擦

## v1 Requirements — HTTP-Only Refactor

Requirements for moving from Socket.IO to HTTP API.

### API

- [ ] **API-01**: `GET /` returns static phone page — no auth required for the page itself
- [ ] **API-02**: `GET /api/status` returns `{ enabled, version }` — auth required
- [ ] **API-03**: `GET /api/history` returns recent input history — auth required
- [ ] **API-04**: `POST /api/commands` accepts tagged JSON commands via `{ "type": "...", ... }`
- [ ] **API-05**: Correct HTTP status codes: 202 queued, 400 bad request, 401 unauthorized, 409 paused, 413 too long, 429 queue full

### Auth

- [ ] **AUTH-01**: 128-bit token generated at startup (existing `generate_token()` retained)
- [ ] **AUTH-02**: QR code URL includes `?token=...` for initial page load
- [ ] **AUTH-03**: Page JS reads token from `location.search` → `sessionStorage`
- [ ] **AUTH-04**: JS uses `history.replaceState()` to remove token from address bar
- [ ] **AUTH-05**: All `/api/*` requests use `Authorization: Bearer <token>` header
- [ ] **AUTH-06**: No CORS headers served (browser blocks cross-origin reads by default)
- [ ] **AUTH-07**: API rejects token passed as query parameter

### Security

- [ ] **SEC-01**: `Cache-Control: no-store` on all responses
- [ ] **SEC-02**: `Referrer-Policy: no-referrer`
- [ ] **SEC-03**: `X-Content-Type-Options: nosniff`
- [ ] **SEC-04**: `Cross-Origin-Resource-Policy: same-origin`
- [ ] **SEC-05**: CSP that does not break existing inline CSS/JS: `default-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'`

### Frontend

- [ ] **FE-01**: Remove all Socket.IO client code from HTML
- [ ] **FE-02**: Remove `<script src="/sio.min.js">` and `src/socket.io.min.js`
- [ ] **FE-03**: Replace `socket.emit()` calls with `fetch()` to `/api/*`
- [ ] **FE-04**: Promise-based command queue — commands execute serially, not concurrently
- [ ] **FE-05**: "Send to PC" sends `type_text`, clears phone textarea on 202
- [ ] **FE-06**: "Backspace" sends `backspace` (single char, not Option/Ctrl+Backspace)
- [ ] **FE-07**: "Enter" sends `enter`
- [ ] **FE-08**: "Clear text" only clears phone textarea — no HTTP request
- [ ] **FE-09**: "Clear PC field" sends `clear_pc_field` — visually distinct as remote/dangerous action
- [ ] **FE-10**: Status pill polls `/api/status` (no longer depends on socket connected/disconnected events)
- [ ] **FE-11**: History populated from `/api/history` response

### Rust Backend

- [ ] **RUST-01**: Remove `socketioxide` from `Cargo.toml`
- [ ] **RUST-02**: Introduce `AppState` struct holding token, enabled, history, command_tx
- [ ] **RUST-03**: Server handlers use `axum::extract::State<Arc<AppState>>`
- [ ] **RUST-04**: `execute_backspace` sends plain `Key::Backspace` click (no Alt/Ctrl modifier)
- [ ] **RUST-05**: `clear_pc_field` = SelectAll + Backspace (existing behavior, kept)
- [ ] **RUST-06**: `keyboard` module retains `KeyCommand`, `merge_commands`, `execute` — queue API refactored to expose result (queued/paused/full/too_long)
- [ ] **RUST-07**: `main.rs` retains `sync_channel` + winit event loop drain/merge/execute pattern

### Testing

- [ ] **TEST-01**: `cargo test` all pass
- [ ] **TEST-02**: Auth tests: no Authorization → 401, wrong token → 401, correct token → pass
- [ ] **TEST-03**: Command tests: type_text, enter, backspace, clear_pc_field → correct KeyCommand queued
- [ ] **TEST-04**: Paused state → POST /api/commands returns 409
- [ ] **TEST-05**: Full queue → POST /api/commands returns 429
- [ ] **TEST-06**: Empty text → POST /api/commands returns 400
- [ ] **TEST-07**: Text > 10,000 chars → POST /api/commands returns 413
- [ ] **TEST-08**: History write, consecutive-dedup, 30-entry cap all correct
- [ ] **TEST-09**: Frontend HTML tests: no socket.io, uses fetch, sessionStorage, Authorization Bearer, history.replaceState, command queue
- [ ] **TEST-10**: Socket.IO E2E tests removed/replaced with HTTP equivalents

### Documentation

- [ ] **DOC-01**: README arch diagram updated (axum HTTP, no Socket.IO)
- [ ] **DOC-02**: Button table updated: Clear text vs Clear PC field distinction
- [ ] **DOC-03**: "与 Python 原版区别" section updated — remove WebSocket/Socket.IO references

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time push (SSE/WebSocket reconnect) | HTTP polling sufficient for status |
| Multi-client support | Single-user use case |
| Cookie-based auth | Adds CSRF risk |
| External CSS/JS files | Single-file simplicity is a feature |
| i18n/localization | Not needed for current user base |
| Config file | CLI args sufficient |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| API-01 | Phase 1 | Pending |
| API-02 | Phase 1 | Pending |
| API-03 | Phase 1 | Pending |
| API-04 | Phase 1 | Pending |
| API-05 | Phase 1 | Pending |
| AUTH-01 | Phase 1 | Pending |
| AUTH-02 | Phase 1 | Pending |
| AUTH-03 | Phase 2 | Pending |
| AUTH-04 | Phase 2 | Pending |
| AUTH-05 | Phase 1 | Pending |
| AUTH-06 | Phase 1 | Pending |
| AUTH-07 | Phase 1 | Pending |
| SEC-01 | Phase 1 | Pending |
| SEC-02 | Phase 1 | Pending |
| SEC-03 | Phase 1 | Pending |
| SEC-04 | Phase 1 | Pending |
| SEC-05 | Phase 2 | Pending |
| RUST-01 | Phase 1 | Pending |
| RUST-02 | Phase 1 | Pending |
| RUST-03 | Phase 1 | Pending |
| RUST-04 | Phase 2 | Pending |
| RUST-05 | Phase 2 | Pending |
| RUST-06 | Phase 1 | Pending |
| RUST-07 | Phase 1 | Pending |
| FE-01 | Phase 1 | Pending |
| FE-02 | Phase 1 | Pending |
| FE-03 | Phase 2 | Pending |
| FE-04 | Phase 2 | Pending |
| FE-05 | Phase 2 | Pending |
| FE-06 | Phase 2 | Pending |
| FE-07 | Phase 2 | Pending |
| FE-08 | Phase 2 | Pending |
| FE-09 | Phase 2 | Pending |
| FE-10 | Phase 2 | Pending |
| FE-11 | Phase 2 | Pending |
| TEST-01 | Phase 3 | Pending |
| TEST-02 | Phase 3 | Pending |
| TEST-03 | Phase 3 | Pending |
| TEST-04 | Phase 3 | Pending |
| TEST-05 | Phase 3 | Pending |
| TEST-06 | Phase 3 | Pending |
| TEST-07 | Phase 3 | Pending |
| TEST-08 | Phase 3 | Pending |
| TEST-09 | Phase 3 | Pending |
| TEST-10 | Phase 3 | Pending |
| DOC-01 | Phase 3 | Pending |
| DOC-02 | Phase 3 | Pending |
| DOC-03 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 47 total
- Mapped to phases: 47
- Unmapped: 0 ✓

---
*Requirements defined: 2026-06-08*
*Last updated: 2026-06-08 after milestone v1.0 spec*
