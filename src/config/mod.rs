//! Configuration management for MosaicTerm
//!
//! This module provides comprehensive configuration management for MosaicTerm,
//! including loading/saving configurations, theme management, shell detection,
//! and runtime configuration handling.

#[allow(unexpected_cfgs)]
pub mod loader;
pub mod prompt;
pub mod shell;
pub mod theme;
pub mod watcher;

use crate::config::shell::ShellManager;
use crate::config::theme::ThemeManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure for MosaicTerm
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,

    /// Terminal configuration
    #[serde(default)]
    pub terminal: TerminalConfig,

    /// PTY configuration
    #[serde(default)]
    pub pty: PtyConfig,

    /// Key binding configuration
    #[serde(default)]
    pub key_bindings: KeyBindings,

    /// Interactive TUI app configuration
    #[serde(default)]
    pub tui_apps: TuiAppConfig,
}

/// UI-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Font family for terminal text
    pub font_family: String,

    /// Font size in points
    pub font_size: u32,

    /// Maximum number of lines to keep in history
    pub scrollback_lines: usize,

    /// UI theme name (for preset selection)
    pub theme_name: String,

    /// Custom theme colors (overrides theme_name if specified)
    #[serde(default)]
    pub theme: crate::models::config::Theme,

    /// Enable smooth scrolling
    pub smooth_scrolling: bool,

    /// Animation duration in milliseconds
    pub animation_duration_ms: u32,

    /// Show line numbers
    pub show_line_numbers: bool,

    /// Word wrap mode
    pub word_wrap: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            font_family: "JetBrains Mono".to_string(),
            font_size: 12,
            scrollback_lines: 100000, // Increased for unlimited output
            theme_name: "default-dark".to_string(),
            theme: crate::models::config::Theme::default(),
            smooth_scrolling: true,
            animation_duration_ms: 200,
            show_line_numbers: false,
            word_wrap: true,
        }
    }
}

/// Terminal-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Shell type
    pub shell_type: crate::models::ShellType,

    /// Shell executable path
    pub shell_path: PathBuf,

    /// Shell arguments
    pub shell_args: Vec<String>,

    /// Working directory for new terminals
    pub working_directory: Option<PathBuf>,

    /// Terminal dimensions (cols, rows)
    pub dimensions: (u16, u16),

    /// Enable mouse support
    pub mouse_support: bool,

    /// Scrollback buffer size
    pub scrollback_buffer: usize,

    /// Bell style
    pub bell_style: BellStyle,

    /// Custom prompt format
    /// Supports variables: $USER, $HOSTNAME, $PWD, $HOME, $SHELL
    /// Example: "$USER@$HOSTNAME:$PWD$ "
    pub prompt_format: String,

    /// Command execution timeout settings
    #[serde(default)]
    pub timeout: TimeoutConfig,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell_type: crate::models::ShellType::Bash,
            shell_path: PathBuf::from("/bin/bash"),
            shell_args: vec!["--login".to_string(), "-i".to_string()],
            working_directory: None,
            dimensions: (120, 30),
            mouse_support: true,
            scrollback_buffer: 1000000, // Increased to 1M for unlimited output
            bell_style: BellStyle::Sound,
            prompt_format: "$USER@$HOSTNAME:$PWD$ ".to_string(),
            timeout: TimeoutConfig::default(),
        }
    }
}

/// Command timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeoutConfig {
    /// Timeout in seconds for regular commands (0 = disabled)
    /// Default: 30 seconds
    pub regular_command_timeout_secs: u64,

    /// Timeout in seconds for interactive commands (0 = disabled)
    /// Default: 300 seconds (5 minutes)
    pub interactive_command_timeout_secs: u64,

    /// Whether to automatically kill commands that exceed timeout
    /// Default: false (just mark as completed)
    pub kill_on_timeout: bool,

    /// Grace period in seconds after timeout before force-killing
    /// Only used if kill_on_timeout is true
    /// Default: 5 seconds
    pub kill_grace_period_secs: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            regular_command_timeout_secs: 0,     // Disabled by default
            interactive_command_timeout_secs: 0, // Disabled by default
            kill_on_timeout: false,
            kill_grace_period_secs: 5,
        }
    }
}

