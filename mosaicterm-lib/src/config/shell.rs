//! Shell Configuration Detection
//!
//! Detects and configures shell environments for MosaicTerm,
//! including shell type detection, configuration file parsing, and environment setup.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::env;
use std::fs;
use regex::Regex;
use crate::error::{Error, Result};

/// Shell configuration manager
#[derive(Debug, Clone)]
pub struct ShellManager {
    /// Available shell configurations
    shells: Vec<ShellConfig>,
    /// Current detected shell
    current_shell: Option<ShellType>,
    /// Environment variables to set
    environment: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Shell type
    pub shell_type: ShellType,
    /// Shell name
    pub name: String,
    /// Default executable paths
    pub executable_paths: Vec<PathBuf>,
    /// Default arguments
    pub default_args: Vec<String>,
    /// Configuration file patterns
    pub config_files: Vec<String>,
    /// Environment variables to set
    pub environment_variables: std::collections::HashMap<String, String>,
    /// Prompt detection patterns
    pub prompt_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShellType {
    /// Bourne Again Shell
    Bash,
    /// Z Shell
    Zsh,
    /// Fish Shell
    Fish,
    /// Korn Shell
    Ksh,
    /// C Shell
    Csh,
    /// Tcsh
    Tcsh,
    /// Dash
    Dash,
    /// PowerShell
    PowerShell,
    /// Command Prompt
    Cmd,
    /// Other/Unknown shell
    Other,
}

impl ShellManager {
    /// Create a new shell manager
    pub fn new() -> Self {
        let mut manager = Self {
            shells: Vec::new(),
            current_shell: None,
            environment: std::collections::HashMap::new(),
        };

        manager.initialize_shell_configs();
        manager
    }

    /// Initialize built-in shell configurations
    fn initialize_shell_configs(&mut self) {
        // Bash configuration
        self.shells.push(ShellConfig {
            shell_type: ShellType::Bash,
            name: "Bash".to_string(),
            executable_paths: vec![
                "/bin/bash".into(),
                "/usr/bin/bash".into(),
                "/usr/local/bin/bash".into(),
                "bash".into(),
            ],
            default_args: vec!["--login".to_string(), "-i".to_string()],
            config_files: vec![
                "~/.bashrc".to_string(),
                "~/.bash_profile".to_string(),
                "~/.profile".to_string(),
                "/etc/bash.bashrc".to_string(),
            ],
            environment_variables: std::collections::HashMap::from([
                ("SHELL".to_string(), "/bin/bash".to_string()),
                ("BASH_ENV".to_string(), "~/.bashrc".to_string()),
            ]),
            prompt_patterns: vec![
                r"^\$ $".to_string(),
                r"^bash-\d+\.\d+\$ $".to_string(),
                r"^\[.*\]\$ $".to_string(),
            ],
        });

        // Zsh configuration
        self.shells.push(ShellConfig {
            shell_type: ShellType::Zsh,
            name: "Zsh".to_string(),
            executable_paths: vec![
                "/bin/zsh".into(),
                "/usr/bin/zsh".into(),
                "/usr/local/bin/zsh".into(),
                "zsh".into(),
            ],
            default_args: vec!["--login".to_string()],
            config_files: vec![
                "~/.zshrc".to_string(),
                "~/.zshenv".to_string(),
                "~/.zprofile".to_string(),
                "/etc/zsh/zshrc".to_string(),
            ],
            environment_variables: std::collections::HashMap::from([
                ("SHELL".to_string(), "/bin/zsh".to_string()),
                ("ZDOTDIR".to_string(), "~/.zsh".to_string()),
            ]),
            prompt_patterns: vec![
                r"^% $".to_string(),
                r"^zsh-\d+\.\d+% $".to_string(),
                r"^\[.*\]% $".to_string(),
            ],
        });

        // Fish configuration
        self.shells.push(ShellConfig {
            shell_type: ShellType::Fish,
            name: "Fish".to_string(),
            executable_paths: vec![
                "/bin/fish".into(),
                "/usr/bin/fish".into(),
                "/usr/local/bin/fish".into(),
                "fish".into(),
            ],
            default_args: vec![],
            config_files: vec![
                "~/.config/fish/config.fish".to_string(),
                "/etc/fish/config.fish".to_string(),
            ],
            environment_variables: std::collections::HashMap::from([
                ("SHELL".to_string(), "/bin/fish".to_string()),
            ]),
            prompt_patterns: vec![
                r"^> $".to_string(),
                r"^\[.*\]> $".to_string(),
            ],
        });

        // PowerShell configuration
        self.shells.push(ShellConfig {
            shell_type: ShellType::PowerShell,
            name: "PowerShell".to_string(),
            executable_paths: vec![
                "/usr/bin/pwsh".into(),
                "/usr/local/bin/pwsh".into(),
                "pwsh".into(),
                "powershell".into(),
            ],
            default_args: vec!["-NoLogo".to_string()],
            config_files: vec![
                "~/.config/powershell/Microsoft.PowerShell_profile.ps1".to_string(),
            ],
            environment_variables: std::collections::HashMap::from([
                ("SHELL".to_string(), "/usr/bin/pwsh".to_string()),
            ]),
            prompt_patterns: vec![
                r"^PS .*> $".to_string(),
            ],
        });
    }

