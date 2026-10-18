//! Search and launch logic. Depends on repository and models only.

use std::path::{Path, PathBuf};

use crate::models::AppEntry;
use crate::repository::{
    AppLauncher, AppRepository, LaunchError, RepositoryError, UserAppRepository,
};

pub const MAX_RESULTS: usize = 5;

const PREFIX_RANK: u8 = 0;
const SUBSTRING_RANK: u8 = 1;

/// Outcome of a successful launch. A failed save does not undo the launch.
pub struct LaunchOutcome {
    pub save_error: Option<RepositoryError>,
}

pub struct AppSearch<S, L> {
    user_apps: Vec<AppEntry>,
    installed_apps: Vec<AppEntry>,
    store: S,
    launcher: L,
}

impl<S: UserAppRepository, L: AppLauncher> AppSearch<S, L> {
    pub fn new(
        installed: &impl AppRepository,
        store: S,
        launcher: L,
    ) -> Result<Self, RepositoryError> {
        Ok(Self {
            user_apps: store.load()?,
            installed_apps: installed.all_apps()?,
            store,
            launcher,
        })
    }

    /// Searches the user's apps first. Installed apps are searched only when no user app matches.
    pub fn search(&self, query: &str) -> Vec<AppEntry> {
        let user_matches = rank(&self.user_apps, query);
        if user_matches.is_empty() {
            rank(&self.installed_apps, query)
        } else {
            user_matches
        }
    }

    /// Executables to choose from. A stored app always has exactly one.
    pub fn launch_options(&self, app: &AppEntry) -> Vec<PathBuf> {
        if self.is_stored(app) {
            vec![PathBuf::from(&app.launch_command)]
        } else {
            self.launcher.executable_options(app)
        }
    }

    /// Starts `exe` for `app`. On success the app is stored with `exe` as its link.
    pub fn launch(&mut self, app: &AppEntry, exe: &Path) -> Result<LaunchOutcome, LaunchError> {
        let target = AppEntry {
            name: app.name.clone(),
            launch_command: exe.to_string_lossy().into_owned(),
            icon_path: app.icon_path.clone(),
        };
        self.launcher.launch(&target)?;
        self.upsert(target);
        let save_error = self.store.save(&self.user_apps).err();
        Ok(LaunchOutcome { save_error })
    }

    fn is_stored(&self, app: &AppEntry) -> bool {
        self.user_apps
            .iter()
            .any(|stored| stored.launch_command == app.launch_command)
    }

    /// Updates the stored entry with the same link, or adds a new one.
    fn upsert(&mut self, app: AppEntry) {
        match self
            .user_apps
            .iter_mut()
            .find(|stored| stored.launch_command == app.launch_command)
        {
            Some(stored) => {
                stored.name = app.name;
                stored.icon_path = app.icon_path;
            }
            None => self.user_apps.push(app),
        }
    }
}

