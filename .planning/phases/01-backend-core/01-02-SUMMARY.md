---
phase: 01-backend-core
plan: 02
subsystem: keyboard
tags: [rust, mpsc, sync-channel, command-result]
requires:
  - phase: 01-01
    provides: keyboard module with KeyCommand enum, queue functions, execute loop
provides:
  - CommandResult enum with Queued/Paused/Full/TooLong variants
  - Refactored queue functions returning CommandResult instead of ()
  - queue_select_all function and KeyCommand::SelectAll variant
  - Bounded mpsc::SyncSender channel for full-queue detection
  - Serialized tests for global-state-dependent keyboard tests
affects: [01-03, 01-04]

tech-stack:
  added:
    - serial_test = "3" (test-only cargo dep)
  patterns:
    - Queue functions return CommandResult enum instead of silently dropping
    - Bounded channel (SyncSender) with try_send for backpressure
    - Global state tests isolated via serial_test crate

key-files:
  created: []
  modified:
    - src/keyboard.rs
    - src/main.rs
    - src/server.rs
    - Cargo.toml

key-decisions:
  - "Changed mpsc::Sender to mpsc::SyncSender for bounded queue semantics"
  - "Changed COMMAND_TX from OnceLock<Mutex<...>> to Mutex<Option<...>> for test replaceability"
  - "Added KeyCommand::SelectAll variant and queue_select_all stub in execute"
  - "Removed empty-text guard from keyboard module (moved to server handler responsibility)"
  - "Used serial_test crate for global-state test isolation instead of manual synchronization"
  - "MAX_TEXT_LEN = 10,000 chars as the TooLong threshold"

patterns-established:
  - "Queue API: entry functions check enabled state and length guard, delegate to send_command"
  - "send_command: acquires channel lock, calls try_send, maps result to CommandResult"
  - "execute: exhaustive match on KeyCommand — unchanged for existing variants"

requirements-completed: [RUST-06]

duration: 32min
completed: 2026-06-08
---

# Phase 01 Plan 02: CommandResult Return Type and Bounded Queue Summary

**Refactored keyboard queue API to return CommandResult enum (Queued/Paused/Full/TooLong) using bounded mpsc::SyncSender, added queue_select_all, adapted all tests for global-state-aware serialization**

## Performance

- **Duration:** 32 min
- **Started:** 2026-06-08T... (wave start)
- **Completed:** 2026-06-08
- **Tasks:** 2 (Task 1 TDD: RED + GREEN phases, Task 2 inline)
- **Files modified:** 4

## Accomplishments

- CommandResult enum with 4 variants (Queued, Paused, Full, TooLong) defined and exported
- All queue functions (queue_type_text, queue_backspace, queue_enter, queue_select_all) return CommandResult
- send_command maps try_send outcomes: Ok -> Queued, Full -> Full, Disconnected -> Full
- Disabled state returns CommandResult::Paused for all queue functions
- Text > 10,000 chars returns CommandResult::TooLong (with warning log)
- Channel changed to bounded mpsc::SyncSender (capacity 128) for backpressure detection
- KeyCommand::SelectAll variant added for future queue_select_all server wiring
- queue_select_all function defined, wired to send_command(KeyCommand::SelectAll)
- serial_test crate added for reliable parallel-test execution of global-state-dependent tests
- All 36 tests pass (13 keyboard + 23 server), 0 clippy errors

## Task Commits

Each task was committed atomically:

1. **Task 1 (TDD RED): Add failing tests for CommandResult queue API** — `8a27906`
2. **Task 1 (TDD GREEN): Add CommandResult enum and refactor queue functions** — `6a41b15`
3. **Task 2: Adapt keyboard tests for CommandResult** — completed inline within Task 1 (tests were rewritten during RED/GREEN phases)

## Files Created/Modified

- `src/keyboard.rs` — Core changes: CommandResult enum, MAX_TEXT_LEN, SyncSender, refactored queue functions, SelectAll variant, serialized tests
- `src/main.rs` — Changed mpsc::channel() to mpsc::sync_channel(128) for bounded queue
- `src/server.rs` — Suppressed unused CommandResult return values (pattern: `let _ = ...`)
- `Cargo.toml` / `Cargo.lock` — Added serial_test = "3" dev-dependency

## Decisions Made

