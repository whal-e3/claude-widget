use crate::models::CornerPosition;
use log::info;
use tauri::{AppHandle, Manager, PhysicalPosition};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect,
};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};

const PADDING: i32 = 16;

#[tauri::command(async)]
pub async fn set_widget_position(app: AppHandle, corner: String) -> Result<(), String> {
    let position = match corner.as_str() {
        "top-right" => CornerPosition::TopRight,
        "top-left" => CornerPosition::TopLeft,
        "bottom-left" => CornerPosition::BottomLeft,
        _ => CornerPosition::BottomRight,
    };

    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No monitor found")?;

    let monitor_size = monitor.size();
    let monitor_pos = monitor.position();
    let window_size = window.outer_size().map_err(|e| e.to_string())?;

    let (x, y) = match position {
        CornerPosition::TopRight => (
            monitor_pos.x + monitor_size.width as i32 - window_size.width as i32 - PADDING,
            monitor_pos.y + PADDING,
        ),
        CornerPosition::TopLeft => (monitor_pos.x + PADDING, monitor_pos.y + PADDING),
        CornerPosition::BottomRight => (
            monitor_pos.x + monitor_size.width as i32 - window_size.width as i32 - PADDING,
            monitor_pos.y + monitor_size.height as i32 - window_size.height as i32 - PADDING,
        ),
        CornerPosition::BottomLeft => (
            monitor_pos.x + PADDING,
            monitor_pos.y + monitor_size.height as i32 - window_size.height as i32 - PADDING,
        ),
    };

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;

    info!("Widget positioned at {:?}", position);
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_fullscreen_app_active() -> bool {
    unsafe {
        // Method 1: SHQueryUserNotificationState
        if let Ok(state) = SHQueryUserNotificationState() {
            if state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN {
                return true;
            }
        }

        // Method 2: Check if foreground window covers entire monitor
        let foreground = GetForegroundWindow();
        if foreground == HWND::default() {
            return false;
        }

        let mut window_rect = RECT::default();
        if GetWindowRect(foreground, &mut window_rect).is_err() {
            return false;
        }

        let monitor = MonitorFromWindow(foreground, MONITOR_DEFAULTTOPRIMARY);
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
            return false;
        }

        let mr = monitor_info.rcMonitor;
        window_rect.left <= mr.left
            && window_rect.top <= mr.top
            && window_rect.right >= mr.right
            && window_rect.bottom >= mr.bottom
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_fullscreen_app_active() -> bool {
    false
}

pub fn start_fullscreen_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut was_hidden = false;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));

            let is_fullscreen = is_fullscreen_app_active();

            if let Some(window) = app.get_webview_window("main") {
                if is_fullscreen && !was_hidden {
                    let _ = window.hide();
                    was_hidden = true;
                } else if !is_fullscreen && was_hidden {
                    let _ = window.show();
                    was_hidden = false;
                }
            }
        }
    });
}