/// PTY-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PtyConfig {
    /// Environment variables to set
    pub environment: std::collections::HashMap<String, String>,

    /// Whether to inherit parent environment
    #[serde(default = "default_true")]
    pub inherit_env: bool,

    /// PTY buffer size
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,

    /// Enable raw mode
    #[serde(default = "default_true")]
    pub raw_mode: bool,

    /// Timeout for PTY operations in milliseconds
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_true() -> bool {
    true
}

fn default_buffer_size() -> usize {
    256 * 1024
}

fn default_timeout_ms() -> u64 {
    10
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            environment: std::collections::HashMap::new(),
            inherit_env: true,
            buffer_size: 256 * 1024, // 256KB - balanced for most use cases
            raw_mode: true,
            timeout_ms: 10, // Reduced for faster response
        }
    }
}

/// Key binding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    /// Key bindings for actions
    pub bindings: std::collections::HashMap<String, KeyBinding>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut bindings = std::collections::HashMap::new();

        // Default key bindings
        bindings.insert("interrupt".to_string(), KeyBinding::new("Ctrl+C"));
        bindings.insert("copy".to_string(), KeyBinding::new("Ctrl+Shift+C"));
        bindings.insert("paste".to_string(), KeyBinding::new("Ctrl+V"));
        bindings.insert("new_tab".to_string(), KeyBinding::new("Ctrl+T"));
        bindings.insert("close_tab".to_string(), KeyBinding::new("Ctrl+W"));
        bindings.insert("next_tab".to_string(), KeyBinding::new("Ctrl+Tab"));
        bindings.insert("prev_tab".to_string(), KeyBinding::new("Ctrl+Shift+Tab"));
        bindings.insert("clear".to_string(), KeyBinding::new("Ctrl+L"));
        bindings.insert("quit".to_string(), KeyBinding::new("Ctrl+Q"));

        Self { bindings }
    }
}

/// Individual key binding
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeyBinding {
    /// Key combination string (e.g., "Ctrl+C", "Alt+F4")
    pub key: String,
    /// Whether this binding is enabled
    pub enabled: bool,
}

impl KeyBinding {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            enabled: true,
        }
    }
}

/// Bell style for terminal bell
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum BellStyle {
    /// No bell
    None,
    /// Sound bell
    #[default]
    Sound,
    /// Visual bell (screen flash)
    Visual,
}

/// Configuration for interactive TUI applications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiAppConfig {
    /// List of commands that should open in fullscreen mode
    pub fullscreen_commands: Vec<String>,
}

impl Default for TuiAppConfig {
    fn default() -> Self {
        Self {
            fullscreen_commands: vec![
                // Text editors
                "vim".to_string(),
                "nvim".to_string(),
                "vi".to_string(),
                "nano".to_string(),
                "emacs".to_string(),
                "helix".to_string(),
                "micro".to_string(),
                // System monitors
                "top".to_string(),
                "htop".to_string(),
                "btop".to_string(),
                "gotop".to_string(),
                "ytop".to_string(),
                "atop".to_string(),
                // Interactive tools
                "less".to_string(),
                "more".to_string(),
                "man".to_string(),
                "tmux".to_string(),
                "screen".to_string(),
                // File managers
                "ranger".to_string(),
                "nnn".to_string(),
                "mc".to_string(),
                "vifm".to_string(),
                // Other TUI apps
                "ncdu".to_string(),
                "cmus".to_string(),
                "weechat".to_string(),
                "irssi".to_string(),
                "mutt".to_string(),
                "ncmpcpp".to_string(),
            ],
        }
    }
}

/// Runtime configuration manager
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Current configuration
    config: Config,
    /// Theme manager
    theme_manager: ThemeManager,
    /// Shell manager
    shell_manager: ShellManager,
    /// Configuration file path
    config_path: Option<PathBuf>,
}

impl RuntimeConfig {
    /// Create a new runtime configuration manager
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut theme_manager = ThemeManager::new();
        let mut shell_manager = ShellManager::new();

        // Detect current shell
        shell_manager.detect_current_shell()?;

        // Load configuration
        let config = loader::ConfigLoader::load()?;

        // Apply current theme
        theme_manager.set_theme(&config.ui.theme_name).unwrap_or(());

        let runtime_config = Self {
            config,
            theme_manager,
            shell_manager,
            config_path: None,
        };

