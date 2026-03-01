---
title: "refactor: Split lib.rs into focused Rust modules"
type: refactor
status: completed
date: 2026-03-01
---

# refactor: Split lib.rs into focused Rust modules

## Overview

`src-tauri/src/lib.rs` is 1473 lines containing all backend logic: data models, settings I/O, keychain access, OAuth PKCE, token refresh, usage API fetching/parsing, Tauri commands, app update system, and tray setup. Split it into modules with clear responsibilities so each file is easy to navigate and modify independently.

## Problem Statement / Motivation

- **Navigability:** Finding code requires scrolling through 1400+ lines guided only by comment headers.
- **Merge conflicts:** Any backend change touches the same file, increasing conflict risk on branches.
- **Testability:** Hard to unit-test individual concerns when everything is coupled in one file.
- **Onboarding:** New contributors (human or AI) must parse the entire file to understand any single feature.

## Proposed Module Structure

```
src-tauri/src/
├── main.rs              # Unchanged (7 lines, calls run())
├── lib.rs               # Slim orchestrator: mod declarations, re-exports, run()
├── models.rs            # All data structs + serde derives
├── settings.rs          # Settings path, load, save
├── keychain.rs          # macOS Keychain read/write/validate + constants
├── oauth.rs             # PKCE flow, token refresh, credential loading
├── usage.rs             # Usage API fetch + all parsing helpers
├── commands.rs          # All #[tauri::command] functions
├── updater.rs           # Update check/install flow + update window management
└── tray.rs              # Tray icon builder, menu, click/menu event handlers
```

### Module Breakdown

| Module | Lines (approx) | Responsibility | Key items moved |
|--------|----------------|----------------|-----------------|
| `models.rs` | ~130 | Data types shared across modules | `ClaudeUsageData`, `Settings`, `ClaudeCredentials`, `ClaudeOAuthBlob`, `ClaudeUsageWindow`, `ClaudeExtraUsage`, `ClaudeOAuthUsageResponse`, `ClaudeProfileResponse`, `OAuthTokenResponse`, `SaveTokenInput` |
| `settings.rs` | ~30 | Settings persistence | `settings_path()`, `load_settings()`, `save_settings()`, default fns |
| `keychain.rs` | ~50 | Keychain CRUD + token validation | `KEYCHAIN_SERVICE`, `KEYCHAIN_ACCOUNT`, `read_keychain_oauth_blob()`, `write_keychain_oauth_blob()`, `validate_claude_oauth_access_token()` |
| `oauth.rs` | ~360 | OAuth PKCE login + token refresh | `OAUTH_CLIENT_ID`, `generate_code_verifier()`, `generate_code_challenge()`, `login_oauth()`, `refresh_claude_token()`, `load_claude_credentials()` |
| `usage.rs` | ~200 | Fetch + parse usage API response | `fetch_claude_usage_impl()`, `plan_display_from_profile()`, `clamp_percent()`, all `usage_window_*` and `extract_*` helpers |
| `commands.rs` | ~110 | Thin Tauri command wrappers | All `#[tauri::command]` fns: `fetch_claude_usage`, `save_token`, `has_token`, `clear_token`, `get_settings`, `save_poll_interval`, `update_tray_title`, `login_oauth` (command), `debug_token_info`, `force_refresh_token` |
| `updater.rs` | ~150 | App self-update system | `UPDATE_CHECK_IN_PROGRESS`, `check_for_update()`, `check_for_update_inner()` |
| `tray.rs` | ~140 | Tray icon + menu setup | Extract tray builder + menu event handler from `run()` into `pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>>` |
| `lib.rs` | ~40 | Module declarations + `run()` | `mod` statements, `pub use` re-exports, slim `run()` that calls `tray::setup_tray()` |

### Shared Utilities

The `now_ms()` helper and `TrayState` wrapper are used across modules. Keep `now_ms()` in `lib.rs` (or a small `util.rs` if preferred) and `TrayState` + `PANEL_HIDDEN_AT_MS` in `tray.rs`.

## Technical Considerations