    /// Detect the current shell from environment
    pub fn detect_current_shell(&mut self) -> Result<ShellType> {
        // Try to detect from SHELL environment variable
        if let Ok(shell_path) = env::var("SHELL") {
            if let Some(shell_type) = self.detect_shell_from_path(&shell_path) {
                self.current_shell = Some(shell_type);
                return Ok(shell_type);
            }
        }

        // Try to detect from parent process
        if let Some(shell_type) = self.detect_from_parent_process() {
            self.current_shell = Some(shell_type);
            return Ok(shell_type);
        }

        // Fallback to testing common shells
        for config in &self.shells {
            for path in &config.executable_paths {
                if path.exists() {
                    self.current_shell = Some(config.shell_type);
                    return Ok(config.shell_type);
                }
            }
        }

        // Default to Bash if nothing else works
        self.current_shell = Some(ShellType::Bash);
        Ok(ShellType::Bash)
    }

    /// Detect shell type from executable path
    fn detect_shell_from_path(&self, path: &str) -> Option<ShellType> {
        let path_lower = path.to_lowercase();

        if path_lower.contains("bash") {
            Some(ShellType::Bash)
        } else if path_lower.contains("zsh") {
            Some(ShellType::Zsh)
        } else if path_lower.contains("fish") {
            Some(ShellType::Fish)
        } else if path_lower.contains("ksh") {
            Some(ShellType::Ksh)
        } else if path_lower.contains("csh") {
            Some(ShellType::Csh)
        } else if path_lower.contains("tcsh") {
            Some(ShellType::Tcsh)
        } else if path_lower.contains("dash") {
            Some(ShellType::Dash)
        } else if path_lower.contains("pwsh") || path_lower.contains("powershell") {
            Some(ShellType::PowerShell)
        } else if path_lower.contains("cmd") {
            Some(ShellType::Cmd)
        } else {
            None
        }
    }

    /// Detect shell from parent process (Unix only)
    #[cfg(unix)]
    fn detect_from_parent_process(&self) -> Option<ShellType> {
        use std::os::unix::process::CommandExt;

        // Try to get parent process information
        if let Ok(output) = Command::new("ps")
            .arg("-p")
            .arg(std::os::unix::process::parent_id().to_string())
            .arg("-o")
            .arg("comm=")
            .output()
        {
            if let Ok(parent_comm) = String::from_utf8(output.stdout) {
                let parent_comm = parent_comm.trim();
                return self.detect_shell_from_path(parent_comm);
            }
        }

        None
    }

    /// Detect shell from parent process (Windows/Other)
    #[cfg(not(unix))]
    fn detect_from_parent_process(&self) -> Option<ShellType> {
        // Windows implementation would use different APIs
        None
    }

    /// Get configuration for a specific shell type
    pub fn get_shell_config(&self, shell_type: ShellType) -> Option<&ShellConfig> {
        self.shells.iter().find(|config| config.shell_type == shell_type)
    }

    /// Get current shell configuration
    pub fn current_shell_config(&self) -> Option<&ShellConfig> {
        self.current_shell.and_then(|shell_type| self.get_shell_config(shell_type))
    }

    /// Get all available shells
    pub fn available_shells(&self) -> Vec<ShellType> {
        self.shells.iter()
            .filter(|config| self.is_shell_available(config))
            .map(|config| config.shell_type)
            .collect()
    }

