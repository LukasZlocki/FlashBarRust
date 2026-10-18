//! Traits describing access to application data. Depends on models only.

use std::error::Error;
use std::fmt;

use crate::models::AppEntry;

#[derive(Debug)]
pub enum RepositoryError {
    Io(std::io::Error),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "application discovery failed: {err}"),
        }
    }
}

impl Error for RepositoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

pub trait AppRepository {
    fn all_apps(&self) -> Result<Vec<AppEntry>, RepositoryError>;
}
