//! Configuration File Loading
//!
//! Handles loading and saving configuration files from various locations
//! with support for multiple formats and fallback mechanisms.

use super::Config;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration file loader
pub struct ConfigLoader {
    /// Search paths for configuration files
    search_paths: Vec<PathBuf>,
    /// Supported configuration file formats
    supported_formats: Vec<ConfigFormat>,
    /// Current configuration file path (if loaded)
    current_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFormat {
    /// TOML format
    Toml,
    /// JSON format
    Json,
    /// YAML format (if yaml feature is enabled)
    #[cfg(feature = "yaml")]
    Yaml,
}

#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Whether to create default config if none exists
    pub create_default: bool,
    /// Whether to merge with default config
    pub merge_defaults: bool,
    /// Whether to validate configuration after loading
    pub validate: bool,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            create_default: true,
            merge_defaults: true,
            validate: true,
        }
    }
}

impl ConfigLoader {
    /// Create a new configuration loader
    pub fn new() -> Self {
        Self {
            search_paths: Self::get_search_paths(),
            supported_formats: vec![ConfigFormat::Toml, ConfigFormat::Json],
            current_path: None,
        }
    }

    /// Load configuration with default options
    pub fn load() -> Result<Config> {
        Self::load_with_options(LoadOptions::default())
    }

    /// Load configuration with custom options
    pub fn load_with_options(options: LoadOptions) -> Result<Config> {
        let mut loader = Self::new();

        // Try to find and load existing configuration
        if let Some((path, config)) = loader.find_and_load_config()? {
            loader.current_path = Some(path);

            let config = if options.merge_defaults {
                loader.merge_with_defaults(config)
            } else {
                config
            };

            if options.validate {
                loader.validate_config(&config)?;
            }

            return Ok(config);
        }

        // No configuration found, create default if requested
        if options.create_default {
            let config = Config::default();
            if options.validate {
                loader.validate_config(&config)?;
            }
            Ok(config)
        } else {
            Err(Error::ConfigNotFound)
        }
    }

    /// Save configuration to the current path or default location
    pub fn save(&self, config: &Config) -> Result<PathBuf> {
        let path = self
            .current_path
            .clone()
            .unwrap_or_else(Self::get_default_config_path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Save in TOML format by default
        let toml_content =
            toml::to_string_pretty(config).map_err(|e| Error::ConfigSerializationFailed {
                format: "TOML".to_string(),
                reason: e.to_string(),
            })?;

        fs::write(&path, toml_content)?;
        Ok(path)
    }

    /// Save configuration to a specific path
    pub fn save_to_path(&self, config: &Config, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Determine format from file extension
        let content = match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => serde_json::to_string_pretty(config).map_err(|e| {
                Error::ConfigSerializationFailed {
                    format: "JSON".to_string(),
                    reason: e.to_string(),
                }
            })?,
            Some("toml") => {
                toml::to_string_pretty(config).map_err(|e| Error::ConfigSerializationFailed {
                    format: "TOML".to_string(),
                    reason: e.to_string(),
                })?
            }
            _ => toml::to_string_pretty(config).map_err(|e| Error::ConfigSerializationFailed {
                format: "TOML".to_string(),
                reason: e.to_string(),
            })?,
        };

        fs::write(path, content)?;
        Ok(())
    }

