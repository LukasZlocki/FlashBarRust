//! Platform implementations and Slint UI integration. Depends on service, repository and models.

pub mod storage;
pub mod ui;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;
