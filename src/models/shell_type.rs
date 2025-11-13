//! Shell Type Definitions
//!
//! Canonical definition of shell types supported by MosaicTerm.
//! This consolidates multiple scattered definitions into a single source of truth.

use serde::{Deserialize, Serialize};

/// Type of shell being used
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ShellType {
    /// Bourne Again Shell
    Bash,
    /// Z Shell
    #[default]
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

impl ShellType {
    /// Get a string representation of the shell type
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Ksh => "ksh",
            ShellType::Csh => "csh",
            ShellType::Tcsh => "tcsh",
            ShellType::Dash => "dash",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Other => "other",
        }
    }

    /// Get shell type from string (case-insensitive)
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "ksh" => ShellType::Ksh,
            "csh" => ShellType::Csh,
            "tcsh" => ShellType::Tcsh,
            "dash" => ShellType::Dash,
            "powershell" => ShellType::PowerShell,
            "cmd" => ShellType::Cmd,
            _ => ShellType::Other,
        }
    }

    /// Check if shell supports certain features
    pub fn supports_prompt_detection(&self) -> bool {
        matches!(self, ShellType::Bash | ShellType::Zsh | ShellType::Fish)
    }

    /// Get default prompt patterns for this shell
    pub fn get_default_prompt_patterns(&self) -> Vec<String> {
        match self {
            ShellType::Bash => vec![
                r"^\$ $".to_string(),       // Simple dollar prompt
                r"^\[.*\]\$ $".to_string(), // User@host prompt
                r"^.*\$ $".to_string(),     // Generic dollar prompt
            ],
            ShellType::Zsh => vec![
                r"^% $".to_string(),       // Simple percent prompt
                r"^\[.*\]% $".to_string(), // User@host prompt
                r"^.*% $".to_string(),     // Generic percent prompt
            ],
            ShellType::Fish => vec![
                r"^> $".to_string(),   // Simple arrow prompt
                r"^.*> $".to_string(), // Generic arrow prompt
            ],
            _ => vec![r"^.*[$%>]$ $".to_string()], // Generic fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_as_str() {
        assert_eq!(ShellType::Bash.as_str(), "bash");
        assert_eq!(ShellType::Zsh.as_str(), "zsh");
        assert_eq!(ShellType::Fish.as_str(), "fish");
        assert_eq!(ShellType::Ksh.as_str(), "ksh");
        assert_eq!(ShellType::Csh.as_str(), "csh");
        assert_eq!(ShellType::Tcsh.as_str(), "tcsh");
        assert_eq!(ShellType::Dash.as_str(), "dash");
        assert_eq!(ShellType::PowerShell.as_str(), "powershell");
        assert_eq!(ShellType::Cmd.as_str(), "cmd");
        assert_eq!(ShellType::Other.as_str(), "other");
    }

    #[test]
    fn test_shell_type_from_string() {
        assert_eq!(ShellType::from_string("bash"), ShellType::Bash);
        assert_eq!(ShellType::from_string("BASH"), ShellType::Bash);
        assert_eq!(ShellType::from_string("Bash"), ShellType::Bash);
        assert_eq!(ShellType::from_string("zsh"), ShellType::Zsh);
        assert_eq!(ShellType::from_string("fish"), ShellType::Fish);
        assert_eq!(ShellType::from_string("ksh"), ShellType::Ksh);
        assert_eq!(ShellType::from_string("csh"), ShellType::Csh);
        assert_eq!(ShellType::from_string("tcsh"), ShellType::Tcsh);
        assert_eq!(ShellType::from_string("dash"), ShellType::Dash);
        assert_eq!(ShellType::from_string("powershell"), ShellType::PowerShell);
        assert_eq!(ShellType::from_string("cmd"), ShellType::Cmd);
        assert_eq!(ShellType::from_string("unknown"), ShellType::Other);
        assert_eq!(ShellType::from_string(""), ShellType::Other);
    }

    #[test]
    fn test_shell_type_default() {
        assert_eq!(ShellType::default(), ShellType::Zsh);
    }

    #[test]
    fn test_shell_type_supports_prompt_detection() {
        assert!(ShellType::Bash.supports_prompt_detection());
        assert!(ShellType::Zsh.supports_prompt_detection());
        assert!(ShellType::Fish.supports_prompt_detection());
        assert!(!ShellType::Ksh.supports_prompt_detection());
        assert!(!ShellType::Csh.supports_prompt_detection());
        assert!(!ShellType::Tcsh.supports_prompt_detection());
        assert!(!ShellType::Dash.supports_prompt_detection());
        assert!(!ShellType::PowerShell.supports_prompt_detection());
        assert!(!ShellType::Cmd.supports_prompt_detection());
        assert!(!ShellType::Other.supports_prompt_detection());
    }

    #[test]
    fn test_get_default_prompt_patterns_bash() {
        let patterns = ShellType::Bash.get_default_prompt_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains("$")));
    }

    #[test]
    fn test_get_default_prompt_patterns_zsh() {
        let patterns = ShellType::Zsh.get_default_prompt_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains("%")));
    }

    #[test]
    fn test_get_default_prompt_patterns_fish() {
        let patterns = ShellType::Fish.get_default_prompt_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains(">")));
    }

    #[test]
    fn test_get_default_prompt_patterns_other() {
        let patterns = ShellType::Other.get_default_prompt_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_shell_type_equality() {
        assert_eq!(ShellType::Bash, ShellType::Bash);
        assert_ne!(ShellType::Bash, ShellType::Zsh);
    }

    #[test]
    fn test_shell_type_hash() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(ShellType::Bash, "bash_path");
        map.insert(ShellType::Zsh, "zsh_path");
        assert_eq!(map.get(&ShellType::Bash), Some(&"bash_path"));
        assert_eq!(map.get(&ShellType::Zsh), Some(&"zsh_path"));
    }

    #[test]
    fn test_shell_type_serialization() {
        let shell = ShellType::Bash;
        let serialized = serde_json::to_string(&shell).unwrap();
        let deserialized: ShellType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(shell, deserialized);
    }
}