        // Validate the runtime configuration
        runtime_config.validate()?;

        Ok(runtime_config)
    }

    /// Create a minimal runtime configuration (used as fallback when initialization fails)
    pub fn new_minimal() -> Self {
        let theme_manager = ThemeManager::new();
        let shell_manager = ShellManager::new();
        let config = Config::default();

        Self {
            config,
            theme_manager,
            shell_manager,
            config_path: None,
        }
    }

    /// Load configuration from file
    pub fn load_from_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut theme_manager = ThemeManager::new();
        let shell_manager = ShellManager::new();

        // Load configuration
        let config = loader::ConfigLoader::load_with_options(loader::LoadOptions {
            create_default: false,
            merge_defaults: false,
            validate: true,
        })?;

        // Apply current theme
        theme_manager.set_theme(&config.ui.theme_name).unwrap_or(());

        Ok(Self {
            config,
            theme_manager,
            shell_manager,
            config_path: Some(path.to_path_buf()),
        })
    }

    /// Save current configuration
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(path) = &self.config_path {
            loader::ConfigLoader::new().save_to_path(&self.config, path)?;
        } else {
            loader::ConfigLoader::new().save(&self.config)?;
        }
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Get theme manager
    pub fn theme_manager(&self) -> &ThemeManager {
        &self.theme_manager
    }

    /// Get mutable theme manager
    pub fn theme_manager_mut(&mut self) -> &mut ThemeManager {
        &mut self.theme_manager
    }

    /// Get shell manager
    pub fn shell_manager(&self) -> &ShellManager {
        &self.shell_manager
    }

    /// Get mutable shell manager
    pub fn shell_manager_mut(&mut self) -> &mut ShellManager {
        &mut self.shell_manager
    }

    /// Set configuration and apply changes
    pub fn set_config(&mut self, config: Config) -> Result<(), Box<dyn std::error::Error>> {
        self.config = config;

        // Apply theme changes
        self.theme_manager.set_theme(&self.config.ui.theme_name)?;

        Ok(())
    }

    /// Reload configuration from file
    pub fn reload(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_path) = &self.config_path {
            let new_config = loader::ConfigLoader::load_with_options(loader::LoadOptions {
                create_default: false,
                merge_defaults: false,
                validate: true,
            })?;
            self.set_config(new_config)?;
        }
        Ok(())
    }

    /// Get current theme
    pub fn current_theme(&self) -> Result<&theme::Theme, Box<dyn std::error::Error>> {
        self.theme_manager.current_theme().map_err(|e| e.into())
    }

    /// Get current shell configuration
    pub fn current_shell_config(&self) -> Option<&shell::ShellConfig> {
        self.shell_manager.current_shell_config()
    }

    /// Validate current configuration
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Validate theme exists
        self.theme_manager.current_theme()?;

        // Validate shell configuration
        if let Some(shell_config) = self.current_shell_config() {
            shell::utils::validate_shell_config(shell_config)?;
        }

        // Validate key bindings
        for (action, binding) in &self.config.key_bindings.bindings {
            if binding.key.trim().is_empty() {
                return Err(format!("Empty key binding for action: {}", action).into());
            }
        }

        Ok(())
    }
}

// Note: RuntimeConfig cannot have a Default implementation because
// initialization may fail. Use RuntimeConfig::new() instead.

/// Configuration utilities
pub mod utils {
    use super::*;

    /// Get configuration file format from path
    pub fn get_config_format(path: &Path) -> Option<loader::ConfigFormat> {
        match path.extension()?.to_str()? {
            "toml" => Some(loader::ConfigFormat::Toml),
            "json" => Some(loader::ConfigFormat::Json),
            #[cfg(feature = "yaml")]
            "yaml" | "yml" => Some(loader::ConfigFormat::Yaml),
            _ => None,
        }
    }

    /// Create a default configuration file content
    pub fn create_default_config_content(
        format: loader::ConfigFormat,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let config = Config::default();

        match format {
            loader::ConfigFormat::Toml => toml::to_string_pretty(&config)
                .map_err(|e| format!("Failed to serialize TOML: {}", e).into()),
            loader::ConfigFormat::Json => serde_json::to_string_pretty(&config)
                .map_err(|e| format!("Failed to serialize JSON: {}", e).into()),
            #[cfg(feature = "yaml")]
            loader::ConfigFormat::Yaml => serde_yaml::to_string(&config)
                .map_err(|e| format!("Failed to serialize YAML: {}", e).into()),
        }
    }

