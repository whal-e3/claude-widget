mod api;
mod auth;
mod models;
mod poller;
mod scraper;
mod session;
mod window;

use poller::{Poller, PollerState};
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Position widget at bottom-right corner on startup
            let pos_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Small delay to let window initialize
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let _ = window::set_widget_position(pos_handle, "bottom-right".to_string()).await;
            });

            // Start fullscreen watcher
            window::start_fullscreen_watcher(app_handle.clone());

            // Start usage poller and store in state
            let poller = Arc::new(Poller::new(app_handle.clone()));
            app.manage(PollerState(poller.clone()));
            tauri::async_runtime::spawn(async move {
                poller.start_polling().await;
            });

            // Setup system tray menu
            setup_tray(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::open_login_browser,
            auth::capture_browser_cookies,
            auth::save_session_cookie,
            auth::check_session,
            auth::clear_session,
            window::set_widget_position,
            poller::get_usage_history,
            poller::force_refresh,
            poller::get_current_usage,
            poller::get_cached_usage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let show_hide = MenuItemBuilder::with_id("show_hide", "Show/Hide").build(app)?;
    let login = MenuItemBuilder::with_id("login", "Login").build(app)?;
    let refresh = MenuItemBuilder::with_id("refresh", "Refresh Now").build(app)?;
    let pos_tr = MenuItemBuilder::with_id("pos_tr", "Position: Top Right").build(app)?;
    let pos_tl = MenuItemBuilder::with_id("pos_tl", "Position: Top Left").build(app)?;
    let pos_br = MenuItemBuilder::with_id("pos_br", "Position: Bottom Right").build(app)?;
    let pos_bl = MenuItemBuilder::with_id("pos_bl", "Position: Bottom Left").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &show_hide,
            &login,
            &refresh,
            &pos_tr,
            &pos_tl,
            &pos_br,
            &pos_bl,
            &quit,
        ])
        .build()?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let app_handle = app.clone();
            match event.id().as_ref() {
                "show_hide" => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "login" => {
                    tauri::async_runtime::spawn(async move {
                        let _ = auth::open_login_browser().await;
                    });
                }
                "refresh" => {
                    let _ = app_handle.emit("force-refresh", ());
                }
                "pos_tr" => {
                    tauri::async_runtime::spawn(async move {
                        let _ = window::set_widget_position(app_handle, "top-right".to_string())
                            .await;
                    });
                }
                "pos_tl" => {
                    tauri::async_runtime::spawn(async move {
                        let _ = window::set_widget_position(app_handle, "top-left".to_string())
                            .await;
                    });
                }
                "pos_br" => {
                    tauri::async_runtime::spawn(async move {
                        let _ =
                            window::set_widget_position(app_handle, "bottom-right".to_string())
                                .await;
                    });
                }
                "pos_bl" => {
                    tauri::async_runtime::spawn(async move {
                        let _ =
                            window::set_widget_position(app_handle, "bottom-left".to_string())
                                .await;
                    });
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
