mod infrastructure;
mod models;
mod repository;
mod service;

use std::cell::RefCell;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use infrastructure::storage::JsonAppStore;
use infrastructure::ui::{self, AppWindow, ResultRow};
use models::AppEntry;
use repository::AppRepository;
use service::{AppSearch, MAX_RESULTS};
use slint::{ComponentHandle, ModelRc, VecModel};

#[cfg(target_os = "linux")]
type PlatformLauncher = infrastructure::linux::LinuxAppLauncher;

#[cfg(target_os = "windows")]
type PlatformLauncher = infrastructure::windows::WindowsAppLauncher;

#[cfg(target_os = "linux")]
fn platform_repository() -> impl AppRepository {
    infrastructure::linux::LinuxAppRepository::new()
}

#[cfg(target_os = "windows")]
fn platform_repository() -> impl AppRepository {
    infrastructure::windows::WindowsAppRepository::new()
}

#[cfg(target_os = "linux")]
fn platform_launcher() -> PlatformLauncher {
    infrastructure::linux::LinuxAppLauncher
}

#[cfg(target_os = "windows")]
fn platform_launcher() -> PlatformLauncher {
    infrastructure::windows::WindowsAppLauncher
}

/// Exe picker opened for an installed app that has more than one executable.
#[derive(Clone)]
struct Picker {
    app: AppEntry,
    exes: Vec<PathBuf>,
}

struct State {
    search: AppSearch<JsonAppStore, PlatformLauncher>,
    /// Rows currently shown when no picker is open.
    results: Vec<AppEntry>,
    picker: Option<Picker>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let store = JsonAppStore::beside_executable()?;
    let search = AppSearch::new(&platform_repository(), store, platform_launcher())?;
    let window = ui::create_window()?;

    let rows = Rc::new(VecModel::<ResultRow>::default());
    window.set_results(ModelRc::from(rows.clone()));
    let state = Rc::new(RefCell::new(State {
        search,
        results: Vec::new(),
        picker: None,
    }));

    {
        let state = state.clone();
        let rows = rows.clone();
        let weak = window.as_weak();
        window.on_query_changed(move |query| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            state.picker = None;
            state.results = state.search.search(&query);
            window.set_status_text("".into());
            let list = state.results.iter().map(ui::app_row).collect();
            show(&window, &rows, list);
        });
    }

    {
        let state = state.clone();
        let rows = rows.clone();
        let weak = window.as_weak();
        window.on_launch_requested(move |index| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let index = index as usize;
            let mut state = state.borrow_mut();

            if let Some(picker) = state.picker.clone() {
                let Some(exe) = picker.exes.get(index) else {
                    return;
                };
                if launch_app(&mut state, &window, &picker.app, exe) {
                    state.picker = None;
                    let _ = window.hide();
                }
                return;
            }

            let Some(app) = state.results.get(index).cloned() else {
                return;
            };
            let exes = state.search.launch_options(&app);
            if exes.len() > 1 {
                let list = exes
                    .iter()
                    .take(MAX_RESULTS)
                    .map(|exe| ui::exe_row(exe))
                    .collect();
                window.set_status_text(
                    "Choose the program to start. Enter starts it, Esc goes back.".into(),
                );
                show(&window, &rows, list);
                state.picker = Some(Picker { app, exes });
            } else if let Some(exe) = exes.first() {
                if launch_app(&mut state, &window, &app, exe) {
                    let _ = window.hide();
                }
            }
        });
    }

    {
        let state = state.clone();
        let rows = rows.clone();
        let weak = window.as_weak();
        window.on_escape_pressed(move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            if state.picker.take().is_some() {
                window.set_status_text("".into());
                let list = state.results.iter().map(ui::app_row).collect();
                show(&window, &rows, list);
            } else {
                let _ = window.hide();
            }
        });
    }

    ui::center_on_startup(&window);
    window.run()?;
    Ok(())
}

/// Replaces the visible rows and moves the selection back to the first row.
fn show(window: &AppWindow, rows: &VecModel<ResultRow>, list: Vec<ResultRow>) {
    rows.set_vec(list);
    window.set_selected(0);
}

/// Starts `exe` for `app`. Returns `true` when the launch succeeded. A failure is shown in the window.
fn launch_app(state: &mut State, window: &AppWindow, app: &AppEntry, exe: &Path) -> bool {
    match state.search.launch(app, exe) {
        Ok(outcome) => {
            if let Some(err) = outcome.save_error {
                eprintln!("FlashBar: launched {}, but could not save apps.json: {err}", app.name);
            }
            true
        }
        Err(err) => {
            window.set_status_text(format!("Could not start {}: {err}", app.name).into());
            false
        }
    }
}