    /// Check if a shell is available on the system
    fn is_shell_available(&self, config: &ShellConfig) -> bool {
        config.executable_paths.iter().any(|path| {
            if let Some(path_str) = path.to_str() {
                // Check if it's a full path
                if path.is_absolute() {
                    path.exists()
                } else {
                    // Check if it's in PATH
                    env::var("PATH").map_or(false, |path_var| {
                        env::split_paths(&path_var).any(|dir| dir.join(path_str).exists())
                    })
                }
            } else {
                false
            }
        })
    }

    /// Get the best available shell path
    pub fn get_shell_path(&self, shell_type: ShellType) -> Option<&Path> {
        if let Some(config) = self.get_shell_config(shell_type) {
            config.executable_paths.iter()
                .find(|path| {
                    if path.is_absolute() {
                        path.exists()
                    } else {
                        // Check PATH
                        env::var("PATH").map_or(false, |path_var| {
                            env::split_paths(&path_var).any(|dir| dir.join(path).exists())
                        })
                    }
                })
                .map(|path| path.as_path())
        } else {
            None
        }
    }

    /// Get shell arguments
    pub fn get_shell_args(&self, shell_type: ShellType) -> &[String] {
        self.get_shell_config(shell_type)
            .map(|config| &config.default_args[..])
            .unwrap_or(&[])
    }

    /// Load shell configuration files
    pub fn load_shell_config(&self, shell_type: ShellType) -> Result<ShellEnvironment> {
        let config = self.get_shell_config(shell_type)
            .ok_or_else(|| Error::Other(format!("Shell configuration not found for {:?}", shell_type)))?;

        let mut environment = std::collections::HashMap::new();

        // Load configuration from files
        for config_file in &config.config_files {
            if let Some(content) = self.load_config_file(config_file)? {
                // Parse configuration based on shell type
                let parsed_env = self.parse_shell_config(shell_type, &content)?;
                environment.extend(parsed_env);
            }
        }

        // Add default environment variables
        environment.extend(config.environment_variables.clone());

        Ok(ShellEnvironment {
            shell_type,
            environment,
            config_files: config.config_files.clone(),
        })
    }

    /// Load a configuration file
    fn load_config_file(&self, config_file: &str) -> Result<Option<String>> {
        let expanded_path = self.expand_path(config_file)?;

        if expanded_path.exists() {
            let content = fs::read_to_string(&expanded_path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    /// Expand path with tilde expansion
    fn expand_path(&self, path: &str) -> Result<PathBuf> {
        if let Some(home) = dirs::home_dir() {
            Ok(PathBuf::from(path.replace("~", &home.to_string_lossy())))
        } else {
            Ok(PathBuf::from(path))
        }
    }

    /// Parse shell configuration content
    fn parse_shell_config(&self, shell_type: ShellType, content: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut environment = std::collections::HashMap::new();

        match shell_type {
            ShellType::Bash | ShellType::Zsh => {
                self.parse_bash_zsh_config(content, &mut environment)?;
            }
            ShellType::Fish => {
                self.parse_fish_config(content, &mut environment)?;
            }
            _ => {
                // Generic parsing for other shells
                self.parse_generic_config(content, &mut environment)?;
            }
        }

        Ok(environment)
    }

    /// Parse Bash/Zsh configuration
    fn parse_bash_zsh_config(&self, content: &str, environment: &mut std::collections::HashMap<String, String>) -> Result<()> {
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Parse export statements
            if line.starts_with("export ") {
                if let Some(eq_pos) = line.find('=') {
                    let var_part = &line[7..eq_pos];
                    let value_part = &line[eq_pos + 1..];

                    // Remove quotes if present
                    let value = value_part.trim_matches('"').trim_matches('\'');

                    if let Some(var_name) = var_part.split_whitespace().next() {
                        environment.insert(var_name.to_string(), value.to_string());
                    }
                }
            }

            // Parse simple variable assignments
            if let Some(eq_pos) = line.find('=') {
                let var_name = line[..eq_pos].trim();
                let value_part = &line[eq_pos + 1..];

                // Skip if it contains complex expressions
                if !value_part.contains('$') && !value_part.contains('`') {
                    let value = value_part.trim_matches('"').trim_matches('\'');
                    environment.insert(var_name.to_string(), value.to_string());
                }
            }
        }

        Ok(())
    }

    /// Parse Fish configuration
    fn parse_fish_config(&self, content: &str, environment: &mut std::collections::HashMap<String, String>) -> Result<()> {
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Parse set statements
            if line.starts_with("set ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == "-x" { // export variable
                    let var_name = parts[2];
                    let value = parts.get(3).unwrap_or(&"").to_string();
                    environment.insert(var_name.to_string(), value);
                }
            }
        }

        Ok(())
    }

