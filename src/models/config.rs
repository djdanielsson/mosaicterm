//! Configuration Model
//!
//! Application configuration settings for MosaicTerm.
//! This model handles all user-configurable settings including
//! UI theme, terminal settings, and shell configuration.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Main configuration structure for MosaicTerm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// UI configuration
    pub ui: UiConfig,

    /// Terminal configuration
    pub terminal: TerminalConfig,

    /// Key bindings configuration
    pub key_bindings: KeyBindingsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            terminal: TerminalConfig::default(),
            key_bindings: KeyBindingsConfig::default(),
        }
    }
}

/// UI-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Font family for terminal text
    pub font_family: String,

    /// Font size in points
    pub font_size: u32,

    /// Maximum number of lines to keep in history
    pub scrollback_lines: usize,

    /// UI theme
    pub theme: Theme,

    /// Window dimensions
    pub window_width: u32,
    pub window_height: u32,

    /// Whether to start maximized
    pub start_maximized: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            font_family: "Monaco".to_string(),
            font_size: 12,
            scrollback_lines: 1000,
            theme: Theme::default(),
            window_width: 1200,
            window_height: 800,
            start_maximized: false,
        }
    }
}

impl UiConfig {
    /// Validate the UI configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.font_size < 8 || self.font_size > 72 {
            return Err(ConfigError::InvalidFontSize(self.font_size));
        }
        if self.scrollback_lines == 0 || self.scrollback_lines > 100000 {
            return Err(ConfigError::InvalidScrollbackLines(self.scrollback_lines));
        }
        if self.window_width < 400 || self.window_height < 300 {
            return Err(ConfigError::InvalidWindowSize(self.window_width, self.window_height));
        }
        Ok(())
    }
}

/// Terminal-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Path to shell executable
    pub shell_path: PathBuf,

    /// Shell arguments
    pub shell_args: Vec<String>,

    /// Working directory for new terminals
    pub working_directory: Option<PathBuf>,

    /// Environment variables to set
    pub environment: std::collections::HashMap<String, String>,

    /// Whether to inherit parent environment
    pub inherit_env: bool,

    /// Terminal scrollback buffer size
    pub scrollback_buffer: usize,

    /// Whether to enable mouse support
    pub mouse_support: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell_path: PathBuf::from("/bin/zsh"),
            shell_args: vec!["--login".to_string()],
            working_directory: None,
            environment: std::collections::HashMap::new(),
            inherit_env: true,
            scrollback_buffer: 10000,
            mouse_support: true,
        }
    }
}

impl TerminalConfig {
    /// Validate the terminal configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.shell_path.exists() {
            return Err(ConfigError::ShellNotFound(self.shell_path.clone()));
        }
        if !self.shell_path.is_file() {
            return Err(ConfigError::InvalidShellPath(self.shell_path.clone()));
        }

        // Check if shell is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&self.shell_path)?;
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return Err(ConfigError::ShellNotExecutable(self.shell_path.clone()));
            }
        }

        if self.scrollback_buffer > 100000 {
            return Err(ConfigError::InvalidScrollbackBuffer(self.scrollback_buffer));
        }

        Ok(())
    }

    /// Get the effective environment variables
    pub fn get_effective_environment(&self) -> std::collections::HashMap<String, String> {
        let mut env = if self.inherit_env {
            std::env::vars().collect()
        } else {
            std::collections::HashMap::new()
        };

        // Override with custom environment variables
        for (key, value) in &self.environment {
            env.insert(key.clone(), value.clone());
        }

        env
    }
}

/// UI theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Background color
    pub background: Color,

    /// Foreground/text color
    pub foreground: Color,

    /// Accent color for UI elements
    pub accent: Color,

    /// Success color (green)
    pub success: Color,

    /// Error color (red)
    pub error: Color,

    /// Warning color (yellow)
    pub warning: Color,

    /// Selection color
    pub selection: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::new(0.1, 0.1, 0.1, 1.0), // Dark gray
            foreground: Color::new(0.9, 0.9, 0.9, 1.0), // Light gray
            accent: Color::new(0.3, 0.6, 0.9, 1.0),     // Blue
            success: Color::new(0.3, 0.7, 0.3, 1.0),    // Green
            error: Color::new(0.8, 0.3, 0.3, 1.0),      // Red
            warning: Color::new(0.8, 0.6, 0.2, 1.0),    // Yellow
            selection: Color::new(0.2, 0.4, 0.6, 0.5),  // Semi-transparent blue
        }
    }
}

/// Key bindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindingsConfig {
    /// Copy key binding
    pub copy: String,

    /// Paste key binding
    pub paste: String,

    /// New tab key binding
    pub new_tab: String,

    /// Close tab key binding
    pub close_tab: String,

    /// Clear terminal key binding
    pub clear: String,

    /// Search key binding
    pub search: String,
}

impl Default for KeyBindingsConfig {
    fn default() -> Self {
        Self {
            copy: "Command+C".to_string(),
            paste: "Command+V".to_string(),
            new_tab: "Command+T".to_string(),
            close_tab: "Command+W".to_string(),
            clear: "Command+K".to_string(),
            search: "Command+F".to_string(),
        }
    }
}

