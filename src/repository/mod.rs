//! Traits describing access to application data. Depends on models only.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::models::AppEntry;

#[derive(Debug)]
pub enum RepositoryError {
    Io(std::io::Error),
    InvalidData(String),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "file access failed: {err}"),
            Self::InvalidData(message) => write!(f, "invalid data: {message}"),
        }
    }
}

impl Error for RepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::InvalidData(_) => None,
        }
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Installed apps found on the system.
pub trait AppRepository {
    fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError>;
}

/// Apps the user has launched before, persisted between runs.
pub trait UserAppRepository {
    fn load(&self) -> Result<Vec<AppEntry>, RepositoryError>;
    fn save(&self, apps: &[AppEntry]) -> Result<(), RepositoryError>;
}

#[derive(Debug)]
pub enum LaunchError {
    Spawn(std::io::Error),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(err) => write!(f, "{err}"),
        }
    }
}

impl Error for LaunchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(err) => Some(err),
        }
    }
}

pub trait AppLauncher {
    fn launch(&self, app: &AppEntry) -> Result<(), LaunchError>;

    /// Executables the user can choose from for an app that is not stored yet.
    fn executable_options(&self, app: &AppEntry) -> Vec<PathBuf> {
        vec![PathBuf::from(&app.launch_command)]
    }
}
