//! Ubuntu Linux app discovery from `.desktop` files.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::models::AppEntry;
use crate::repository::{AppRepository, RepositoryError};

const SYSTEM_APPLICATIONS_DIR: &str = "/usr/share/applications";
const SYSTEM_ICON_THEME_DIR: &str = "/usr/share/icons/hicolor";
const PIXMAPS_DIR: &str = "/usr/share/pixmaps";

pub struct LinuxAppRepository {
    application_dirs: Vec<PathBuf>,
    icon_theme_dirs: Vec<PathBuf>,
    pixmap_dir: PathBuf,
}

impl LinuxAppRepository {
    /// Later directories override earlier ones with the same file name.
    pub fn new() -> Self {
        let mut application_dirs = vec![PathBuf::from(SYSTEM_APPLICATIONS_DIR)];
        let mut icon_theme_dirs = vec![PathBuf::from(SYSTEM_ICON_THEME_DIR)];
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            application_dirs.push(home.join(".local/share/applications"));
            icon_theme_dirs.push(home.join(".local/share/icons/hicolor"));
        }
        Self {
            application_dirs,
            icon_theme_dirs,
            pixmap_dir: PathBuf::from(PIXMAPS_DIR),
        }
    }

    fn to_app_entry(&self, entry: DesktopEntry) -> AppEntry {
        let icon_path = entry
            .icon
            .as_deref()
            .and_then(|icon| self.resolve_icon(icon));
        AppEntry {
            name: entry.name,
            launch_command: entry.exec,
            icon_path,
        }
    }

    fn resolve_icon(&self, icon: &str) -> Option<PathBuf> {
        let path = Path::new(icon);
        if path.is_absolute() {
            return Some(path.to_path_buf()).filter(|path| is_supported_icon(path) && path.is_file());
        }
        self.icon_theme_dirs
            .iter()
            .find_map(|root| find_in_theme(root, icon))
            .or_else(|| icon_file_in(&self.pixmap_dir, icon))
    }
}

impl AppRepository for LinuxAppRepository {
    fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError> {
        let mut by_file_name: BTreeMap<OsString, DesktopEntry> = BTreeMap::new();
        for dir in &self.application_dirs {
            for path in desktop_files(dir)? {
                let Some(file_name) = path.file_name().map(ToOwned::to_owned) else {
                    continue;
                };
                let Ok(contents) = fs::read_to_string(&path) else {
                    continue;
                };
                match parse_desktop_entry(&contents) {
                    Some(entry) if entry.no_display => {
                        by_file_name.remove(&file_name);
                    }
                    Some(entry) => {
                        by_file_name.insert(file_name, entry);
                    }
                    None => {}
                }
            }
        }
        Ok(by_file_name
            .into_values()
            .map(|entry| self.to_app_entry(entry))
            .collect())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DesktopEntry {
    name: String,
    exec: String,
    icon: Option<String>,
    no_display: bool,
}

fn desktop_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("desktop") {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Returns `None` for files that are not launchable applications.
fn parse_desktop_entry(contents: &str) -> Option<DesktopEntry> {
    let mut in_main_group = false;
    let mut is_application = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = None;
    let mut no_display = false;

    for line in contents.lines().map(str::trim) {
        if let Some(group) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            in_main_group = group == "Desktop Entry";
            continue;
        }
        if !in_main_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Type" => is_application = value == "Application",
            "Name" => name = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "Icon" => icon = Some(value.to_string()),
            "NoDisplay" => no_display = value == "true",
            _ => {}
        }
    }

    if !is_application {
        return None;
    }
    let name = name.filter(|name| !name.is_empty())?;
    let exec = strip_field_codes(&exec?);
    if exec.is_empty() {
        return None;
    }
    Some(DesktopEntry {
        name,
        exec,
        icon: icon.filter(|icon| !icon.is_empty()),
        no_display,
    })
}

fn strip_field_codes(exec: &str) -> String {
    let mut stripped = String::with_capacity(exec.len());
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            stripped.push(c);
        } else if let Some('%') = chars.next() {
            stripped.push('%');
        }
    }
    stripped.trim().to_string()
}

fn find_in_theme(root: &Path, icon: &str) -> Option<PathBuf> {
    let mut size_dirs: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    size_dirs.sort_by_key(|dir| {
        size_rank(dir.file_name().and_then(|name| name.to_str()).unwrap_or(""))
    });
    size_dirs
        .iter()
        .find_map(|dir| icon_file_in(&dir.join("apps"), icon))
}

