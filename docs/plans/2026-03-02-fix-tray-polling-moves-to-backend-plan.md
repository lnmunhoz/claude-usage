---
title: "fix: Move tray polling from frontend to Rust backend"
type: fix
status: completed
date: 2026-03-02
---

# fix: Move tray polling from frontend to Rust backend

## Overview

The tray icon percentage only updates when the user clicks the tray icon to open the panel. After a few hours of inactivity, the displayed percentage becomes stale. It should continuously refresh in the background regardless of whether the panel is open.

## Problem Statement / Motivation

All usage polling currently happens in the **React frontend** via `setInterval` in `src/App.tsx:546`:

```tsx
const interval = setInterval(fetchUsage, pollIntervalSeconds * 1000);
```

The tray title is updated from within `fetchUsage` (App.tsx:532-533):

```tsx
const pctLeft = (100 - data.sessionPercentUsed).toFixed(0);
await invoke("update_tray_title", { title: `${pctLeft}%` });
```

When the user clicks away from the panel, the webview is **hidden** on focus loss (`src-tauri/src/tray.rs:133-136`):

```rust
if let tauri::WindowEvent::Focused(false) = event {
    PANEL_HIDDEN_AT_MS.store(now_ms(), Ordering::SeqCst);
    let _ = win.hide();
}
```

**macOS/WebKit throttles or suspends JavaScript timers in hidden windows.** Once the webview is hidden, `setInterval` stops firing reliably, so `fetchUsage` never runs, and the tray title never updates. The percentage only refreshes when the user clicks the tray icon again, which shows the window and unfreezes the JS event loop.

## Proposed Solution

Move the background polling loop from the React frontend to the **Rust backend** using `tokio::spawn` with `tokio::time::interval`. The Rust task runs independently of webview visibility and directly updates the tray title. The frontend receives usage data via Tauri events instead of polling itself.

### Architecture change

```
BEFORE:
  React setInterval → invoke("fetch_claude_usage") → Rust fetches → React updates state → invoke("update_tray_title")
  (breaks when webview is hidden)

AFTER:
  Rust tokio::spawn loop → fetch_claude_usage_impl() → update tray title directly → emit "usage-updated" event
  Frontend listens for "usage-updated" event → updates UI state
  (works regardless of webview visibility)
```

## Technical Considerations

### Concurrency: Token Refresh Races

With single-use refresh tokens, concurrent callers can race on token refresh. The backend loop should be the **sole caller** of `fetch_claude_usage_impl()`. The frontend should consume events rather than invoking `fetch_claude_usage` directly. This eliminates the entire class of refresh-token race conditions.

As defense-in-depth, wrap the token refresh flow in a `tokio::sync::Mutex` to protect the read-check-refresh-write keychain sequence. This also protects against the existing `force_refresh_token` command.

### Polling Loop Lifecycle

Use `tokio_util::sync::CancellationToken` to manage the loop lifecycle:

- **Start:** On app launch if a token exists, or after successful OAuth login
- **Stop:** On disconnect (user clears token)
- **Restart:** On re-login after disconnect

Store the `CancellationToken` + `JoinHandle` in Tauri managed state so commands can cancel/restart the loop.

### Poll Interval Hot-Reload

Use a `tokio::sync::watch<u64>` channel to notify the loop of interval changes. The loop does `tokio::select!` between the interval tick, the watch channel, and the cancellation token. On receiving a new interval, it recreates the `tokio::time::Interval` with the updated duration.

### macOS Sleep/Wake

Set `MissedTickBehavior::Delay` on the tokio interval. After a 30-minute sleep, the loop fires **once immediately** then resumes normal cadence, rather than bursting 30 requests.

### Network Errors

On failure, retain the last known tray title value (stale but not alarming). Implement simple backoff: double the interval on consecutive failures (up to 15-minute cap), reset to normal on success. On auth failure (refresh token invalid), stop the loop and clear the tray title.

### Display Mode

The backend must read `Settings.display_mode` from managed state when formatting the tray title, so the tray respects the "Show Remaining" vs "Show Used" toggle.

## Acceptance Criteria

- [x] Tray percentage updates continuously in the background without clicking the tray icon
- [x] Tray updates work after macOS sleep/wake cycles
- [x] Token refresh works transparently during background polling
- [x] Poll interval changes take effect immediately (no app restart)
- [x] Disconnecting stops the background loop; reconnecting restarts it
- [x] Frontend panel still shows correct usage data when opened (via events)
- [x] No duplicate API calls (frontend no longer polls independently)
- [x] Network errors don't crash the polling loop
- [x] Display mode setting (remaining vs used) is respected in tray title

## Success Metrics

- Tray percentage stays up-to-date for 8+ hours without user interaction
- No regression in panel UI behavior when clicking the tray icon
- No increase in API call volume (same poll frequency, single caller)

## Dependencies & Risks

- **Risk:** `tokio_util` crate may need to be added as a dependency for `CancellationToken`
- **Risk:** Existing `fetch_claude_usage` command is used by the frontend; removing it is a breaking change to the frontend code
- **Risk:** The `force_refresh_token` debug command could race with the polling loop if both attempt refresh simultaneously

## Implementation Suggestions

### New/Modified Files

1. **`src-tauri/src/poller.rs`** (new) — Background polling loop module
   - `PollerState` struct: holds `CancellationToken`, `JoinHandle`, `watch::Sender<u64>` for interval
   - `start_polling()`: spawns the tokio task, returns `PollerState`
   - `stop_polling()`: cancels the token, awaits the handle
   - The loop body: fetch usage → update tray title → emit `usage-updated` event

2. **`src-tauri/src/lib.rs`** — Register `poller` module, start polling on setup if token exists

3. **`src-tauri/src/commands.rs`** — Modify:
   - `save_poll_interval`: also send new interval via watch channel
   - `clear_token` (disconnect): stop the polling loop
   - `login_oauth` (or `handleTokenSaved` path): start/restart the polling loop
   - Keep `fetch_claude_usage` as a fallback or remove it (frontend will use events)

4. **`src-tauri/src/usage.rs`** — Add `tokio::sync::Mutex` around token refresh in `fetch_claude_usage_impl`

5. **`src/App.tsx`** — Replace `setInterval` polling with a `listen("usage-updated")` event handler. Remove the direct `invoke("fetch_claude_usage")` calls. Add `listen("usage-error")` for error states.

### Event Contract

```
Event: "usage-updated"
Payload: ClaudeUsageData { sessionPercentUsed, weeklyPercentUsed, sessionReset, weeklyReset, planType, extraUsageSpend, extraUsageLimit }

Event: "usage-error"
Payload: { message: string }
```

## Sources & References

- Root cause identified in: `src/App.tsx:542-548` (setInterval polling), `src-tauri/src/tray.rs:130-138` (webview hide on focus loss)
- Token refresh logic: `src-tauri/src/oauth.rs:30-122`
- Usage fetching: `src-tauri/src/usage.rs:139-320`
- Tray title update command: `src-tauri/src/commands.rs:73-81`
- Related plan: `docs/plans/2026-03-01-fix-oauth-token-refresh-plan.md` (OAuth refresh fix, already implemented)
