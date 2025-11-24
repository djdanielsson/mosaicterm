//! Windows path operations

use crate::error::{Error, Result};
use crate::platform::traits::PathOps;
use dirs;
use std::path::PathBuf;

pub struct WindowsPaths;

impl WindowsPaths {
    pub fn new() -> Self {
        Self
    }
}

impl PathOps for WindowsPaths {
    fn config_dir(&self) -> Result<PathBuf> {
        // Use AppData\Roaming on Windows
        if let Some(config_dir) = dirs::config_dir() {
            Ok(config_dir)
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            )))
        }
    }

    fn data_dir(&self) -> Result<PathBuf> {
        // Use AppData\Local on Windows
        if let Some(data_dir) = dirs::data_local_dir() {
            Ok(data_dir)
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine data directory",
            )))
        }
    }

    fn cache_dir(&self) -> Result<PathBuf> {
        // Use AppData\Local on Windows (same as data_dir)
        self.data_dir()
    }
}