    /// Merge two configurations
    pub fn merge_configs(base: Config, overlay: Config) -> Config {
        Config {
            ui: merge_ui_configs(base.ui, overlay.ui),
            terminal: merge_terminal_configs(base.terminal, overlay.terminal),
            pty: merge_pty_configs(base.pty, overlay.pty),
            key_bindings: merge_key_bindings(base.key_bindings, overlay.key_bindings),
            tui_apps: merge_tui_apps_configs(base.tui_apps, overlay.tui_apps),
        }
    }

    fn merge_ui_configs(base: UiConfig, overlay: UiConfig) -> UiConfig {
        UiConfig {
            font_family: if overlay.font_family.is_empty() {
                base.font_family
            } else {
                overlay.font_family
            },
            font_size: if overlay.font_size == 0 {
                base.font_size
            } else {
                overlay.font_size
            },
            scrollback_lines: if overlay.scrollback_lines == 0 {
                base.scrollback_lines
            } else {
                overlay.scrollback_lines
            },
            theme_name: if overlay.theme_name.is_empty() {
                base.theme_name
            } else {
                overlay.theme_name
            },
            // Use overlay theme, falling back to base theme
            theme: overlay.theme,
            smooth_scrolling: overlay.smooth_scrolling,
            animation_duration_ms: if overlay.animation_duration_ms == 0 {
                base.animation_duration_ms
            } else {
                overlay.animation_duration_ms
            },
            show_line_numbers: overlay.show_line_numbers,
            word_wrap: overlay.word_wrap,
        }
    }

    fn merge_terminal_configs(base: TerminalConfig, overlay: TerminalConfig) -> TerminalConfig {
        TerminalConfig {
            shell_type: overlay.shell_type,
            shell_path: if overlay.shell_path.as_os_str().is_empty() {
                base.shell_path
            } else {
                overlay.shell_path
            },
            shell_args: if overlay.shell_args.is_empty() {
                base.shell_args
            } else {
                overlay.shell_args
            },
            working_directory: overlay.working_directory.or(base.working_directory),
            dimensions: if overlay.dimensions == (0, 0) {
                base.dimensions
            } else {
                overlay.dimensions
            },
            mouse_support: overlay.mouse_support,
            scrollback_buffer: if overlay.scrollback_buffer == 0 {
                base.scrollback_buffer
            } else {
                overlay.scrollback_buffer
            },
            bell_style: overlay.bell_style,
            prompt_format: if overlay.prompt_format.is_empty() {
                base.prompt_format
            } else {
                overlay.prompt_format
            },
            timeout: overlay.timeout,
        }
    }

    fn merge_pty_configs(base: PtyConfig, overlay: PtyConfig) -> PtyConfig {
        PtyConfig {
            environment: {
                let mut merged = base.environment;
                merged.extend(overlay.environment);
                merged
            },
            inherit_env: overlay.inherit_env,
            buffer_size: if overlay.buffer_size == 0 {
                base.buffer_size
            } else {
                overlay.buffer_size
            },
            raw_mode: overlay.raw_mode,
            timeout_ms: if overlay.timeout_ms == 0 {
                base.timeout_ms
            } else {
                overlay.timeout_ms
            },
        }
    }

    fn merge_key_bindings(base: KeyBindings, overlay: KeyBindings) -> KeyBindings {
        KeyBindings {
            bindings: {
                let mut merged = base.bindings;
                merged.extend(overlay.bindings);
                merged
            },
        }
    }