- **Visibility:** Items currently private in `lib.rs` will need `pub(crate)` when moved to separate modules so sibling modules can access them.
- **Circular dependencies:** Avoid by ensuring the dependency graph flows downward: `commands` → `oauth`/`usage`/`settings`/`keychain` → `models`. No module should import from `commands`.
- **Tauri command registration:** The `invoke_handler!` macro in `run()` references command functions. These must be accessible from `lib.rs` via `pub(crate)` or re-exported.
- **Tests:** The existing `#[cfg(test)] mod tests` block uses `super::*`. Move tests alongside their module (e.g., keychain tests in `keychain.rs`, usage parsing tests in `usage.rs`, integration tests stay in `lib.rs` or a dedicated `tests/` module).
- **No behavioral changes:** This is a pure restructuring. No logic changes, no API changes, no dependency changes.

## Module Dependency Graph

```
lib.rs (run)
├── tray.rs ← models, settings
├── commands.rs ← models, settings, keychain, oauth, usage, updater
├── updater.rs ← (standalone, uses tauri APIs)
├── oauth.rs ← models, keychain
├── usage.rs ← models, keychain, oauth
├── keychain.rs ← models
├── settings.rs ← models
└── models.rs ← (leaf, no internal deps)
```

## Implementation Steps

1. **Create `models.rs`** — Move all struct/enum definitions. Add `pub(crate)` visibility. Update `lib.rs` with `mod models;` and verify compilation.

2. **Create `settings.rs`** — Move `settings_path()`, `load_settings()`, `save_settings()`, default fns. Import from `models`. Verify compilation.

3. **Create `keychain.rs`** — Move constants, keychain functions, token validation. Import from `models`. Verify compilation.

4. **Create `usage.rs`** — Move `fetch_claude_usage_impl()` and all parsing helpers. Import from `models`, `keychain`, `oauth`. Verify compilation.

5. **Create `oauth.rs`** — Move PKCE functions, `refresh_claude_token()`, `load_claude_credentials()`, `login_oauth()` impl. Import from `models`, `keychain`. Verify compilation.

6. **Create `updater.rs`** — Move update check logic. Standalone module using Tauri APIs. Verify compilation.

7. **Create `tray.rs`** — Extract tray setup from `run()` into a function. Move `TrayState`, `PANEL_HIDDEN_AT_MS`. Import from `settings`, `models`. Verify compilation.

8. **Create `commands.rs`** — Move all `#[tauri::command]` functions. Import from all other modules as needed. Verify compilation.

9. **Slim down `lib.rs`** — Should contain only: `mod` declarations, `pub use` for `run`, the `run()` function, and `now_ms()`.

10. **Redistribute tests** — Move test functions to their relevant modules. Keep integration tests that exercise multiple modules in `lib.rs` or `tests/integration.rs`.

11. **Final verification** — `cargo build`, `cargo test`, `npm run tauri dev` — confirm no regressions.

## Acceptance Criteria

- [x] `lib.rs` is under 60 lines (mod declarations + `run()` + `now_ms()`) — 49 lines of code (100 with integration test)
- [x] Each new module file has a single clear responsibility
- [x] `cargo build` succeeds with no warnings
- [x] `cargo test` passes (all existing tests still compile and run)
- [ ] `npm run tauri dev` launches the app with identical behavior
- [x] No logic changes — pure file reorganization
- [x] No new dependencies added

## Success Metrics

- Each module file is under 400 lines
- Any developer can locate code by filename alone (e.g., "OAuth logic? → `oauth.rs`")
- Future PRs touch fewer files for single-concern changes

## Dependencies & Risks

- **Risk: Merge conflict with current branch** — `fix/oauth-token-refresh` has uncommitted changes to `lib.rs`. Commit or stash current work before starting the refactor.
- **Risk: Tauri macro compatibility** — `#[tauri::command]` functions must be importable in `invoke_handler!`. Verify `pub(crate)` works with the macro.
- **Mitigation:** Do one module at a time, compiling after each extraction to catch issues immediately.
