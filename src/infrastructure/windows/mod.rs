//! Windows 11 app discovery from Start Menu shortcuts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::AppEntry;
use crate::repository::{AppLauncher, AppRepository, LaunchError, RepositoryError};

pub struct WindowsAppRepository {
    start_menu_dirs: Vec<PathBuf>,
}

impl WindowsAppRepository {
    pub fn new() -> Self {
        let start_menu = r"Microsoft\Windows\Start Menu\Programs";
        let start_menu_dirs = [std::env::var_os("ProgramData"), std::env::var_os("APPDATA")]
            .into_iter()
            .flatten()
            .map(|base| PathBuf::from(base).join(start_menu))
            .collect();
        Self { start_menu_dirs }
    }
}

fn collect_shortcuts(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_shortcuts(&path, found)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("lnk") {
            found.push(path);
        }
    }
    Ok(())
}

impl AppRepository for WindowsAppRepository {
    fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError> {
        let mut shortcuts = Vec::new();
        for dir in &self.start_menu_dirs {
            collect_shortcuts(dir, &mut shortcuts)?;
        }

        let mut apps: Vec<AppEntry> = shortcuts
            .into_iter()
            .filter_map(|path| {
                let name = path.file_stem()?.to_str()?.to_string();
                let target = shortcut_target(&path)?;
                Some(AppEntry {
                    name,
                    launch_command: target.to_str()?.to_string(),
                    icon_path: None,
                })
            })
            .collect();
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);
        Ok(apps)
    }
}

/// The `.exe` a shortcut starts, if that file exists. Shortcuts to documents, folders,
/// or URLs are skipped.
///
/// Reads the local base path from the link info instead of `ShellLink::link_target`,
/// which can panic on malformed shortcuts.
fn shortcut_target(lnk_path: &Path) -> Option<PathBuf> {
    let shortcut = lnk::ShellLink::open(lnk_path, lnk::encoding::WINDOWS_1252).ok()?;
    let info = shortcut.link_info().as_ref()?;
    let base = info
        .local_base_path_unicode()
        .as_deref()
        .or(info.local_base_path())?;
    let suffix = info
        .common_path_suffix_unicode()
        .as_deref()
        .unwrap_or(info.common_path_suffix());

    let target = if suffix.is_empty() {
        PathBuf::from(base)
    } else {
        Path::new(base).join(suffix)
    };
    is_existing_exe(&target).then_some(target)
}

fn is_existing_exe(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && path.is_file()
}

/// The `.exe` files in the folder of `exe`, sorted by name. `exe` itself is included when it exists.
fn exe_files_in_folder_of(exe: &Path) -> Vec<PathBuf> {
    let Some(dir) = exe.parent() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut exes: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| is_existing_exe(path))
        .collect();
    exes.sort();
    exes
}

/// Starts apps by running their `.exe`, with the app's folder as the working directory.
pub struct WindowsAppLauncher;

impl AppLauncher for WindowsAppLauncher {
    fn launch(&self, app: &AppEntry) -> Result<(), LaunchError> {
        let exe = Path::new(&app.launch_command);
        let mut command = Command::new(exe);
        if let Some(dir) = exe.parent() {
            command.current_dir(dir);
        }
        let mut child = command.spawn().map_err(LaunchError::Spawn)?;
        // Reap the child in the background so it does not stay a zombie process.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }

    /// The app's own `.exe` first, then the other `.exe` files in its folder.
    fn executable_options(&self, app: &AppEntry) -> Vec<PathBuf> {
        let target = PathBuf::from(&app.launch_command);
        let mut options = vec![target.clone()];
        options.extend(
            exe_files_in_folder_of(&target)
                .into_iter()
                .filter(|exe| *exe != target),
        );
        options
    }
}
