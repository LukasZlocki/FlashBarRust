//! Windows 11 app discovery from Start Menu shortcuts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::models::AppEntry;
use crate::repository::{AppRepository, RepositoryError};

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
                let launch_command = path.to_str()?.to_string();
                Some(AppEntry {
                    name,
                    launch_command,
                    icon_path: None,
                })
            })
            .collect();
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        apps.dedup_by(|a, b| a.name == b.name);
        Ok(apps)
    }
}