    /// Parse generic configuration
    fn parse_generic_config(&self, content: &str, environment: &mut std::collections::HashMap<String, String>) -> Result<()> {
        let lines: Vec<&str> = content.lines().collect();

        for line in lines {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Parse simple variable assignments
            if let Some(eq_pos) = line.find('=') {
                let var_name = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
                environment.insert(var_name.to_string(), value.to_string());
            }
        }

        Ok(())
    }

    /// Get current shell type
    pub fn current_shell(&self) -> Option<ShellType> {
        self.current_shell
    }

    /// Set current shell
    pub fn set_current_shell(&mut self, shell_type: ShellType) {
        self.current_shell = Some(shell_type);
    }

    /// Get environment variables
    pub fn environment(&self) -> &std::collections::HashMap<String, String> {
        &self.environment
    }

    /// Set environment variable
    pub fn set_environment_variable(&mut self, key: String, value: String) {
        self.environment.insert(key, value);
    }
}

impl Default for ShellManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shell environment information
#[derive(Debug, Clone)]
pub struct ShellEnvironment {
    /// Shell type
    pub shell_type: ShellType,
    /// Environment variables
    pub environment: std::collections::HashMap<String, String>,
    /// Configuration files loaded
    pub config_files: Vec<String>,
}

/// Shell detection utilities
pub mod utils {
    use super::*;

    /// Get shell name from type
    pub fn shell_name(shell_type: ShellType) -> &'static str {
        match shell_type {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Ksh => "ksh",
            ShellType::Csh => "csh",
            ShellType::Tcsh => "tcsh",
            ShellType::Dash => "dash",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Other => "unknown",
        }
    }

    /// Get default shell for the current platform
    pub fn default_shell_for_platform() -> ShellType {
        #[cfg(unix)]
        {
            // On Unix systems, try to detect from SHELL env var
            if let Ok(shell_path) = env::var("SHELL") {
                let shell_path_lower = shell_path.to_lowercase();
                if shell_path_lower.contains("zsh") {
                    return ShellType::Zsh;
                } else if shell_path_lower.contains("fish") {
                    return ShellType::Fish;
                }
            }
            // Default to Bash on Unix
            ShellType::Bash
        }

        #[cfg(windows)]
        {
            // Default to PowerShell on Windows (if available), otherwise Cmd
            ShellType::PowerShell
        }

        #[cfg(not(any(unix, windows)))]
        {
            ShellType::Bash
        }
    }

    /// Check if shell supports certain features
    pub fn shell_supports_feature(shell_type: ShellType, feature: ShellFeature) -> bool {
        match (shell_type, feature) {
            (ShellType::Bash | ShellType::Zsh, ShellFeature::Scripting) => true,
            (ShellType::Fish, ShellFeature::Scripting) => true,
            (ShellType::PowerShell, ShellFeature::Scripting) => true,
            (ShellType::Bash | ShellType::Zsh | ShellType::Fish, ShellFeature::Colors) => true,
            (ShellType::PowerShell, ShellFeature::Colors) => true,
            (_, ShellFeature::Interactive) => true,
            _ => false,
        }
    }

