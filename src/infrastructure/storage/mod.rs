//! JSON storage for the apps the user launches, kept beside the executable.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::AppEntry;
use crate::repository::{RepositoryError, UserAppRepository};

const FILE_NAME: &str = "apps.json";

pub struct JsonAppStore {
    path: PathBuf,
}

impl JsonAppStore {
    /// Stores `apps.json` in the folder that contains the running executable.
    pub fn beside_executable() -> io::Result<Self> {
        let exe = std::env::current_exe()?;
        let folder = exe
            .parent()
            .ok_or_else(|| io::Error::other("executable has no parent folder"))?;
        Ok(Self::at(folder.join(FILE_NAME)))
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Serialize, Deserialize)]
struct StoredFile {
    apps: Vec<StoredApp>,
}

#[derive(Serialize, Deserialize)]
struct StoredApp {
    name: String,
    link: String,
    #[serde(default)]
    icon: Option<String>,
}

impl From<&AppEntry> for StoredApp {
    fn from(app: &AppEntry) -> Self {
        Self {
            name: app.name.clone(),
            link: app.launch_command.clone(),
            icon: app
                .icon_path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}

impl From<StoredApp> for AppEntry {
    fn from(stored: StoredApp) -> Self {
        Self {
            name: stored.name,
            launch_command: stored.link,
            icon_path: stored.icon.map(PathBuf::from),
        }
    }
}

impl UserAppRepository for JsonAppStore {
    /// A missing file means no stored apps yet. An unreadable file is an error and is never overwritten.
    fn load(&self) -> Result<Vec<AppEntry>, RepositoryError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let file: StoredFile = serde_json::from_str(&text)
            .map_err(|err| RepositoryError::InvalidData(err.to_string()))?;
        Ok(file.apps.into_iter().map(AppEntry::from).collect())
    }

    /// Writes the file sorted by name through a temporary file, so a failed write keeps the old file.
    fn save(&self, apps: &[AppEntry]) -> Result<(), RepositoryError> {
        let mut sorted: Vec<&AppEntry> = apps.iter().collect();
        sorted.sort_by_key(|app| app.name.to_lowercase());
        let file = StoredFile {
            apps: sorted.into_iter().map(StoredApp::from).collect(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|err| RepositoryError::InvalidData(err.to_string()))?;

        if let Some(folder) = self.path.parent() {
            fs::create_dir_all(folder)?;
        }
        let temp_path = self.path.with_extension("json.tmp");
        fs::write(&temp_path, json)?;
        fs::rename(&temp_path, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn app(name: &str, link: &str, icon: Option<&str>) -> AppEntry {
        AppEntry {
            name: name.into(),
            launch_command: link.into(),
            icon_path: icon.map(PathBuf::from),
        }
    }

    /// Returns a fresh folder path for the test. The folder itself is not created.
    fn temp_folder(label: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "flashbar-storage-{}-{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    fn store_in(folder: &Path) -> JsonAppStore {
        JsonAppStore::at(folder.join(FILE_NAME))
    }

    #[test]
    fn missing_file_loads_as_empty() {
        let folder = temp_folder("missing");
        let apps = store_in(&folder).load().expect("missing file is not an error");
        assert!(apps.is_empty());
    }

    #[test]
    fn save_then_load_round_trips_sorted_by_name() {
        let folder = temp_folder("round-trip");
        let store = store_in(&folder);
        let apps = vec![
            app("Firefox", "/usr/bin/firefox", None),
            app("code", "/opt/code/code", Some("/opt/code/icon.png")),
        ];

        store.save(&apps).expect("save succeeds");
        let loaded = store.load().expect("load succeeds");
        fs::remove_dir_all(&folder).unwrap();

        assert_eq!(
            loaded,
            vec![
                app("code", "/opt/code/code", Some("/opt/code/icon.png")),
                app("Firefox", "/usr/bin/firefox", None),
            ]
        );
    }

    #[test]
    fn save_creates_missing_folder_and_leaves_no_temp_file() {
        let folder = temp_folder("create-folder");
        let store = store_in(&folder);

        store.save(&[app("Tool", "/usr/bin/tool", None)]).expect("save succeeds");
        let temp_exists = folder.join("apps.json.tmp").exists();
        let file_exists = folder.join(FILE_NAME).exists();
        fs::remove_dir_all(&folder).unwrap();

        assert!(file_exists);
        assert!(!temp_exists);
    }

    #[test]
    fn invalid_file_is_reported_and_not_overwritten() {
        let folder = temp_folder("invalid");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(FILE_NAME);
        fs::write(&path, "{ not json").unwrap();
        let store = JsonAppStore::at(path.clone());

        let result = store.load();
        let content = fs::read_to_string(&path).unwrap();
        fs::remove_dir_all(&folder).unwrap();

        assert!(matches!(result, Err(RepositoryError::InvalidData(_))));
        assert_eq!(content, "{ not json");
    }

    #[test]
    fn missing_icon_field_is_allowed() {
        let folder = temp_folder("no-icon");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(FILE_NAME);
        fs::write(&path, r#"{"apps":[{"name":"Tool","link":"/usr/bin/tool"}]}"#).unwrap();

        let loaded = JsonAppStore::at(path).load().expect("load succeeds");
        fs::remove_dir_all(&folder).unwrap();

        assert_eq!(loaded, vec![app("Tool", "/usr/bin/tool", None)]);
    }
}
