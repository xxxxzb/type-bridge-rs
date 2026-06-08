---
phase: 02-frontend-rewrite
plan: 01
type: execute
subsystem: keyboard, server
tags:
  - backspace-fix
  - csp
  - security-headers
  - keyboard
  - server
requires:
  - 01-04 (HTTP API with auth and merged commands)
provides:
  - execute_backspace sends plain single Backspace click (no Alt/Ctrl modifier)
  - Content-Security-Policy header on all HTTP responses
affects:
  - src/keyboard.rs (execute_backspace simplified)
  - src/server.rs (CSP header + test added)
tech-stack:
  added: []
  removed: []
  patterns:
    - Middleware-fn for injecting security headers on all responses
    - Single-key click via enigo.key(Key::Backspace, Direction::Click)
key-files:
  created: []
  modified:
    - src/keyboard.rs
    - src/server.rs
decisions:
  - CSP uses 'unsafe-inline' for script-src because JS is embedded in assets.rs HTML const (LAN-local tool, pragmatic choice)
  - CSP includes explicit connect-src 'self' override despite default-src 'self' for clarity
metrics:
  duration: "~5 min"
  completed: "2026-06-08"
  commits: 2
  tests_passing: 72
---

# Phase 2 Plan 1: Fix Backspace Behavior and Add CSP Header

Fix execute_backspace to send a plain single-character Backspace (no modifier keys) per RUST-04. Add Content-Security-Policy header to all responses per SEC-05.

## One-liner

Removed Alt/Ctrl modifier logic from execute_backspace so it sends plain `Key::Backspace` click. Added `Content-Security-Policy` header (default-src 'self'; script-src 'unsafe-inline'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none') to all responses via security_headers middleware. 72 tests pass, clippy clean.

## Task Summary

### Task 1: Fix execute_backspace to send plain Backspace without modifier

In `src/keyboard.rs`, removed the `mod_key` detection (`#[cfg(target_os = "macos")] Key::Alt` / `#[cfg(not(target_os = "macos"))] Key::Control`), the `enigo.key(mod_key, Direction::Press)`, and the `enigo.key(mod_key, Direction::Release)` calls. The function now only calls `enigo.key(Key::Backspace, Direction::Click)`. `execute_select_all()` and `execute_type_text()` were left unchanged as they still need Meta/Control modifiers.

- **Commit:** 0832c39
- **Files modified:** src/keyboard.rs

### Task 2: Add Content-Security-Policy header to security_headers middleware

In `src/server.rs`, added a `Content-Security-Policy` header insertion to the `security_headers` middleware after the existing four headers (Cache-Control, Referrer-Policy, X-Content-Type-Options, Cross-Origin-Resource-Policy). Added a new `test_csp_header_present` async test that verifies all CSP directives are present.

Policy values:
- `default-src 'self'` -- all resources from same origin
- `script-src 'unsafe-inline'` -- required for embedded JS in assets.rs HTML const
- `connect-src 'self'` -- fetch() to same-origin API only
- `base-uri 'none'` -- prevent base tag injection
- `frame-ancestors 'none'` -- prevent clickjacking
- `form-action 'none'` -- block form submissions (all API calls use fetch)

- **Commit:** cb744dd
- **Files modified:** src/server.rs

## Deviations from Plan

None -- plan executed exactly as written.

## Auth Gates

None.

## Known Stubs

None.

## Threat Flags

No new security-relevant surface introduced beyond the CSP header (threat model T-02-01 through T-02-SC cover all).

## Key Decisions

1. **CSP 'unsafe-inline' for scripts**: Required because the JS is embedded directly in the HTML const string in assets.rs. This is a LAN-local tool where the risk of XSS injection is minimal and controlled by local auth. Pragmatic tradeoff.

2. **Explicit connect-src 'self' despite default-src 'self'**: Redundant with default-src but included for clarity and defense-in-depth when other directives are added in the future.

## Verification

- [x] `cargo test` 72 passed (17 keyboard tests + 28 server tests + 27 others)
- [x] `cargo clippy` no issues
- [x] `execute_backspace()` has no modifier key references -- only `Key::Backspace` click
- [x] `Content-Security-Policy` header present on all responses
- [x] CSP values contain all required directives
