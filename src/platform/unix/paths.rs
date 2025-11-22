//! Unix path operations

use crate::error::{Error, Result};
use crate::platform::traits::PathOps;
use dirs;
use std::path::PathBuf;

pub struct UnixPaths;

impl UnixPaths {
    pub fn new() -> Self {
        Self
    }
}

impl PathOps for UnixPaths {
    fn config_dir(&self) -> Result<PathBuf> {
        // Use XDG_CONFIG_HOME if set, otherwise ~/.config
        if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            Ok(PathBuf::from(xdg_config))
        } else if let Some(config_dir) = dirs::config_dir() {
            Ok(config_dir)
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".config"))
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            )))
        }
    }

    fn data_dir(&self) -> Result<PathBuf> {
        // Use XDG_DATA_HOME if set, otherwise ~/.local/share
        if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
            Ok(PathBuf::from(xdg_data))
        } else if let Some(data_dir) = dirs::data_dir() {
            Ok(data_dir)
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".local").join("share"))
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine data directory",
            )))
        }
    }

    fn cache_dir(&self) -> Result<PathBuf> {
        // Use XDG_CACHE_HOME if set, otherwise ~/.cache
        if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
            Ok(PathBuf::from(xdg_cache))
        } else if let Some(cache_dir) = dirs::cache_dir() {
            Ok(cache_dir)
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".cache"))
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine cache directory",
            )))
        }
    }
}