/// Orders the scalable directory first, then sizes from largest to smallest.
fn size_rank(dir_name: &str) -> (bool, Reverse<u32>) {
    let size = dir_name
        .split('x')
        .next()
        .and_then(|side| side.parse().ok())
        .unwrap_or(0);
    (dir_name != "scalable", Reverse(size))
}

fn icon_file_in(dir: &Path, icon: &str) -> Option<PathBuf> {
    std::iter::once(dir.join(icon))
        .chain(["svg", "png"].map(|ext| dir.join(format!("{icon}.{ext}"))))
        .find(|path| is_supported_icon(path) && path.is_file())
}

fn is_supported_icon(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("png" | "svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("flashbar-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn parses_main_group_fields() {
        let entry = parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName=Terminal\nExec=gnome-terminal %U\nIcon=utilities-terminal\n",
        )
        .expect("valid entry");
        assert_eq!(
            entry,
            DesktopEntry {
                name: "Terminal".into(),
                exec: "gnome-terminal".into(),
                icon: Some("utilities-terminal".into()),
                no_display: false,
            }
        );
    }

    #[test]
    fn ignores_localized_names() {
        let entry = parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName[de]=Terminal DE\nName=Terminal\nExec=term\n",
        )
        .expect("valid entry");
        assert_eq!(entry.name, "Terminal");
    }

    #[test]
    fn ignores_keys_from_action_groups() {
        let entry = parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName=App\nExec=app\n[Desktop Action New]\nName=New\nExec=app --new\n",
        )
        .expect("valid entry");
        assert_eq!(entry.exec, "app");
    }

    #[test]
    fn rejects_non_application_types() {
        assert!(parse_desktop_entry("[Desktop Entry]\nType=Link\nName=A\nExec=a\n").is_none());
    }

    #[test]
    fn rejects_entries_without_exec() {
        assert!(parse_desktop_entry("[Desktop Entry]\nType=Application\nName=A\n").is_none());
    }

    #[test]
    fn flags_no_display_entries() {
        let entry = parse_desktop_entry(
            "[Desktop Entry]\nType=Application\nName=A\nExec=a\nNoDisplay=true\n",
        )
        .expect("valid entry");
        assert!(entry.no_display);
    }

    #[test]
    fn strips_field_codes_and_keeps_literal_percent() {
        assert_eq!(strip_field_codes("firefox %u"), "firefox");
        assert_eq!(strip_field_codes("app --name=%% %F"), "app --name=%");
    }

    #[test]
    fn user_entry_overrides_system_and_no_display_hides() {
        let root = temp_dir("override");
        let system = root.join("system");
        let user = root.join("user");
        fs::create_dir_all(&system).unwrap();
        fs::create_dir_all(&user).unwrap();
        fs::write(
            system.join("editor.desktop"),
            "[Desktop Entry]\nType=Application\nName=System Editor\nExec=sys-editor\n",
        )
        .unwrap();
        fs::write(
            user.join("editor.desktop"),
            "[Desktop Entry]\nType=Application\nName=My Editor\nExec=my-editor\n",
        )
        .unwrap();
        fs::write(
            system.join("hidden.desktop"),
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        )
        .unwrap();
        fs::write(system.join("notes.txt"), "not a desktop file").unwrap();

        let repo = LinuxAppRepository {
            application_dirs: vec![system, user],
            icon_theme_dirs: vec![],
            pixmap_dir: root.join("pixmaps"),
        };
        let apps = repo.all_apps().expect("discovery succeeds");
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(
            apps,
            vec![AppEntry {
                name: "My Editor".into(),
                launch_command: "my-editor".into(),
                icon_path: None,
            }]
        );
    }

    #[test]
    fn icons_resolve_from_theme_then_pixmaps() {
        let root = temp_dir("icons");
        let theme = root.join("hicolor");
        fs::create_dir_all(theme.join("48x48/apps")).unwrap();
        fs::write(theme.join("48x48/apps/tool.png"), b"png").unwrap();
        let pixmaps = root.join("pixmaps");
        fs::create_dir_all(&pixmaps).unwrap();
        fs::write(pixmaps.join("legacy.svg"), b"<svg/>").unwrap();

        let repo = LinuxAppRepository {
            application_dirs: vec![],
            icon_theme_dirs: vec![theme.clone()],
            pixmap_dir: pixmaps,
        };
        let tool = repo.resolve_icon("tool");
        let legacy = repo.resolve_icon("legacy");
        let missing = repo.resolve_icon("missing");
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(tool, Some(theme.join("48x48/apps/tool.png")));
        assert_eq!(legacy, Some(root.join("pixmaps/legacy.svg")));
        assert_eq!(missing, None);
    }
}