    fn merge_tui_apps_configs(base: TuiAppConfig, overlay: TuiAppConfig) -> TuiAppConfig {
        TuiAppConfig {
            // Merge overlay commands with base defaults
            // This allows users to ADD to the default list, not replace it
            fullscreen_commands: {
                let mut merged = base.fullscreen_commands;
                for cmd in overlay.fullscreen_commands {
                    if !merged.contains(&cmd) {
                        merged.push(cmd);
                    }
                }
                merged
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = Config::default();
        assert_eq!(config.ui.font_family, "JetBrains Mono");
        assert_eq!(config.ui.font_size, 12);
        assert_eq!(config.terminal.shell_type, crate::TerminalShellType::Bash);
    }

    #[test]
    fn test_key_binding_creation() {
        let binding = KeyBinding::new("Ctrl+C");
        assert_eq!(binding.key, "Ctrl+C");
        assert!(binding.enabled);
    }

    #[test]
    fn test_key_bindings_default() {
        let bindings = KeyBindings::default();
        assert!(bindings.bindings.contains_key("copy"));
        assert!(bindings.bindings.contains_key("paste"));
        assert!(bindings.bindings.contains_key("quit"));
    }

    #[test]
    fn test_bell_style_variants() {
        assert_eq!(format!("{:?}", BellStyle::None), "None");
        assert_eq!(format!("{:?}", BellStyle::Sound), "Sound");
        assert_eq!(format!("{:?}", BellStyle::Visual), "Visual");
    }

    #[test]
    fn test_get_config_format() {
        assert_eq!(
            utils::get_config_format(Path::new("config.toml")),
            Some(loader::ConfigFormat::Toml)
        );
        assert_eq!(
            utils::get_config_format(Path::new("config.json")),
            Some(loader::ConfigFormat::Json)
        );
        assert_eq!(utils::get_config_format(Path::new("config.txt")), None);
    }

    #[test]
    fn test_merge_configs() {
        let base = Config::default();
        let mut overlay = Config::default();
        overlay.ui.font_size = 14;
        overlay.ui.theme_name = "light".to_string();

        let merged = utils::merge_configs(base, overlay);
        assert_eq!(merged.ui.font_size, 14);
        assert_eq!(merged.ui.theme_name, "light");
    }

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        // Basic validation - should pass with default config
        assert!(config.ui.font_size > 0);
        assert!(!config.terminal.shell_path.as_os_str().is_empty());
    }

    #[test]
    fn test_tui_apps_config_default() {
        let config = Config::default();
        // Verify default TUI apps are present
        assert!(config
            .tui_apps
            .fullscreen_commands
            .contains(&"vim".to_string()));
        assert!(config
            .tui_apps
            .fullscreen_commands
            .contains(&"nvim".to_string()));
        assert!(config
            .tui_apps
            .fullscreen_commands
            .contains(&"htop".to_string()));
        assert!(!config.tui_apps.fullscreen_commands.is_empty());
    }

    #[test]
    fn test_merge_tui_apps_configs() {
        let base = Config::default();
        let base_len = base.tui_apps.fullscreen_commands.len();
        let mut overlay = Config::default();
        // Set custom TUI apps in overlay - these should be ADDED to defaults
        overlay.tui_apps.fullscreen_commands =
            vec!["custom_app".to_string(), "another_app".to_string()];

        let merged = utils::merge_configs(base, overlay);
        // Should have base commands PLUS overlay's custom commands
        assert_eq!(merged.tui_apps.fullscreen_commands.len(), base_len + 2);
        // Should contain the new custom apps
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"custom_app".to_string()));
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"another_app".to_string()));
        // Should ALSO still contain the defaults
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"vim".to_string()));
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"top".to_string()));
    }

    #[test]
    fn test_merge_tui_apps_configs_empty_overlay() {
        let base = Config::default();
        let mut overlay = Config::default();
        // Empty overlay should use base
        overlay.tui_apps.fullscreen_commands = vec![];

        let merged = utils::merge_configs(base, overlay);
        // Should use base's TUI apps since overlay is empty
        assert!(!merged.tui_apps.fullscreen_commands.is_empty());
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"vim".to_string()));
    }

    #[test]
    fn test_merge_tui_apps_configs_no_duplicates() {
        let base = Config::default();
        let mut overlay = Config::default();
        // Try to add "vim" which is already in defaults - should not create duplicate
        overlay.tui_apps.fullscreen_commands = vec!["vim".to_string(), "custom_app".to_string()];

        let merged = utils::merge_configs(base, overlay);
        // Count how many times "vim" appears
        let vim_count = merged
            .tui_apps
            .fullscreen_commands
            .iter()
            .filter(|c| *c == "vim")
            .count();
        assert_eq!(vim_count, 1, "vim should only appear once");
        // Should have custom_app added
        assert!(merged
            .tui_apps
            .fullscreen_commands
            .contains(&"custom_app".to_string()));
    }
}
