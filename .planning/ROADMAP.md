# Roadmap: TypeBridge v1.0 — HTTP-Only Refactor

**Milestone:** v1.0
**Granularity:** Standard
**Created:** 2026-06-08
**Core Value:** 手机打字 → PC 输入，零摩擦

## Phases

- [x] **Phase 1: Backend Core** — Serve pure HTTP API with Bearer token auth, security headers, convergent AppState; remove Socket.IO
- [ ] **Phase 2: Frontend Rewrite** — Rewrite phone UI with fetch + Promise queue, correct button semantics, Bearer token from sessionStorage, status polling and history from HTTP API
- [ ] **Phase 3: Testing & Documentation** — Comprehensive test coverage and accurate docs for HTTP-only architecture

## Phase Details

### Phase 1: Backend Core

**Goal:** Server runs on pure HTTP (no Socket.IO) with Bearer token auth, security headers on all responses, and convergent AppState managing global state.

**Depends on:** Nothing (first phase, bootstrap)

**Requirements:** API-01, API-02, API-03, API-04, API-05, AUTH-01, AUTH-02, AUTH-05, AUTH-06, AUTH-07, SEC-01, SEC-02, SEC-03, SEC-04, RUST-01, RUST-02, RUST-03, RUST-06, RUST-07, FE-01, FE-02

**Success Criteria** (what must be TRUE):

1. User can start the server; `GET /` returns the phone HTML page (no auth required); socketioxide is absent from both `Cargo.toml` and runtime
2. `GET /api/status` returns `{ enabled, version }` with valid Bearer token; returns 401 without Authorization header, with wrong token, or with token in query parameter
3. `GET /api/history` returns recent input history with valid Bearer token; returns 401 without valid auth
4. `POST /api/commands` accepts `type_text`, `enter`, `backspace`, `clear_pc_field` commands via JSON body; returns 202 on queue success, 400 for empty text, 409 when paused, 413 for text >10,000 chars, 429 when queue is full
5. All HTTP responses include security headers: `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff`, `Cross-Origin-Resource-Policy: same-origin`; no CORS headers (`Access-Control-*`) are served

**Plans:** 4 plans in 2 waves

```
Wave 1 (parallel):
  [x] 01-01-PLAN.md — AppState + main.rs wiring (RUST-02, RUST-07)
  [x] 01-02-PLAN.md — keyboard.rs CommandResult refactor (RUST-06)
  [x] 01-03-PLAN.md — Strip Socket.IO from assets.rs (FE-01, FE-02)

Wave 2 (depends on W1):
  [x] 01-04-PLAN.md — server.rs HTTP-only rewrite + auth + routes + security headers (API-xx, AUTH-xx, SEC-xx, RUST-01/03)
```

---

### Phase 2: Frontend Rewrite

**Goal:** Phone UI sends commands via `fetch()` with a serial Promise queue, all buttons have correct semantics, auth uses `sessionStorage` + `Authorization: Bearer` header, status pill polls `/api/status`, history populates from `/api/history`.

**Depends on:** Phase 1 (backend must serve HTTP API before frontend can use it)

**Requirements:** AUTH-03, AUTH-04, SEC-05, RUST-04, RUST-05, FE-03, FE-04, FE-05, FE-06, FE-07, FE-08, FE-09, FE-10, FE-11

**Success Criteria** (what must be TRUE):

1. User scans QR code; page reads token from `location.search`, stores in `sessionStorage`, removes from address bar via `history.replaceState()`; all `/api/*` fetch calls include `Authorization: Bearer <token>` header
2. User types text and presses "Send to PC"; `fetch()` sends `POST /api/commands` with `type_text` command; phone textarea clears on 202 response
3. "Backspace" sends single-character `backspace` command (not Option/Ctrl+Backspace); "Enter" sends `enter` command; "Clear text" clears phone textarea locally without any HTTP request
4. "Clear PC field" is visually distinct (styled as remote/dangerous action); sends `clear_pc_field` command; executes as SelectAll + Backspace on the PC
5. Status pill periodically polls `GET /api/status` and shows enabled/paused state; history list populates from `GET /api/history` response; all commands execute serially (Promise queue ensures no concurrent fetch races)

**Plans:** 2 plans

```
Wave 1 (parallel):
  [ ] 02-01-PLAN.md — Rust backend fixes: backspace plain key + CSP header (RUST-04, SEC-05)
  [ ] 02-02-PLAN.md — Frontend HTML/JS rewrite: fetch, Promise queue, correct buttons, sessionStorage auth, status polling, history (FE-03 through FE-11)
```

---

### Phase 3: Testing & Documentation

**Goal:** All tests pass (unit, edge case, history, frontend HTML), Socket.IO E2E tests replaced with HTTP equivalents, README reflects HTTP-only architecture.

**Depends on:** Phase 2 (tests must verify the complete frontend + backend behavior)

**Requirements:** TEST-01, TEST-02, TEST-03, TEST-04, TEST-05, TEST-06, TEST-07, TEST-08, TEST-09, TEST-10, DOC-01, DOC-02, DOC-03

**Success Criteria** (what must be TRUE):

1. `cargo test` passes: auth tests (no token → 401, wrong token → 401, correct token → pass), command tests (type_text, enter, backspace, clear_pc_field produce correct `KeyCommand`)
2. Edge case tests pass: paused state returns 409, full queue returns 429, empty text returns 400, text >10,000 chars returns 413
3. History tests pass: write to history, consecutive duplicate dedup, 30-entry cap all produce correct results
4. Frontend HTML tests verify: no socket.io JS present, uses `fetch()` for all API calls, `sessionStorage` for token, `Authorization: Bearer` header, `history.replaceState()` token cleanup, Promise-based serial command queue
5. README is updated: architecture diagram shows axum HTTP (no Socket.IO), button table distinguishes "Clear text" (local) vs "Clear PC field" (remote), "与 Python 原版区别" section removes WebSocket/Socket.IO references

**Plans:** TBD

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Backend Core | 4/4 | Complete | 2026-06-08 |
| 2. Frontend Rewrite | 2 planned | Planning | - |
| 3. Testing & Documentation | 0/0 | Not started | - |

---

*Created: 2026-06-08 for milestone v1.0*
