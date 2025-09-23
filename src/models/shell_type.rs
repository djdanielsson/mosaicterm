//! Shell Type Definitions
//!
//! Canonical definition of shell types supported by MosaicTerm.
//! This consolidates multiple scattered definitions into a single source of truth.

use serde::{Deserialize, Serialize};

/// Type of shell being used
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

impl Default for ShellType {
    fn default() -> Self {
        ShellType::Zsh
    }
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
    pub fn from_str(s: &str) -> Self {
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
                r"^\$ $".to_string(),           // Simple dollar prompt
                r"^\[.*\]\$ $".to_string(),     // User@host prompt
                r"^.*\$ $".to_string(),         // Generic dollar prompt
            ],
            ShellType::Zsh => vec![
                r"^% $".to_string(),            // Simple percent prompt
                r"^\[.*\]% $".to_string(),      // User@host prompt
                r"^.*% $".to_string(),          // Generic percent prompt
            ],
            ShellType::Fish => vec![
                r"^> $".to_string(),            // Simple arrow prompt
                r"^.*> $".to_string(),          // Generic arrow prompt
            ],
            _ => vec![r"^.*[$%>]$ $".to_string()], // Generic fallback
        }
    }
}
