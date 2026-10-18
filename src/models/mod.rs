//! Core data types. Depends on no other layer.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEntry {
    pub name: String,
    pub launch_command: String,
    pub icon_path: Option<PathBuf>,
}
