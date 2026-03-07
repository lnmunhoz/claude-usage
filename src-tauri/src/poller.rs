use std::sync::Mutex;
use std::time::Duration;

use rand::Rng;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use tokio::time::{self, MissedTickBehavior};

use tauri::async_runtime;

use crate::models::{ClaudeUsageData, Settings};
use crate::tray::TrayState;
use crate::usage::{fetch_claude_usage_impl, FetchError};

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
const RATE_LIMIT_INITIAL_BACKOFF_SECS: u64 = 30;
const MAX_RATE_LIMIT_BACKOFF_SECS: u64 = 600; // 10 minutes
const MIN_RETRY_AFTER_SECS: u64 = 5; // clamp Retry-After: 0 to avoid zero-period panic

enum PollOutcome {
    Success,
    RateLimit { retry_after_secs: Option<u64> },
    Error,
}

/// Spawn the background polling task. Returns a handle to control it.
pub(crate) fn start_poller(app_handle: AppHandle, initial_interval_secs: u64) -> PollerHandle {
    let (tx, mut rx) = watch::channel(Some(initial_interval_secs));

    async_runtime::spawn(async move {
        println!(
            "[claude-usage] Poller started with interval {}s",
            initial_interval_secs
        );

        let mut base_interval_secs = initial_interval_secs;
        let mut consecutive_failures: u32 = 0;
        let mut rate_limit_failures: u32 = 0;
        let mut is_rate_limited = false;

        // Initial fetch right away
        let initial_outcome = do_poll(&app_handle).await;
        let mut interval = match initial_outcome {
            PollOutcome::RateLimit { retry_after_secs } => {
                is_rate_limited = true;
                rate_limit_failures = 1;
                let secs = retry_after_secs.unwrap_or(RATE_LIMIT_INITIAL_BACKOFF_SECS).max(MIN_RETRY_AFTER_SECS);
                let _ = app_handle.emit(
                    "rate-limited",
                    serde_json::json!({ "retryAfterSecs": secs }),
                );
                time::interval(Duration::from_secs(secs))
            }
            _ => time::interval(Duration::from_secs(base_interval_secs)),
        };
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await; // consume immediate first tick

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match do_poll(&app_handle).await {
                        PollOutcome::Success => {
                            consecutive_failures = 0;
                            if is_rate_limited {
                                is_rate_limited = false;
                                rate_limit_failures = 0;
                                let _ = app_handle.emit("rate-limit-cleared", ());
                                println!("[claude-usage] Rate limit cleared, resuming normal polling");
                            }
                            // Restore normal interval if we were in backoff
                            if interval.period() != Duration::from_secs(base_interval_secs) {
                                interval = time::interval(Duration::from_secs(base_interval_secs));
                                interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                                interval.tick().await;
                            }
                        }
                        PollOutcome::RateLimit { retry_after_secs } => {
                            is_rate_limited = true;
                            rate_limit_failures += 1;
                            let backoff_secs = if let Some(secs) = retry_after_secs {
                                let secs = secs.max(MIN_RETRY_AFTER_SECS);
                                println!("[claude-usage] Rate limited. Respecting Retry-After: {}s", secs);
                                secs
                            } else {
                                // Exponential backoff with jitter starting at RATE_LIMIT_INITIAL_BACKOFF_SECS
                                let exp_backoff = (RATE_LIMIT_INITIAL_BACKOFF_SECS
                                    * 2u64.pow(rate_limit_failures.saturating_sub(1).min(5)))
                                .min(MAX_RATE_LIMIT_BACKOFF_SECS);
                                let max_jitter = (exp_backoff / 4).max(1);
                                let jitter: u64 = rand::rng().random_range(0..=max_jitter);
                                let total = exp_backoff + jitter;
                                println!(
                                    "[claude-usage] Rate limited (failure #{}). Backing off {}s (base={}s, jitter={}s)",
                                    rate_limit_failures, total, exp_backoff, jitter
                                );
                                total
                            };
                            let _ = app_handle.emit(
                                "rate-limited",
                                serde_json::json!({ "retryAfterSecs": backoff_secs }),
                            );
                            interval = time::interval(Duration::from_secs(backoff_secs));
                            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                            interval.tick().await;
                        }
                        PollOutcome::Error => {
                            is_rate_limited = false;
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
                            rate_limit_failures = 0;
                            is_rate_limited = false;
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

/// Perform a single poll: fetch usage, update tray, emit event. Returns the outcome.
async fn do_poll(app_handle: &AppHandle) -> PollOutcome {
    match fetch_claude_usage_impl().await {
        Ok(data) => {
            update_tray_title(app_handle, &data);
            let _ = app_handle.emit("usage-updated", &data);
            PollOutcome::Success
        }
        Err(FetchError::RateLimit { retry_after_secs }) => {
            println!(
                "[claude-usage] Poll rate limited: retry_after={:?}",
                retry_after_secs
            );
            PollOutcome::RateLimit { retry_after_secs }
        }
        Err(FetchError::Other(e)) => {
            println!("[claude-usage] Poll failed: {}", e);
            let _ = app_handle.emit("usage-error", serde_json::json!({ "message": e }));
            PollOutcome::Error
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