/// RGBA color representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create color from hex string (e.g., "#FF0000" or "#FF0000FF")
    pub fn from_hex(hex: &str) -> Result<Self, ConfigError> {
        let hex = hex.trim_start_matches('#');
        let len = hex.len();

        let (r, g, b, a) = match len {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)?;
                let g = u8::from_str_radix(&hex[2..4], 16)?;
                let b = u8::from_str_radix(&hex[4..6], 16)?;
                (r, g, b, 255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)?;
                let g = u8::from_str_radix(&hex[2..4], 16)?;
                let b = u8::from_str_radix(&hex[4..6], 16)?;
                let a = u8::from_str_radix(&hex[6..8], 16)?;
                (r, g, b, a)
            }
            _ => return Err(ConfigError::InvalidHexColor(hex.to_string())),
        };

        Ok(Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        })
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;
        let a = (self.a * 255.0).round() as u8;

        if (a as f32 / 255.0 - 1.0).abs() < f32::EPSILON {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
        }
    }
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid font size: {0} (must be between 8 and 72)")]
    InvalidFontSize(u32),

    #[error("Invalid scrollback lines: {0} (must be between 1 and 100000)")]
    InvalidScrollbackLines(usize),

    #[error("Invalid window size: {0}x{1} (minimum 400x300)")]
    InvalidWindowSize(u32, u32),

    #[error("Shell not found: {0}")]
    ShellNotFound(PathBuf),

    #[error("Invalid shell path: {0}")]
    InvalidShellPath(PathBuf),

    #[error("Shell not executable: {0}")]
    ShellNotExecutable(PathBuf),

    #[error("Invalid scrollback buffer: {0} (maximum 100000)")]
    InvalidScrollbackBuffer(usize),

    #[error("Invalid hex color: {0}")]
    InvalidHexColor(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse int error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
}

impl Config {
    /// Load configuration from default locations
    pub fn load() -> Result<Self, ConfigError> {
        // TODO: Implement file-based configuration loading
        let config = Self::default();

        // Try to load from standard locations
        let config_paths = Self::get_config_paths();

        for path in config_paths {
            if path.exists() {
                // TODO: Load and parse configuration file
                // For now, return default config
                break;
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to default location
    pub fn save(&self) -> Result<(), ConfigError> {
        self.validate()?;

        // TODO: Implement configuration saving
        // For now, just validate
        Ok(())
    }

    /// Get configuration file paths in order of preference
    fn get_config_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // User config directory
        if let Some(user_config) = dirs::config_dir() {
            paths.push(user_config.join("mosaicterm").join("config.toml"));
            paths.push(user_config.join("mosaicterm.toml"));
        }

        // Current directory
        paths.push(PathBuf::from("mosaicterm.toml"));
        paths.push(PathBuf::from(".mosaicterm.toml"));

        paths
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.ui.validate()?;
        self.terminal.validate()?;

        // Validate key bindings format
        for (name, binding) in [
            ("copy", &self.key_bindings.copy),
            ("paste", &self.key_bindings.paste),
            ("new_tab", &self.key_bindings.new_tab),
            ("close_tab", &self.key_bindings.close_tab),
            ("clear", &self.key_bindings.clear),
            ("search", &self.key_bindings.search),
        ] {
            if binding.is_empty() {
                return Err(ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Empty key binding for {}", name),
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.ui.font_family, "Monaco");
        assert_eq!(config.ui.font_size, 12);
        assert_eq!(config.terminal.shell_path, PathBuf::from("/bin/zsh"));
        assert!(config.terminal.inherit_env);
    }

    #[test]
    fn test_color_from_hex() {
        let color = Color::from_hex("#FF0000").unwrap();
        assert_eq!(color.r, 1.0);
        assert_eq!(color.g, 0.0);
        assert_eq!(color.b, 0.0);
        assert_eq!(color.a, 1.0);

        let color_with_alpha = Color::from_hex("#FF000080").unwrap();
        assert_eq!(color_with_alpha.a, 128.0 / 255.0);
    }

    #[test]
    fn test_color_to_hex() {
        let color = Color::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(color.to_hex(), "#FF0000");

        let color_with_alpha = Color::new(1.0, 0.0, 0.0, 0.5);
        assert_eq!(color_with_alpha.to_hex(), "#FF000080");
    }

    #[test]
    fn test_ui_config_validation() {
        let mut config = UiConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid font size
        config.font_size = 5;
        assert!(config.validate().is_err());

        // Reset and test invalid scrollback
        config.font_size = 12;
        config.scrollback_lines = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_terminal_config_validation() {
        let mut config = TerminalConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid shell path
        config.shell_path = PathBuf::from("/nonexistent/shell");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_theme_colors() {
        let theme = Theme::default();

        assert_eq!(theme.background.r, 0.1);
        assert_eq!(theme.foreground.r, 0.9);
        assert_eq!(theme.accent.b, 0.9);
    }
}