- Used `Mutex<Option<mpsc::SyncSender>>` instead of `OnceLock<Mutex<mpsc::Sender>>` to allow the channel to be replaced in tests (test isolation)
- Added `KeyCommand::SelectAll` despite plan saying "KeyCommand unchanged" — needed for queue_select_all; existing variants (TypeText, Backspace, Enter) are unmodified
- MAX_TEXT_LEN = 10,000 was chosen as a practical upper bound for paste operations
- Channel buffer capacity = 128 chosen as a default — enough for burst typing without unbounded memory growth
- Used `serial_test` crate for reliable parallel test execution rather than complicated manual synchronization of global state

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Channel type mismatch — mpsc::Sender vs SyncSender**
- **Found during:** Task 1 (GREEN implementation)
- **Issue:** Plan's `send_command` uses `try_send` with `TrySendError::Full`, but the existing channel is unbounded `mpsc::Sender` which never returns Full. The test `test_queue_returns_full_when_channel_full` requires Full to be reachable.
- **Fix:** Changed `COMMAND_TX` from `OnceLock<Mutex<mpsc::Sender<KeyCommand>>>` to `Mutex<Option<mpsc::SyncSender<KeyCommand>>>`. Changed `main.rs` from `mpsc::channel()` to `mpsc::sync_channel(128)`. Changed `init_command_queue` parameter type.
- **Files modified:** src/keyboard.rs, src/main.rs
- **Verification:** `test_queue_returns_full_when_channel_full` passes (capacity-1 channel fills and returns Full on second try_send)
- **Committed in:** 6a41b15 (GREEN commit)

**2. [Rule 3 - Blocking] Missing SelectAll variant in KeyCommand**
- **Found during:** Task 1 (GREEN implementation)
- **Issue:** Plan's action code includes `queue_select_all` calling `KeyCommand::SelectAll`, but the enum doesn't have a SelectAll variant. Would not compile.
- **Fix:** Added `SelectAll` variant to `KeyCommand`. Added handler in `execute()` with `tracing::warn!` stub. Added unreachable arm in `test_command_queue_full_flow` match. Plan's must_have "KeyCommand unchanged" refers to existing variants (TypeText, Backspace, Enter) which are indeed unchanged — SelectAll is an addition.
- **Files modified:** src/keyboard.rs
- **Verification:** Code compiles, all tests pass, clippy reports SelectAll as "never constructed" (expected — server wiring is in Plan 04)
- **Committed in:** 6a41b15 (GREEN commit)

**3. [Rule 3 - Blocking] server.rs type mismatch — queue functions now return CommandResult**
- **Found during:** Task 1 (GREEN verification)
- **Issue:** Server socket handlers call queue functions and their return values produce "incompatible types" error (CommandResult vs () in match arms)
- **Fix:** Added `let _ = ...` discard to all queue function calls in server socket handlers
- **Files modified:** src/server.rs
- **Verification:** Code compiles, server tests pass
- **Committed in:** 6a41b15 (GREEN commit)

**4. [Rule 3 - Blocking] Parallel test execution races on global ENABLED state**
- **Found during:** Task 1 (GREEN verification, full test suite)
- **Issue:** Tests share global `ENABLED` atomic and `COMMAND_TX` mutex. Parallel execution causes `test_queue_returns_full_when_channel_full` to see wrong enabled state (set by concurrently running test).
- **Fix:** Added `serial_test = "3"` dev-dependency. Marked all 9 global-state-dependent tests with `#[serial]` (enable/disable, Paused, TooLong, Full, Queued). Raw channel tests remain parallel-safe.
- **Files modified:** Cargo.toml, Cargo.lock, src/keyboard.rs
- **Verification:** Full `cargo test` pass (36 tests) with zero race-condition failures
- **Committed in:** 6a41b15 (GREEN commit)

---

**Total deviations:** 4 auto-fixed (4 Rule 3 — blocking)
**Impact on plan:** All fixes essential for correctness. No scope creep — bounded channel, SelectAll variant, server callers, and test isolation are all necessary for the plan's stated requirements to work correctly.

## Known Stubs

- `KeyCommand::SelectAll => tracing::warn!(...)` in `execute()` — SelectAll keyboard execution not yet implemented. A later plan will wire actual behavior (Cmd+A on macOS, Ctrl+A on Windows/Linux).
- `queue_select_all` is defined but unused — planned for server handler wiring in Plan 04.

Both stubs are intentional forward-looking additions included per the plan's full scope.

## Issues Encountered

- The plan's provided `send_command` code in `<action>` had a syntax error: `COMMAND_TX.lock()` was written as if `COMMAND_TX` were a Mutex, but it is a `OnceLock<Mutex<...>>`. Fixed by restructuring to get the OnceLock first, then lock the inner Mutex.
- The plan's action omitted empty-text handling — intentionally removed from keyboard module (moved to server handler responsibility per the plan's commentary).

## Next Phase Readiness

- Queue API is ready for server handler integration (Plan 04) — all functions return typed CommandResult, enabling correct HTTP status codes
- Plan 03 can proceed independently for tray enhancements
- Channel type changed; main.rs callers already updated

---
*Phase: 01-backend-core*
*Completed: 2026-06-08*