fn rank(apps: &[AppEntry], query: &str) -> Vec<AppEntry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<(u8, String, &AppEntry)> = apps
        .iter()
        .filter_map(|app| {
            let name = app.name.to_lowercase();
            let rank = if name.starts_with(&query) {
                PREFIX_RANK
            } else if name.contains(&query) {
                SUBSTRING_RANK
            } else {
                return None;
            };
            Some((rank, name, app))
        })
        .collect();

    matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    matches
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(_, _, app)| app.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::io;
    use std::rc::Rc;

    use super::*;

    struct FakeRepository {
        apps: Vec<AppEntry>,
    }

    impl AppRepository for FakeRepository {
        fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError> {
            Ok(self.apps.clone())
        }
    }

    struct FailingRepository;

    impl AppRepository for FailingRepository {
        fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError> {
            Err(io::Error::other("boom").into())
        }
    }

    fn app(name: &str) -> AppEntry {
        AppEntry {
            name: name.to_string(),
            launch_command: name.to_lowercase(),
            icon_path: None,
        }
    }

    fn names(apps: &[AppEntry]) -> Vec<&str> {
        apps.iter().map(|app| app.name.as_str()).collect()
    }

    /// Store fake that records every save. Clones share the same record.
    #[derive(Default)]
    struct FakeStore {
        stored: Vec<AppEntry>,
        saves: Rc<RefCell<Vec<Vec<AppEntry>>>>,
        fail_save: bool,
    }

    impl UserAppRepository for FakeStore {
        fn load(&self) -> Result<Vec<AppEntry>, RepositoryError> {
            Ok(self.stored.clone())
        }

        fn save(&self, apps: &[AppEntry]) -> Result<(), RepositoryError> {
            self.saves.borrow_mut().push(apps.to_vec());
            if self.fail_save {
                Err(io::Error::other("disk full").into())
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct FakeLauncher {
        fail: bool,
        options: Vec<PathBuf>,
        launched: Rc<RefCell<Vec<String>>>,
    }

    impl AppLauncher for FakeLauncher {
        fn launch(&self, app: &AppEntry) -> Result<(), LaunchError> {
            self.launched.borrow_mut().push(app.launch_command.clone());
            if self.fail {
                Err(LaunchError::Spawn(io::Error::other("missing")))
            } else {
                Ok(())
            }
        }

        fn executable_options(&self, _app: &AppEntry) -> Vec<PathBuf> {
            self.options.clone()
        }
    }

    fn service(
        installed: Vec<AppEntry>,
        store: FakeStore,
        launcher: FakeLauncher,
    ) -> AppSearch<FakeStore, FakeLauncher> {
        AppSearch::new(&FakeRepository { apps: installed }, store, launcher)
            .expect("fakes load")
    }

    fn search_for(apps: Vec<AppEntry>, query: &str) -> Vec<AppEntry> {
        service(apps, FakeStore::default(), FakeLauncher::default()).search(query)
    }

    #[test]
    fn empty_query_returns_nothing() {
        let results = search_for(vec![app("Code")], "");
        assert!(results.is_empty());
    }

    #[test]
    fn whitespace_only_query_returns_nothing() {
        let results = search_for(vec![app("Code")], "   ");
        assert!(results.is_empty());
    }

    #[test]
    fn query_is_trimmed() {
        let results = search_for(vec![app("Code")], "  code  ");
        assert_eq!(names(&results), vec!["Code"]);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let results = search_for(vec![app("Firefox")], "FIRE");
        assert_eq!(names(&results), vec!["Firefox"]);
    }

    #[test]
    fn prefix_matches_rank_above_substring_matches() {
        let apps = vec![app("Xcode"), app("Compass"), app("Code")];
        let results = search_for(apps, "co");
        assert_eq!(names(&results), vec!["Code", "Compass", "Xcode"]);
    }

    #[test]
    fn results_are_capped_at_max_results() {
        let apps = (1..=7).map(|i| app(&format!("App{i}"))).collect();
        let results = search_for(apps, "app");
        assert_eq!(results.len(), MAX_RESULTS);
    }

    #[test]
    fn repository_error_is_propagated() {
        let result = AppSearch::new(
            &FailingRepository,
            FakeStore::default(),
            FakeLauncher::default(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn stored_match_hides_installed_apps() {
        let store = FakeStore {
            stored: vec![app("Code")],
            ..Default::default()
        };
        let search = service(vec![app("Compass")], store, FakeLauncher::default());
        assert_eq!(names(&search.search("co")), vec!["Code"]);
    }

    #[test]
    fn installed_apps_are_searched_when_no_stored_app_matches() {
        let store = FakeStore {
            stored: vec![app("Code")],
            ..Default::default()
        };
        let search = service(vec![app("Firefox")], store, FakeLauncher::default());
        assert_eq!(names(&search.search("fire")), vec!["Firefox"]);
    }

    #[test]
    fn empty_user_list_searches_installed_apps() {
        let search = service(
            vec![app("Code"), app("Compass")],
            FakeStore::default(),
            FakeLauncher::default(),
        );
        assert_eq!(names(&search.search("co")), vec!["Code", "Compass"]);
    }

    #[test]
    fn launching_new_app_stores_it_once() {
        let saves = Rc::new(RefCell::new(Vec::new()));
        let store = FakeStore {
            saves: saves.clone(),
            ..Default::default()
        };
        let mut search = service(vec![app("Firefox")], store, FakeLauncher::default());
        let firefox = app("Firefox");

        let outcome = search
            .launch(&firefox, Path::new("firefox"))
            .expect("launch succeeds");

        assert!(outcome.save_error.is_none());
        let saved = saves.borrow();
        assert_eq!(saved.len(), 1);
        assert_eq!(names(&saved[0]), vec!["Firefox"]);
    }

    #[test]
    fn launching_stored_app_updates_entry_without_duplicate() {
        let saves = Rc::new(RefCell::new(Vec::new()));
        let store = FakeStore {
            stored: vec![app("Code")],
            saves: saves.clone(),
            ..Default::default()
        };
        let mut search = service(vec![], store, FakeLauncher::default());
        let renamed = AppEntry {
            name: "Code Editor".into(),
            launch_command: "code".into(),
            icon_path: None,
        };

        search
            .launch(&renamed, Path::new("code"))
            .expect("launch succeeds");

        let saved = saves.borrow();
        assert_eq!(names(&saved[0]), vec!["Code Editor"]);
    }

    #[test]
    fn failed_launch_changes_nothing() {
        let saves = Rc::new(RefCell::new(Vec::new()));
        let store = FakeStore {
            saves: saves.clone(),
            ..Default::default()
        };
        let launcher = FakeLauncher {
            fail: true,
            ..Default::default()
        };
        let mut search = service(vec![app("Firefox")], store, launcher);

        let result = search.launch(&app("Firefox"), Path::new("firefox"));

        assert!(result.is_err());
        assert!(saves.borrow().is_empty());
        assert_eq!(names(&search.search("fire")), vec!["Firefox"]);
    }

    #[test]
    fn stored_app_with_missing_exe_stays_after_failed_launch() {
        let store = FakeStore {
            stored: vec![AppEntry {
                name: "Old".into(),
                launch_command: "/gone/old".into(),
                icon_path: None,
            }],
            ..Default::default()
        };
        let launcher = FakeLauncher {
            fail: true,
            ..Default::default()
        };
        let mut search = service(vec![], store, launcher);
        let old = search.search("old").remove(0);

        let result = search.launch(&old, Path::new("/gone/old"));

        assert!(result.is_err());
        assert_eq!(names(&search.search("old")), vec!["Old"]);
    }

    #[test]
    fn failed_save_still_reports_successful_launch() {
        let store = FakeStore {
            fail_save: true,
            ..Default::default()
        };
        let mut search = service(vec![app("Firefox")], store, FakeLauncher::default());

        let outcome = search
            .launch(&app("Firefox"), Path::new("firefox"))
            .expect("launch succeeds");

        assert!(outcome.save_error.is_some());
    }

    #[test]
    fn picked_executable_is_stored_as_link() {
        let saves = Rc::new(RefCell::new(Vec::new()));
        let store = FakeStore {
            saves: saves.clone(),
            ..Default::default()
        };
        let mut search = service(vec![app("Code")], store, FakeLauncher::default());

        search
            .launch(&app("Code"), Path::new("/opt/code/bin/code-b"))
            .expect("launch succeeds");

        let saved = saves.borrow();
        assert_eq!(saved[0][0].launch_command, "/opt/code/bin/code-b");
    }

    #[test]
    fn stored_app_has_single_launch_option() {
        let stored = AppEntry {
            name: "Code".into(),
            launch_command: "/opt/code/code".into(),
            icon_path: None,
        };
        let store = FakeStore {
            stored: vec![stored.clone()],
            ..Default::default()
        };
        let launcher = FakeLauncher {
            options: vec![PathBuf::from("/opt/code/other")],
            ..Default::default()
        };
        let search = service(vec![], store, launcher);

        let options = search.launch_options(&stored);

        assert_eq!(options, vec![PathBuf::from("/opt/code/code")]);
    }

    #[test]
    fn installed_app_options_come_from_launcher() {
        let launcher = FakeLauncher {
            options: vec![PathBuf::from("/opt/a.exe"), PathBuf::from("/opt/b.exe")],
            ..Default::default()
        };
        let search = service(vec![], FakeStore::default(), launcher);

        let options = search.launch_options(&app("Tool"));

        assert_eq!(options.len(), 2);
    }
}
