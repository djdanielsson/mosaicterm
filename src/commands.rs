//! Command parsing and validation utilities
//!
//! This module provides shell-specific command processing including
//! validation, tilde expansion, history expansion, and multi-line detection.

use crate::error::{Error, Result};
use crate::models::ShellType;
use crate::terminal::input::validation;
use std::collections::HashMap;

/// Command processing utilities for shell-specific features
pub struct CommandProcessor {
    /// Shell type for command processing
    shell_type: ShellType,
    /// Environment variables
    environment: HashMap<String, String>,
    /// Command history for history expansion
    history: Vec<String>,
}

impl CommandProcessor {
    /// Create a new command processor
    pub fn new(shell_type: ShellType) -> Self {
        Self {
            shell_type,
            environment: std::env::vars().collect(),
            history: Vec::new(),
        }
    }

    /// Validate and prepare a command for execution
    pub fn prepare_command(&self, input: &str) -> Result<String> {
        let trimmed = input.trim();

        // Validate command
        validation::validate_command(trimmed)?;

        // Sanitize command
        let sanitized = validation::sanitize_command(trimmed);

        // Handle shell-specific command processing
        let processed = self.process_shell_specific(&sanitized)?;

        Ok(processed)
    }

    /// Check if input represents a complete command
    pub fn is_complete_command(&self, input: &str) -> bool {
        let trimmed = input.trim();

        // Empty input is not complete
        if trimmed.is_empty() {
            return false;
        }

        // Check for shell-specific completion rules
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh => {
                // Check for backslash continuation
                if trimmed.ends_with('\\') {
                    return false;
                }

                // Check for unclosed quotes
                let quote_count = trimmed.chars().filter(|&c| c == '"' || c == '\'').count();
                if quote_count % 2 != 0 {
                    return false;
                }

                // Check for unclosed parentheses/braces
                let open_parens = trimmed.chars().filter(|&c| c == '(').count();
                let close_parens = trimmed.chars().filter(|&c| c == ')').count();
                if open_parens != close_parens {
                    return false;
                }

                true
            }
            ShellType::Fish => {
                // Fish has simpler syntax, mainly check for backslash
                !trimmed.ends_with('\\')
            }
            _ => {
                // For other shells, basic check
                !trimmed.ends_with('\\') && !trimmed.is_empty()
            }
        }
    }

    /// Process shell-specific command transformations
    fn process_shell_specific(&self, command: &str) -> Result<String> {
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh => {
                // Handle bash/zsh specific features
                self.process_bash_zsh_command(command)
            }
            ShellType::Fish => {
                // Handle fish specific features (currently passthrough)
                Ok(command.to_string())
            }
            _ => Ok(command.to_string()),
        }
    }

    /// Process bash/zsh specific commands
    fn process_bash_zsh_command(&self, command: &str) -> Result<String> {
        let mut processed = command.to_string();

        // Handle history expansion (!!)
        if command.contains("!!") {
            if let Some(last_cmd) = self.history.last() {
                processed = command.replace("!!", last_cmd);
            } else {
                return Err(Error::Other("No previous command in history".to_string()));
            }
        }

        // Handle tilde expansion
        if command.starts_with('~') {
            if let Some(home) = self.environment.get("HOME") {
                processed = command.replacen('~', home, 1);
            }
        }

        Ok(processed)
    }

    /// Add command to history
    pub fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() {
            // Remove duplicates
            self.history.retain(|c| c != &command);
            self.history.push(command);

            // Keep last 1000 commands
            if self.history.len() > 1000 {
                self.history.remove(0);
            }
        }
    }

    /// Get command history
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Get last command from history
    pub fn last_command(&self) -> Option<&String> {
        self.history.last()
    }

    /// Clear command history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Expand tilde in path
    pub fn expand_tilde(&self, path: &str) -> String {
        if path.starts_with('~') {
            if let Some(home) = self.environment.get("HOME") {
                return path.replacen('~', home, 1);
            }
        }
        path.to_string()
    }

    /// Expand environment variables in command
    pub fn expand_env_vars(&self, command: &str) -> String {
        let mut result = command.to_string();
        for (key, value) in &self.environment {
            let pattern = format!("${}", key);
            result = result.replace(&pattern, value);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_processor_creation() {
        let processor = CommandProcessor::new(ShellType::Bash);
        assert!(processor.history().is_empty());
    }

    #[test]
    fn test_command_validation() {
        let processor = CommandProcessor::new(ShellType::Bash);
        assert!(processor.prepare_command("echo hello").is_ok());
        assert!(processor.prepare_command("").is_err());
        assert!(processor.prepare_command("rm -rf /").is_err());
    }

    #[test]
    fn test_command_completion_check() {
        let processor = CommandProcessor::new(ShellType::Bash);
        assert!(processor.is_complete_command("echo hello"));
        assert!(!processor.is_complete_command("echo hello \\"));
        assert!(!processor.is_complete_command("echo 'hello"));
    }

    #[test]
    fn test_history_management() {
        let mut processor = CommandProcessor::new(ShellType::Bash);
        processor.add_to_history("cmd1".to_string());
        processor.add_to_history("cmd2".to_string());

        assert_eq!(processor.history().len(), 2);
        assert_eq!(processor.last_command(), Some(&"cmd2".to_string()));
    }

    #[test]
    fn test_bash_history_expansion() {
        let mut processor = CommandProcessor::new(ShellType::Bash);
        processor.add_to_history("echo hello".to_string());

        let result = processor.prepare_command("!!").unwrap();
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn test_tilde_expansion() {
        let processor = CommandProcessor::new(ShellType::Bash);
        let expanded = processor.expand_tilde("~/documents");
        assert!(!expanded.starts_with('~'));
    }
}
