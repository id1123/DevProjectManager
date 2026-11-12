use tauri::{Monitor, PhysicalPosition, WebviewWindow};

/// Center in the active monitor work area using its DPI and desktop coordinates.
pub fn position_on_active_monitor(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = active_monitor(window)
        .or_else(|| {
            let cursor = window.cursor_position().ok()?;
            window.monitor_from_point(cursor.x, cursor.y).ok().flatten()
        })
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let logical_size = window
        .outer_size()?
        .to_logical::<f64>(window.scale_factor()?);
    let size = logical_size.to_physical::<u32>(monitor.scale_factor());
    let area = monitor.work_area();
    window.set_position(PhysicalPosition::new(
        area.position.x + (area.size.width.saturating_sub(size.width) / 2) as i32,
        area.position.y + (area.size.height.saturating_sub(size.height) / 2) as i32,
    ))
}

/// Resolve the foreground window monitor before the launcher takes focus.
#[cfg(target_os = "windows")]
fn active_monitor(window: &WebviewWindow) -> Option<Monitor> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }
    // All handles are borrowed; the structure matches Win32 MONITORINFO.
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return None;
        }
        let monitor = MonitorFromWindow(foreground, 2); // MONITOR_DEFAULTTONEAREST
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let x = (f64::from(info.monitor.left) + f64::from(info.monitor.right)) / 2.0;
        let y = (f64::from(info.monitor.top) + f64::from(info.monitor.bottom)) / 2.0;
        window.monitor_from_point(x, y).ok().flatten()
    }
}

/// Other platforms use the cursor monitor as the active screen.
#[cfg(not(target_os = "windows"))]
fn active_monitor(_window: &WebviewWindow) -> Option<Monitor> {
    None
}
