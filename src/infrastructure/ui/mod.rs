//! Slint UI integration.

slint::include_modules!();

use std::error::Error;
use std::path::Path;
use std::time::Duration;

use slint::winit_030::{WinitWindowAccessor, winit};
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

/// Centers the window once the event loop has created the native window.
pub fn center_on_startup(window: &AppWindow) {
    let weak = window.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(window) = weak.upgrade() {
            center(&window);
        }
    });
}

fn center(window: &AppWindow) {
    window.window().with_winit_window(|native| {
        // Wayland and some setups return no monitor; the window then keeps its default place.
        let Some(monitor) = native.current_monitor() else {
            return;
        };
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let window_size = native.outer_size();
        let x = monitor_pos.x + (monitor_size.width as i32 - window_size.width as i32) / 2;
        let y = monitor_pos.y + (monitor_size.height as i32 - window_size.height as i32) / 2;
        native.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
    });
}
