---
phase: 02-frontend-rewrite
plan: 02
type: execute
subsystem: frontend (assets)
tags:
  - fetch-api
  - sessionStorage
  - promise-queue
  - history
  - inline-js
requires:
  - 01-04 (HTTP API with auth — provides /api/status, /api/history, /api/commands)
  - 02-01 (CSP header allows 'unsafe-inline' for embedded JS)
provides:
  - Complete inline JavaScript frontend with sessionStorage token, fetch() API calls, Promise command queue
  - 2x2 button grid (backspace, Clear text, Clear PC field, enter)
  - History section with click-to-fill entries
  - Status polling (GET /api/status every 5s)
  - 18 HTML content tests
affects:
  - src/assets.rs (full rewrite of HTML const + tests)
tech-stack:
  added: []
  removed: []
  patterns:
    - Promise chain queue for serializing fetch() calls
    - sessionStorage + history.replaceState for Bearer token management
    - api() wrapper adding Authorization header to all requests
    - DOMContentLoaded wrapper for safe DOM access
key-files:
  created: []
  modified:
    - src/assets.rs
decisions:
  - enqueue() handles toasts internally (fire-and-forget) so callers never chain off the returned promise, avoiding the classic Promise queue race condition where caller .then() and next enqueue both chain off the same previous promise
  - No onclick attributes — all event binding via addEventListener (enforced by test)
  - Status polling uses 5s interval (matching the original TypeBridge behavior)
metrics:
  duration: "~20 min"
  completed: "2026-06-08"
  commits: 1
  tests_passing: 82
---

# Phase 2 Plan 2: Frontend Rewrite with Inline JavaScript

Complete frontend rewrite: inline JavaScript with sessionStorage token handling, fetch() API with Bearer auth, Promise-based command queue for serial execution, 2x2 button layout, status polling, and history section with click-to-fill.

## One-liner

Rewrote the blank HTML (from Phase 1) into a working frontend with sessionStorage token extraction + history.replaceState cleanup, fetch() API helper with Authorization Bearer header, Promise chain queue for serialized command execution, 4-button 2x2 grid (backspace, Clear text, Clear PC field, enter), status polling via setInterval, history section populated from /api/history, toast notifications, Enter-to-send keyboard handling, and 18 content tests — all inline, no dependencies, 82 tests passing, clippy clean.

## Performance

- **Duration:** ~20 min
- **Started:** 2026-06-08
- **Completed:** 2026-06-08
- **Tasks:** 2 (both in one commit — same file)
- **Files modified:** 1
- **Commits:** 1

## Task Commits

1. **Task 1+2: Rewrite HTML with inline JS, update tests** — `facde14` (feat)

   Combined into one commit since both tasks modify `src/assets.rs`. Includes:
   - Complete inline `<script>` with JS (token, fetch, Promise queue, buttons, status, history)
   - Updated CSS (`.act.local`, `.history-box`, `.history-list`, `.history-item`, `.hidden`, removed `.act.enter` grid-column span)
   - Updated HTML (history section, 2x2 buttons, Clear text + Clear PC field)
   - Updated + new tests (18 total)

## Files Modified

- `src/assets.rs` — Full rewrite of HTML const (CSS + structure + JS) and test module

## Decisions Made

1. **Fire-and-forget enqueue pattern**: The `enqueue()` function handles toasts internally and callers don't chain `.then()` off the returned promise. This avoids the classic Promise queue race condition where a caller's `.then(sideEffect)` and the next `enqueue(fetch)` both become siblings chained to the same previous queue promise, causing concurrent execution.

2. **All event binding via addEventListener**: No `onclick` attributes in HTML, enforced by `test_html_contains_no_onclick` test.

3. **Status polling at 5s interval**: Matches the original TypeBridge behavior and the /api/status endpoint's polling expectation.

## Deviations from Plan

None — plan executed exactly as written. Both Task 1 (HTML rewrite) and Task 2 (test updates) were completed in a single commit since they modify the same file.

## Auth Gates

None — no authentication setup required for this plan.

## Known Stubs

None — all features fully implemented.

## Threat Flags

No new security-relevant surface introduced. The threat model (T-02-04 through T-02-07) covers the token handling and fetch pattern, all accounted for.

## Key Decisions

1. **Fire-and-forget enqueue**: Callers do NOT chain off enqueue's return value, preventing the race condition where a caller's `.then()` and the next `enqueue().then(fetch)` both chain as siblings off the same previous queue promise.

2. **Composite commit**: Task 1 (rewrite) and Task 2 (tests) combined into one commit since both modify `src/assets.rs` and the test changes are inherent to the rewrite.

## Verification

- [x] `cargo test` — 82 passed (all tests)
- [x] `cargo clippy` — no issues
- [x] JS reads token from URL, stores in sessionStorage, calls history.replaceState
- [x] All API calls use Authorization: Bearer header (via api() helper)
- [x] Promise chain queue serializes commands (enqueue chains onto previous)
- [x] 4 buttons in 2x2 grid: backspace, Clear text (local), Clear PC field (danger), enter
- [x] Status pill polls /api/status every 5s
- [x] History loads from /api/history on page load and after each send
- [x] Enter key = Send, Shift+Enter = newline
- [x] No onclick attributes — all addEventListener
- [x] 18 HTML content tests pass (sessionStorage, fetch, replaceState, Authorization Bearer, .then(), setInterval, history-box, Clear text, Clear PC field, no onclick, no socket.io)

---

*Phase: 02-frontend-rewrite*
*Completed: 2026-06-08*
