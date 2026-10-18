//! Slint UI integration.

slint::include_modules!();

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::Image;

use crate::models::AppEntry;

/// Row for a search result. Shows the app icon when one can be loaded.
pub fn app_row(app: &AppEntry) -> ResultRow {
    let icon = app
        .icon_path
        .as_deref()
        .and_then(|path| Image::load_from_path(path).ok());
    ResultRow {
        name: app.name.clone().into(),
        has_icon: icon.is_some(),
        icon: icon.unwrap_or_default(),
    }
}

/// Row for an executable in the picker. Shows the file name only.
pub fn exe_row(exe: &Path) -> ResultRow {
    let name = exe
        .file_name()
        .unwrap_or(exe.as_os_str())
        .to_string_lossy()
        .into_owned();
    ResultRow {
        name: name.into(),
        has_icon: false,
        icon: Image::default(),
    }
}

/// Selects the winit backend, which is needed for window positioning.
pub fn create_window() -> Result<AppWindow, Box<dyn Error>> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;
    Ok(AppWindow::new()?)
}

/// Centers the window on the first native event, when the winit window exists.
pub fn center_on_startup(window: &AppWindow) {
    let weak = window.as_weak();
    let mut centered = false;
    window.window().on_winit_window_event(move |_, event| {
        if !centered && matches!(event, WindowEvent::Resized(_) | WindowEvent::RedrawRequested) {
            if let Some(window) = weak.upgrade() {
                centered = center(&window);
            }
        }
        EventResult::Propagate
    });
}

/// Gives the native window keyboard focus once the event loop has created it.
pub fn focus_on_startup(window: &AppWindow) {
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(window) = weak.upgrade() {
            window.window().with_winit_window(|native| native.focus_window());
        }
    });
}

/// Returns `true` when the window was positioned. Wayland ignores client positions.
fn center(window: &AppWindow) -> bool {
    window
        .window()
        .with_winit_window(|native| {
            let Some(monitor) = native.current_monitor().or_else(|| native.primary_monitor()) else {
                return false;
            };
            // Slint resizes the window after creation, so use its final size, not the current one.
            let scale = native.scale_factor();
            let window_width = (window.get_panel_width() as f64 * scale).round() as i32;
            let window_height = (window.get_panel_height() as f64 * scale).round() as i32;
            let monitor_pos = monitor.position();
            let monitor_size = monitor.size();
            let x = monitor_pos.x + (monitor_size.width as i32 - window_width) / 2;
            let y = monitor_pos.y + (monitor_size.height as i32 - window_height) / 2;
            native.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            true
        })
        .unwrap_or(false)
}
