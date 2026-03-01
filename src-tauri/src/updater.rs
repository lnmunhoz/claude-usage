use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

static UPDATE_CHECK_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub(crate) async fn check_for_update(app: AppHandle, manual: bool) {
    if UPDATE_CHECK_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }
    check_for_update_inner(&app, manual).await;
    UPDATE_CHECK_IN_PROGRESS.store(false, Ordering::SeqCst);
}

async fn check_for_update_inner(app: &AppHandle, manual: bool) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            println!("[claude-usage] Updater unavailable: {}", e);
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            println!("[claude-usage] No update available.");
            if manual {
                app.dialog()
                    .message("You're on the latest version.")
                    .title("No Updates")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
            return;
        }
        Err(e) => {
            println!("[claude-usage] Update check failed: {}", e);
            if manual {
                app.dialog()
                    .message("Failed to check for updates. Please try again later.")
                    .title("Update Error")
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
            return;
        }
    };

    println!(
        "[claude-usage] Update available: v{} (notes: {:?})",
        update.version,
        update.body
    );

    let version = update.version.clone();
    let body = update.body.clone().unwrap_or_default();

    if let Some(win) = app.get_webview_window("update") {
        let _ = win.close();
    }

    let update_window = match WebviewWindowBuilder::new(
        app,
        "update",
        WebviewUrl::App("index.html".into()),
    )
    .title("Claude Usage Update")
    .inner_size(340.0, 400.0)
    .resizable(false)
    .center()
    .background_color(tauri::window::Color(28, 28, 30, 255))
    .build()
    {
        Ok(w) => w,
        Err(e) => {
            println!("[claude-usage] Failed to create update window: {}", e);
            return;
        }
    };

    let ready_handle = app.clone();
    let ready_id = update_window.listen("update-ready", move |_: tauri::Event| {
        if let Some(win) = ready_handle.get_webview_window("update") {
            let payload = serde_json::json!({
                "version": version,
                "body": body,
            });
            let _ = win.emit("update-info", payload);
        }
    });

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    let tx = std::sync::Mutex::new(Some(tx));
    let response_id = update_window.listen("update-response", move |event: tauri::Event| {
        if let Some(tx) = tx.lock().unwrap().take() {
            let accepted = serde_json::from_str::<serde_json::Value>(event.payload())
                .ok()
                .and_then(|v| v.get("accepted").and_then(|a| a.as_bool()))
                .unwrap_or(false);
            let _ = tx.send(accepted);
        }
    });

    let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();
    let close_tx = std::sync::Mutex::new(Some(close_tx));
    let close_id = update_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(tx) = close_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    });

    let accepted = tokio::select! {
        result = rx => result.unwrap_or(false),
        _ = close_rx => false,
    };

    update_window.unlisten(response_id);
    update_window.unlisten(ready_id);
    let _ = close_id;

    if let Some(win) = app.get_webview_window("update") {
        let _ = win.close();
    }

    if accepted {
        println!("[claude-usage] User accepted update, downloading...");
        if let Err(e) = update.download_and_install(|_, _| {}, || {}).await {
            println!("[claude-usage] Update install failed: {}", e);
            app.dialog()
                .message(format!("Failed to install update: {}", e))
                .title("Update Error")
                .kind(MessageDialogKind::Error)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
            return;
        }
        app.restart();
    } else {
        println!("[claude-usage] User declined update.");
    }
}
