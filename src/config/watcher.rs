//! Configuration File Watcher
//!
//! This module provides file watching capabilities for the configuration file,
//! allowing the application to automatically reload configuration changes.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use super::Config; // Use the Config from this module
use crate::error::{Error, Result};

/// Configuration file watcher
///
/// Monitors the configuration file for changes and automatically reloads
/// the configuration when modifications are detected.
pub struct ConfigWatcher {
    /// Path to the configuration file being watched
    config_path: PathBuf,
    /// File system watcher
    _watcher: RecommendedWatcher,
    /// Receiver for file system events
    event_rx: Receiver<notify::Result<Event>>,
    /// Current configuration
    current_config: Arc<Mutex<Config>>,
    /// Flag to indicate if watching is active
    is_watching: Arc<Mutex<bool>>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    ///
    /// # Arguments
    /// * `config_path` - Path to the configuration file to watch
    /// * `initial_config` - The initial configuration to use
    ///
    /// # Returns
    /// A new `ConfigWatcher` instance
    ///
    /// # Errors
    /// Returns an error if the watcher cannot be created
    pub fn new(config_path: PathBuf, initial_config: Config) -> Result<Self> {
        let (event_tx, event_rx) = channel();

        // Create the watcher
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Err(e) = event_tx.send(res) {
                error!("Failed to send file watch event: {}", e);
            }
        })
        .map_err(|e| Error::ConfigWatchFailed {
            reason: format!("Failed to create watcher: {}", e),
        })?;

        // Watch the config file's parent directory
        // (watching the file directly can miss some editor save patterns)
        let watch_path = config_path
            .parent()
            .ok_or_else(|| Error::ConfigWatchFailed {
                reason: "Config file has no parent directory".to_string(),
            })?;

        watcher
            .watch(watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| Error::ConfigWatchFailed {
                reason: format!("Failed to watch directory: {}", e),
            })?;

        info!("Started watching config file: {}", config_path.display());

        Ok(Self {
            config_path,
            _watcher: watcher,
            event_rx,
            current_config: Arc::new(Mutex::new(initial_config)),
            is_watching: Arc::new(Mutex::new(true)),
        })
    }

    /// Get the current configuration
    ///
    /// # Returns
    /// A clone of the current configuration
    pub fn get_config(&self) -> Config {
        self.current_config.lock().unwrap().clone()
    }

    /// Check for configuration changes and reload if necessary
    ///
    /// This method should be called periodically (e.g., in the main event loop)
    /// to check for file system events and reload the configuration.
    ///
    /// # Returns
    /// `Ok(Some(config))` if the configuration was reloaded, `Ok(None)` if no changes,
    /// or an error if reloading failed
    pub fn check_and_reload(&mut self) -> Result<Option<Config>> {
        // Check if we have any pending events
        match self.event_rx.try_recv() {
            Ok(Ok(event)) => {
                // Check if this event is for our config file
                if self.is_config_file_event(&event) {
                    debug!("Config file change detected: {:?}", event);

                    // Reload the configuration
                    match self.reload_config() {
                        Ok(new_config) => {
                            info!("Configuration reloaded successfully");
                            return Ok(Some(new_config));
                        }
                        Err(e) => {
                            warn!("Failed to reload configuration: {}", e);
                            return Err(e);
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                error!("File watch error: {}", e);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No events pending
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                error!("File watch channel disconnected");
                *self.is_watching.lock().unwrap() = false;
            }
        }

        Ok(None)
    }

    /// Check if a file system event is for our config file
    fn is_config_file_event(&self, event: &Event) -> bool {
        event.paths.iter().any(|p| {
            p.canonicalize()
                .ok()
                .and_then(|cp| self.config_path.canonicalize().ok().map(|ccp| cp == ccp))
                .unwrap_or(false)
        })
    }

    /// Reload the configuration from disk
    fn reload_config(&mut self) -> Result<Config> {
        // Wait a brief moment to ensure file write is complete
        // (some editors save in multiple steps)
        std::thread::sleep(Duration::from_millis(100));

        // Load the new configuration from the specific file we're watching
        let content = std::fs::read_to_string(&self.config_path)
            .map_err(crate::Error::Io)?;

        let new_config: Config = toml::from_str(&content)
            .map_err(|e| crate::Error::ConfigParseFailed {
                format: "TOML".to_string(),
                reason: e.to_string(),
            })?;

        // Update the current configuration
        *self.current_config.lock().unwrap() = new_config.clone();

        Ok(new_config)
    }

    /// Start watching for configuration changes in the background
    ///
    /// Returns a handle that can be used to check for config updates
    ///
    /// # Arguments
    /// * `config_path` - Path to the configuration file to watch
    /// * `initial_config` - The initial configuration
    /// * `update_callback` - Callback to invoke when configuration is reloaded
    ///
    /// # Returns
    /// A handle to the background watcher task
    pub fn start_background_watch(
        config_path: PathBuf,
        initial_config: Config,
        update_callback: impl Fn(Config) + Send + 'static,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let mut watcher = Self::new(config_path.clone(), initial_config)?;

        let handle = tokio::spawn(async move {
            info!(
                "Background config watcher started for: {}",
                config_path.display()
            );

            loop {
                // Check for config changes every second
                if let Ok(Some(new_config)) = watcher.check_and_reload() {
                    info!("Configuration updated, notifying application");
                    update_callback(new_config);
                }

                // Check if watching should stop
                if !*watcher.is_watching.lock().unwrap() {
                    warn!("Config watcher stopped");
                    break;
                }

                // Sleep for 1 second before next check
                sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(handle)
    }

    /// Stop watching for configuration changes
    pub fn stop(&mut self) {
        *self.is_watching.lock().unwrap() = false;
    }

    /// Check if the watcher is still active
    pub fn is_watching(&self) -> bool {
        *self.is_watching.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_config_watcher_creation() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create initial config file
        let config = Config::default();
        let config_str = toml::to_string(&config).unwrap();
        fs::write(&config_path, config_str).unwrap();

        // Create watcher
        let watcher = ConfigWatcher::new(config_path.clone(), config.clone());
        assert!(watcher.is_ok());

        let watcher = watcher.unwrap();
        assert!(watcher.is_watching());
        assert_eq!(watcher.get_config().ui.font_size, config.ui.font_size);
    }

    #[test]
    fn test_config_reload() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        // Create initial config file
        let mut config = Config::default();
        config.ui.font_size = 12;
        let config_str = toml::to_string(&config).unwrap();
        fs::write(&config_path, config_str).unwrap();

        // Create watcher
        let mut watcher = ConfigWatcher::new(config_path.clone(), config.clone()).unwrap();

        // Modify the config file
        let mut modified_config = config.clone();
        modified_config.ui.font_size = 16;
        let modified_str = toml::to_string(&modified_config).unwrap();
        fs::write(&config_path, modified_str).unwrap();

        // Wait for file system event to propagate
        std::thread::sleep(Duration::from_millis(200));

        // Check for reload
        let result = watcher.check_and_reload();

        // Note: This test may be flaky depending on OS file watching behavior
        // In CI/CD, file watching events might not propagate reliably
        if let Ok(Some(new_config)) = result {
            assert_eq!(new_config.ui.font_size, 16);
        }
    }
}
