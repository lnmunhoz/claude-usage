use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use tokio::time::{self, MissedTickBehavior};

use tauri::async_runtime;

use crate::models::{ClaudeUsageData, Settings};
use crate::tray::TrayState;
use crate::usage::fetch_claude_usage_impl;

/// Handle to control the background polling loop.
/// Stored as `Mutex<Option<PollerHandle>>` in Tauri managed state.
pub(crate) struct PollerHandle {
    /// Send `Some(seconds)` to change the interval, or `None` to stop the loop.
    interval_tx: watch::Sender<Option<u64>>,
}

impl PollerHandle {
    pub fn update_interval(&self, seconds: u64) {
        let _ = self.interval_tx.send(Some(seconds));
    }

    pub fn stop(&self) {
        let _ = self.interval_tx.send(None);
    }
}

const MAX_BACKOFF_SECS: u64 = 900; // 15 minutes

/// Spawn the background polling task. Returns a handle to control it.
pub(crate) fn start_poller(app_handle: AppHandle, initial_interval_secs: u64) -> PollerHandle {
    let (tx, mut rx) = watch::channel(Some(initial_interval_secs));

    async_runtime::spawn(async move {
        println!(
            "[claude-usage] Poller started with interval {}s",
            initial_interval_secs
        );

        // Initial fetch right away
        do_poll(&app_handle).await;

        let mut base_interval_secs = initial_interval_secs;
        let mut interval = time::interval(Duration::from_secs(base_interval_secs));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await; // consume immediate first tick

        let mut consecutive_failures: u32 = 0;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let ok = do_poll(&app_handle).await;
                    if ok {
                        consecutive_failures = 0;
                        // Restore normal interval if we were in backoff
                        if interval.period() != Duration::from_secs(base_interval_secs) {
                            interval = time::interval(Duration::from_secs(base_interval_secs));
                            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                            interval.tick().await;
                        }
                    } else {
                        consecutive_failures += 1;
                        if consecutive_failures > 1 {
                            let backoff_secs = (base_interval_secs
                                * 2u64.pow(consecutive_failures.min(6) - 1))
                            .min(MAX_BACKOFF_SECS);
                            println!("[claude-usage] Backing off to {}s", backoff_secs);
                            interval = time::interval(Duration::from_secs(backoff_secs));
                            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                            interval.tick().await; // consume immediate tick
                        }
                    }
                }

                result = rx.changed() => {
                    if result.is_err() {
                        // Sender dropped, exit
                        println!("[claude-usage] Poller channel closed, exiting");
                        break;
                    }

                    // Copy the value out of the borrow before any .await
                    let new_value = *rx.borrow_and_update();

                    match new_value {
                        Some(new_secs) => {
                            base_interval_secs = new_secs;
                            consecutive_failures = 0;
                            interval = time::interval(Duration::from_secs(base_interval_secs));
                            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                            interval.tick().await; // consume immediate tick
                            println!("[claude-usage] Poll interval updated to {}s", base_interval_secs);
                        }
                        None => {
                            println!("[claude-usage] Poller stopped by signal");
                            break;
                        }
                    }
                }
            }
        }
    });

    PollerHandle { interval_tx: tx }
}

/// Perform a single poll: fetch usage, update tray, emit event. Returns true on success.
async fn do_poll(app_handle: &AppHandle) -> bool {
    match fetch_claude_usage_impl().await {
        Ok(data) => {
            update_tray_title(app_handle, &data);
            let _ = app_handle.emit("usage-updated", &data);
            true
        }
        Err(e) => {
            println!("[claude-usage] Poll failed: {}", e);
            let _ = app_handle.emit(
                "usage-error",
                serde_json::json!({ "message": e.to_string() }),
            );
            false
        }
    }
}

fn update_tray_title(app_handle: &AppHandle, data: &ClaudeUsageData) {
    let display_mode = app_handle
        .try_state::<Mutex<Settings>>()
        .map(|s| s.lock().unwrap().display_mode.clone())
        .unwrap_or_else(|| "remaining".to_string());

    let pct = if display_mode == "remaining" {
        100.0 - data.session_percent_used
    } else {
        data.session_percent_used
    };

    let title = format!("{:.0}%", pct);

    if let Some(tray_state) = app_handle.try_state::<Mutex<TrayState>>() {
        let tray = tray_state.lock().unwrap();
        let _ = tray.0.set_title(Some(&title));
    }
}