    /// Find and load configuration from search paths
    fn find_and_load_config(&self) -> Result<Option<(PathBuf, Config)>> {
        for path in &self.search_paths {
            for format in &self.supported_formats {
                let config_path = self.get_config_path_for_format(path, *format);

                if config_path.exists() {
                    match self.load_config_file(&config_path, *format) {
                        Ok(config) => return Ok(Some((config_path, config))),
                        Err(e) => {
                            // Log warning but continue searching
                            eprintln!(
                                "Failed to load config from {}: {}",
                                config_path.display(),
                                e
                            );
                            continue;
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Load a specific configuration file
    fn load_config_file(&self, path: &Path, format: ConfigFormat) -> Result<Config> {
        let content = fs::read_to_string(path)?;

        match format {
            ConfigFormat::Toml => toml::from_str(&content).map_err(|e| Error::ConfigParseFailed {
                format: "TOML".to_string(),
                reason: e.to_string(),
            }),
            ConfigFormat::Json => {
                serde_json::from_str(&content).map_err(|e| Error::ConfigParseFailed {
                    format: "JSON".to_string(),
                    reason: e.to_string(),
                })
            }
            #[cfg(feature = "yaml")]
            ConfigFormat::Yaml => {
                serde_yaml::from_str(&content).map_err(|e| Error::ConfigParseFailed {
                    format: "YAML".to_string(),
                    reason: e.to_string(),
                })
            }
        }
    }

    /// Get configuration file path for a specific format
    fn get_config_path_for_format(&self, base_path: &Path, format: ConfigFormat) -> PathBuf {
        let extension = match format {
            ConfigFormat::Toml => "toml",
            ConfigFormat::Json => "json",
            #[cfg(feature = "yaml")]
            ConfigFormat::Yaml => "yaml",
        };

        base_path.with_extension(extension)
    }

    /// Get default search paths for configuration files
    fn get_search_paths() -> Vec<PathBuf> {
        use crate::platform::Platform;

        let mut paths = Vec::new();
        let path_ops = Platform::paths();

        // Use platform abstraction for config directory
        if let Ok(config_dir) = path_ops.config_dir() {
            paths.push(config_dir.join("mosaicterm"));
            paths.push(config_dir.join("mosaicterm").join("config"));
        }

        // XDG config home fallback (for platforms that might set it)
        if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
            paths.push(PathBuf::from(xdg_config).join("mosaicterm"));
        }

        // Home directory fallbacks
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".mosaicterm"));
            paths.push(home.join(".config").join("mosaicterm"));
        }

        // Current working directory
        if let Ok(cwd) = env::current_dir() {
            paths.push(cwd.join(".mosaicterm"));
        }

        paths
    }

    /// Get the default configuration path
    fn get_default_config_path() -> PathBuf {
        use crate::platform::Platform;

        let path_ops = Platform::paths();
        path_ops
            .config_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("mosaicterm")
            .join("config.toml")
    }

    /// Merge configuration with defaults
    fn merge_with_defaults(&self, config: Config) -> Config {
        // For now, just return the loaded config
        // In a full implementation, this would intelligently merge
        // user config with defaults
        config
    }

    /// Validate configuration
    fn validate_config(&self, config: &Config) -> Result<()> {
        // UI validation
        if config.ui.font_size == 0 {
            return Err(Error::ConfigValidationFailed {
                field: "ui.font_size".to_string(),
                reason: "Font size must be greater than 0".to_string(),
            });
        }

        if config.ui.font_size > 72 {
            return Err(Error::ConfigValidationFailed {
                field: "ui.font_size".to_string(),
                reason: "Font size cannot exceed 72".to_string(),
            });
        }

        if config.ui.theme_name.trim().is_empty() {
            return Err(Error::ConfigValidationFailed {
                field: "ui.theme_name".to_string(),
                reason: "Theme name cannot be empty".to_string(),
            });
        }

        // Terminal validation
        if config.terminal.shell_path.as_os_str().is_empty() {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.shell_path".to_string(),
                reason: "Shell path cannot be empty".to_string(),
            });
        }

        if config.terminal.scrollback_buffer == 0 {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.scrollback_buffer".to_string(),
                reason: "Scrollback buffer must be greater than 0".to_string(),
            });
        }

        if config.terminal.scrollback_buffer > 1000000 {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.scrollback_buffer".to_string(),
                reason: "Scrollback buffer cannot exceed 1,000,000".to_string(),
            });
        }

        if config.terminal.prompt_format.is_empty() {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.prompt_format".to_string(),
                reason: "Prompt format cannot be empty".to_string(),
            });
        }

        // PTY validation
        if config.pty.buffer_size == 0 {
            return Err(Error::ConfigValidationFailed {
                field: "pty.buffer_size".to_string(),
                reason: "PTY buffer size must be greater than 0".to_string(),
            });
        }

        if config.pty.buffer_size > 10 * 1024 * 1024 {
            return Err(Error::ConfigValidationFailed {
                field: "pty.buffer_size".to_string(),
                reason: "PTY buffer size cannot exceed 10MB".to_string(),
            });
        }

        // Timeout validation
        if config.terminal.timeout.regular_command_timeout_secs > 3600 {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.timeout.regular_command_timeout_secs".to_string(),
                reason: "Regular command timeout cannot exceed 1 hour (3600 seconds)".to_string(),
            });
        }

        if config.terminal.timeout.interactive_command_timeout_secs > 86400 {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.timeout.interactive_command_timeout_secs".to_string(),
                reason: "Interactive command timeout cannot exceed 24 hours (86400 seconds)"
                    .to_string(),
            });
        }

        if config.terminal.timeout.kill_grace_period_secs > 300 {
            return Err(Error::ConfigValidationFailed {
                field: "terminal.timeout.kill_grace_period_secs".to_string(),
                reason: "Kill grace period cannot exceed 5 minutes (300 seconds)".to_string(),
            });
        }

        Ok(())
    }

    /// Get the current configuration file path
    pub fn current_path(&self) -> Option<&Path> {
        self.current_path.as_deref()
    }

    /// List all search paths
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Add a custom search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Clear all search paths and add a single path
    pub fn set_search_path(&mut self, path: PathBuf) {
        self.search_paths = vec![path];
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration migration utilities
pub mod migration {
    use super::*;

    /// Configuration version information
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ConfigVersion {
        pub version: String,
        pub last_updated: chrono::DateTime<chrono::Utc>,
    }

    /// Migrate configuration from older versions
    pub fn migrate_config(mut config: Config, _from_version: &str) -> Result<Config> {
        // Simplified migration logic - in a real implementation,
        // this would handle version-specific migrations
        let _current_version = env!("CARGO_PKG_VERSION");

        // Apply basic migrations
        // Ensure scrollback lines is at least reasonable
        config.ui.scrollback_lines = config.ui.scrollback_lines.max(100);

        // Ensure font size is reasonable
        config.ui.font_size = config.ui.font_size.clamp(8, 72);

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_loader_creation() {
        let loader = ConfigLoader::new();
        assert!(!loader.search_paths.is_empty());
        assert!(!loader.supported_formats.is_empty());
    }

    #[test]
    fn test_search_paths() {
        let paths = ConfigLoader::get_search_paths();
        assert!(!paths.is_empty());
        // Should contain user config directory
        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("mosaicterm")));
    }

    #[test]
    fn test_default_config_path() {
        let path = ConfigLoader::get_default_config_path();
        assert!(path.to_string_lossy().contains("mosaicterm"));
        assert!(path.extension().unwrap_or_default() == "toml");
    }

    #[test]
    fn test_config_format_extensions() {
        let loader = ConfigLoader::new();
        let base = PathBuf::from("test");

        assert_eq!(
            loader
                .get_config_path_for_format(&base, ConfigFormat::Toml)
                .extension()
                .unwrap(),
            "toml"
        );
        assert_eq!(
            loader
                .get_config_path_for_format(&base, ConfigFormat::Json)
                .extension()
                .unwrap(),
            "json"
        );
    }

    #[test]
    fn test_load_nonexistent_config() {
        let result = ConfigLoader::load_with_options(LoadOptions {
            create_default: false,
            merge_defaults: false,
            validate: false,
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let mut loader = ConfigLoader::new();
        loader.set_search_path(temp_dir.path().to_path_buf());

        let config = Config::default();

        // Save config
        loader.save_to_path(&config, &config_path).unwrap();

        // Verify file exists
        assert!(config_path.exists());

        // Load config
        let loaded = loader
            .load_config_file(&config_path, ConfigFormat::Toml)
            .unwrap();

        // Compare (simplified check)
        assert_eq!(config.ui.font_size, loaded.ui.font_size);
    }

    #[test]
    fn test_config_validation() {
        let loader = ConfigLoader::new();

        // Valid config
        let valid_config = Config::default();
        assert!(loader.validate_config(&valid_config).is_ok());

        // Invalid config (would need to create invalid configs for testing)
    }

    #[test]
    fn test_migration() {
        let config = Config::default();
        let migrated = migration::migrate_config(config, "0.1.0").unwrap();
        assert_eq!(migrated.ui.scrollback_lines, 100000); // Should be unchanged for valid config
    }

    #[test]
    fn test_loader_options() {
        let options = LoadOptions::default();
        assert!(options.create_default);
        assert!(options.merge_defaults);
        assert!(options.validate);
    }
}
