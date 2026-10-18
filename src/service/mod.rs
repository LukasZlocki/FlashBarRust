//! Search and launch logic. Depends on repository and models only.

use crate::models::AppEntry;
use crate::repository::{AppRepository, RepositoryError};

pub const MAX_RESULTS: usize = 5;

const PREFIX_RANK: u8 = 0;
const SUBSTRING_RANK: u8 = 1;

pub struct AppSearch {
    apps: Vec<AppEntry>,
}

impl AppSearch {
    pub fn new(repository: &impl AppRepository) -> Result<Self, RepositoryError> {
        Ok(Self {
            apps: repository.all_apps()?,
        })
    }

    pub fn search(&self, query: &str) -> Vec<AppEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut matches: Vec<(u8, String, &AppEntry)> = self
            .apps
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
}

#[cfg(test)]
mod tests {
    use std::io;

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

    fn search_for(apps: Vec<AppEntry>, query: &str) -> Vec<AppEntry> {
        let search = AppSearch::new(&FakeRepository { apps }).expect("fake repository loads");
        search.search(query)
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
        assert!(AppSearch::new(&FailingRepository).is_err());
    }
}