    /// Validate shell configuration
    pub fn validate_shell_config(config: &ShellConfig) -> Result<()> {
        if config.name.is_empty() {
            return Err(Error::Other("Shell name cannot be empty".to_string()));
        }

        if config.executable_paths.is_empty() {
            return Err(Error::Other("Shell must have at least one executable path".to_string()));
        }

        for path in &config.executable_paths {
            if let Some(path_str) = path.to_str() {
                if path_str.is_empty() {
                    return Err(Error::Other("Shell executable path cannot be empty".to_string()));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellFeature {
    /// Shell supports scripting
    Scripting,
    /// Shell supports colors/ANSI codes
    Colors,
    /// Shell supports interactive mode
    Interactive,
    /// Shell supports job control
    JobControl,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_manager_creation() {
        let manager = ShellManager::new();
        assert!(!manager.shells.is_empty());
        assert!(manager.current_shell.is_none());
    }

    #[test]
    fn test_shell_detection_from_path() {
        let manager = ShellManager::new();

        assert_eq!(manager.detect_shell_from_path("/bin/bash"), Some(ShellType::Bash));
        assert_eq!(manager.detect_shell_from_path("/usr/bin/zsh"), Some(ShellType::Zsh));
        assert_eq!(manager.detect_shell_from_path("/bin/fish"), Some(ShellType::Fish));
        assert_eq!(manager.detect_shell_from_path("/usr/bin/pwsh"), Some(ShellType::PowerShell));
        assert_eq!(manager.detect_shell_from_path("/unknown/shell"), None);
    }

    #[test]
    fn test_get_shell_config() {
        let manager = ShellManager::new();

        let bash_config = manager.get_shell_config(ShellType::Bash);
        assert!(bash_config.is_some());
        assert_eq!(bash_config.unwrap().name, "Bash");

        let unknown_config = manager.get_shell_config(ShellType::Other);
        assert!(unknown_config.is_none());
    }

    #[test]
    fn test_parse_bash_config() {
        let manager = ShellManager::new();
        let mut environment = std::collections::HashMap::new();

        let config_content = r#"
# Comment
export PATH="/usr/local/bin:$PATH"
export EDITOR="vim"
HOME="/home/user"
"#;

        manager.parse_bash_zsh_config(config_content, &mut environment).unwrap();

        assert_eq!(environment.get("PATH"), Some(&"/usr/local/bin:$PATH".to_string()));
        assert_eq!(environment.get("EDITOR"), Some(&"vim".to_string()));
        assert_eq!(environment.get("HOME"), Some(&"/home/user".to_string()));
    }

    #[test]
    fn test_shell_name_conversion() {
        assert_eq!(utils::shell_name(ShellType::Bash), "bash");
        assert_eq!(utils::shell_name(ShellType::Zsh), "zsh");
        assert_eq!(utils::shell_name(ShellType::Fish), "fish");
        assert_eq!(utils::shell_name(ShellType::PowerShell), "powershell");
        assert_eq!(utils::shell_name(ShellType::Other), "unknown");
    }

    #[test]
    fn test_shell_feature_support() {
        assert!(utils::shell_supports_feature(ShellType::Bash, ShellFeature::Scripting));
        assert!(utils::shell_supports_feature(ShellType::Bash, ShellFeature::Colors));
        assert!(utils::shell_supports_feature(ShellType::Bash, ShellFeature::Interactive));
        assert!(!utils::shell_supports_feature(ShellType::Other, ShellFeature::Scripting));
    }

    #[test]
    fn test_validate_shell_config() {
        let valid_config = ShellConfig {
            shell_type: ShellType::Bash,
            name: "Bash".to_string(),
            executable_paths: vec!["/bin/bash".into()],
            default_args: vec![],
            config_files: vec![],
            environment_variables: std::collections::HashMap::new(),
            prompt_patterns: vec![],
        };

        assert!(utils::validate_shell_config(&valid_config).is_ok());

        let invalid_config = ShellConfig {
            shell_type: ShellType::Bash,
            name: "".to_string(),
            executable_paths: vec![],
            default_args: vec![],
            config_files: vec![],
            environment_variables: std::collections::HashMap::new(),
            prompt_patterns: vec![],
        };

        assert!(utils::validate_shell_config(&invalid_config).is_err());
    }

    #[test]
    fn test_default_shell_for_platform() {
        let shell_type = utils::default_shell_for_platform();
        // Should return a valid shell type
        assert!(matches!(shell_type, ShellType::Bash | ShellType::Zsh | ShellType::Fish | ShellType::PowerShell));
    }

    #[test]
    fn test_shell_type_equality() {
        assert_eq!(ShellType::Bash, ShellType::Bash);
        assert_ne!(ShellType::Bash, ShellType::Zsh);
        assert_eq!(ShellType::Other, ShellType::Other);
    }
}
