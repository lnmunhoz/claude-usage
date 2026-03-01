use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;

use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_positioner::{Position, WindowExt as PositionerExt};

use crate::models::Settings;
use crate::now_ms;
use crate::settings::save_settings;
use crate::updater::check_for_update;

// Wrapper so we can manage TrayIcon separately from Settings
pub(crate) struct TrayState(pub TrayIcon);

// Debounce: prevent re-showing panel right after focus-loss hide
pub(crate) static PANEL_HIDDEN_AT_MS: AtomicI64 = AtomicI64::new(0);

pub(crate) fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let settings = crate::settings::load_settings();

    // Build tray context menu
    let remaining_mode_item =
        CheckMenuItemBuilder::with_id("remaining_mode", "Show Remaining (Juice Mode)")
            .checked(settings.display_mode == "remaining")
            .build(app)?;

    let autostart_manager = app.autolaunch();
    let is_autostart = autostart_manager.is_enabled().unwrap_or(false);
    let launch_at_login_item =
        CheckMenuItemBuilder::with_id("launch_at_login", "Launch at Login")
            .checked(is_autostart)
            .build(app)?;

    let quit_item = PredefinedMenuItem::quit(app, Some("Quit Claude Usage"))?;

    let tray_menu = MenuBuilder::new(app)
        .item(&remaining_mode_item)
        .separator()
        .text("configure_interval", "Configure Refresh Interval...")
        .separator()
        .text("check_for_updates", "Check for Updates...")
        .separator()
        .item(&launch_at_login_item)
        .separator()
        .item(&quit_item)
        .build()?;

    // Build tray icon (use dedicated tray icon – black silhouette on transparent)
    let tray_icon_bytes = include_bytes!("../icons/tray-icon@2x.png");
    let tray_icon_image = tauri::image::Image::from_bytes(tray_icon_bytes)
        .expect("Failed to load tray icon");
    let tray = TrayIconBuilder::new()
        .icon(tray_icon_image)
        .icon_as_template(true)
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        // Debounce: don't re-show if just hidden by focus loss
                        let last_hide = PANEL_HIDDEN_AT_MS.load(Ordering::SeqCst);
                        if now_ms() - last_hide < 300 {
                            return;
                        }
                        let _ = window.move_window(Position::TrayCenter);
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .on_menu_event(move |app_handle, event| {
            let id = event.id().0.as_str();
            match id {
                "remaining_mode" => {
                    let state = app_handle.state::<Mutex<Settings>>();
                    let mut settings = state.lock().unwrap();
                    let checked = remaining_mode_item.is_checked().unwrap_or(true);
                    settings.display_mode = if checked {
                        "remaining".to_string()
                    } else {
                        "usage".to_string()
                    };
                    save_settings(&settings);
                    let _ = app_handle.emit("settings-changed", settings.clone());
                }
                "configure_interval" => {
                    let _ = app_handle.emit("show-settings", ());
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "check_for_updates" => {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        check_for_update(handle, true).await;
                    });
                }
                "launch_at_login" => {
                    let manager = app_handle.autolaunch();
                    let checked = launch_at_login_item.is_checked().unwrap_or(false);
                    if checked {
                        let _ = manager.enable();
                    } else {
                        let _ = manager.disable();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    app.manage(Mutex::new(TrayState(tray)));

    // Focus-loss handler: hide panel when it loses focus
    if let Some(window) = app.get_webview_window("main") {
        let win = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                PANEL_HIDDEN_AT_MS.store(now_ms(), Ordering::SeqCst);
                let _ = win.hide();
            }
        });
    }

    // Auto-check for updates on startup (silent)
    let startup_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        check_for_update(startup_handle, false).await;
    });

    Ok(())
}
