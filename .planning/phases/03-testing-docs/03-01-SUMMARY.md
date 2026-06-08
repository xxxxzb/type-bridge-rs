---
phase: "03-testing-docs"
plan: "01"
tags: ["test-coverage", "cleanup", "dead-code"]
key-files:
  created: []
  modified:
    - src/keyboard.rs
    - src/server.rs
  deleted:
    - src/socket.io.min.js
decisions: []
metrics:
  tests-before: 82
  tests-after: 88
  duration: "~1 minute"
  completed-date: "2026-06-08"
---

# Phase 3 Plan 1: Close Test Coverage Gaps — Summary

Close 6 test coverage gaps across keyboard.rs and server.rs, and remove dead socket.io.min.js file. Auth coverage completed for all 3 API routes (status, commands, history) — no token, wrong token, and correct token paths verified. Command coverage completed with clear_pc_field E2E test verifying SelectAll+Backspace. Edge case coverage completed with full-queue 429 test. Unit coverage completed for queue_select_all paused state.

## Test Coverage Changes

| Test | File | Type | Status |
|------|------|------|--------|
| `test_queue_select_all_returns_paused_when_disabled` | `src/keyboard.rs` | Unit | Added |
| `test_api_commands_unauthorized_without_header` | `src/server.rs` | Integration | Added |
| `test_api_commands_unauthorized_with_wrong_token` | `src/server.rs` | Integration | Added |
| `test_api_history_unauthorized_with_wrong_token` | `src/server.rs` | Integration | Added |
| `test_e2e_commands_full_queue_returns_429` | `src/server.rs` | E2E | Added |
| `test_e2e_clear_pc_field_produces_select_all_and_backspace` | `src/server.rs` | E2E | Added |

## Tasks Executed

### Task 1: Add queue_select_all paused unit test

Added `test_queue_select_all_returns_paused_when_disabled` in keyboard.rs `mod tests`, following the established pattern of test_queue_backspace/enter/type_text paused-when-disabled tests. Verifies queue_select_all() returns CommandResult::Paused when ENABLED is false.

### Task 2: Add 5 server tests (3 integration + 2 E2E)

**Integration tests (mod tests, uses test_router + oneshot):**
- `test_api_commands_unauthorized_without_header`: POST /api/commands with no Authorization header -> 401
- `test_api_commands_unauthorized_with_wrong_token`: POST /api/commands with `Bearer wrong` -> 401
- `test_api_history_unauthorized_with_wrong_token`: GET /api/history with `Bearer wrong` -> 401

**E2E tests (mod e2e_tests, uses spawn_server + TcpStream):**
- `test_e2e_commands_full_queue_returns_429`: Creates 1-capacity sync_channel, fills it, then sends second command expecting 429
- `test_e2e_clear_pc_field_produces_select_all_and_backspace`: Posts clear_pc_field, verifies SelectAll + Backspace are queued in order

### Task 3: Delete socket.io.min.js

Removed 48.8KB dead file (leftover from Socket.IO era). Verified file has no references in source code. Ran full suite (88 tests all pass) and cargo clippy (clean).

## Deviations from Plan

None — plan executed exactly as written.

## Test Results

```
cargo test: 88 passed (1 suite)
cargo clippy: No issues found
```

## Plan Commits

- `73154ed` test(03-testing-docs): add queue_select_all returns paused when disabled
- `5073875` test(03-testing-docs): add 5 server tests for auth, full-queue 429, and clear_pc_field
- `f4d73ac` chore(03-testing-docs): remove dead socket.io.min.js file

## Requirements Fulfilled

- TEST-01: queue_select_all returns Paused when disabled ✓
- TEST-02: POST /api/commands without Authorization returns 401 ✓
- TEST-03: POST /api/commands with wrong token returns 401 ✓
- TEST-04: GET /api/history with wrong token returns 401 ✓
- TEST-05: POST /api/commands returns 429 when queue is full ✓
- TEST-06: POST /api/commands clear_pc_field produces SelectAll + Backspace ✓
- TEST-07: cargo test all pass (88 tests) ✓
- TEST-08: cargo clippy clean ✓
- TEST-09: src/socket.io.min.js removed ✓

## Self-Check: PASSED
