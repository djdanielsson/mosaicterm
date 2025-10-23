//! Command parsing and execution
//!
//! This module handles command validation, parsing, and
//! execution coordination between the UI and PTY layers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use regex::Regex;
use crate::error::{Error, Result};
use crate::models::{CommandBlock, TerminalSession};
use crate::terminal::input::{CommandInputProcessor, InputResult, validation};
use crate::pty::{PtyManager, PtyHandle};

/// Command execution context
pub struct CommandContext {
    /// Current working directory
    working_directory: PathBuf,
    /// Environment variables
    environment: HashMap<String, String>,
    /// Command history
    history: Vec<String>,
    /// Maximum history size
    max_history_size: usize,
    /// Shell type for command processing
    shell_type: crate::models::ShellType,
}

impl CommandContext {
    /// Create a new command context
    pub fn new(working_directory: PathBuf, shell_type: crate::models::ShellType) -> Self {
        Self {
            working_directory,
            environment: std::env::vars().collect(),
            history: Vec::new(),
            max_history_size: 1000,
            shell_type,
        }
    }

    /// Create context from terminal session
    pub fn from_session(session: &TerminalSession) -> Self {
        Self::new(session.working_directory.clone(), session.shell_type.clone())
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
            crate::models::ShellType::Bash | crate::models::ShellType::Zsh => {
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
            },
            crate::models::ShellType::Fish => {
                // Fish has simpler syntax, mainly check for backslash
                !trimmed.ends_with('\\')
            },
            _ => {
                // For other shells, basic check
                !trimmed.ends_with('\\') && !trimmed.is_empty()
            }
        }
    }

    /// Process shell-specific command transformations
    fn process_shell_specific(&self, command: &str) -> Result<String> {
        match self.shell_type {
            crate::models::ShellType::Bash | crate::models::ShellType::Zsh => {
                // Handle bash/zsh specific features
                self.process_bash_zsh_command(command)
            },
            crate::models::ShellType::Fish => {
                // Handle fish specific features
                self.process_fish_command(command)
            },
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

    /// Process fish specific commands
    fn process_fish_command(&self, command: &str) -> Result<String> {
        // Fish has different syntax, but for now just pass through
        Ok(command.to_string())
    }

    /// Execute a command via PTY
    pub async fn execute_command(
        &mut self,
        manager: &mut PtyManager,
        handle: &PtyHandle,
        command: &str,
    ) -> Result<CommandBlock> {
        let prepared_command = self.prepare_command(command)?;

        // Create command block
        let mut command_block = CommandBlock::new(
            prepared_command.clone(),
            self.working_directory.clone(),
        );

        // Mark as running
        command_block.mark_running();

        // Send command to PTY
        let command_with_newline = format!("{}\n", prepared_command);
        manager.send_input(handle, command_with_newline.as_bytes()).await?;

        // Add to history
        self.add_to_history(prepared_command);

        Ok(command_block)
    }

    /// Add command to history
    pub fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() {
            // Remove duplicates
            self.history.retain(|c| c != &command);

            self.history.push(command);

            // Enforce history size limit
            if self.history.len() > self.max_history_size {
                self.history.remove(0);
            }
        }
    }

    /// Get command history
    pub fn get_history(&self) -> &[String] {
        &self.history
    }

    /// Search command history
    pub fn search_history(&self, pattern: &str) -> Vec<&String> {
        let regex = match Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        self.history.iter()
            .filter(|cmd| regex.is_match(cmd))
            .collect()
    }

    /// Get last command from history
    pub fn get_last_command(&self) -> Option<&String> {
        self.history.last()
    }


    /// Execute command and collect output
    pub async fn execute_command_with_output(
        &mut self,
        manager: &mut PtyManager,
        handle: &PtyHandle,
        command: &str,
        output_processor: &mut crate::terminal::output::OutputProcessor,
    ) -> Result<CommandBlock> {
        // Start command execution
        let mut command_block = self.execute_command(manager, handle, command).await?;

        // Read output for a short period to collect initial response
        let output_lines = output_processor.read_output_until_timeout(manager, handle, 1000).await?;

        // Add output lines to command block
        for line in output_lines {
            command_block.add_output_line(line);
        }

        // Mark command as completed
        command_block.mark_completed(Duration::from_millis(100)); // Dummy execution time

        Ok(command_block)
    }

    /// Clear command history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Get working directory
    pub fn working_directory(&self) -> &PathBuf {
        &self.working_directory
    }

    /// Set working directory
    pub fn set_working_directory(&mut self, path: PathBuf) {
        self.working_directory = path;
    }

    /// Get environment variable
    pub fn get_env(&self, key: &str) -> Option<&String> {
        self.environment.get(key)
    }

    /// Set environment variable
    pub fn set_env(&mut self, key: String, value: String) {
        self.environment.insert(key, value);
    }
}

/// Command executor for coordinating command execution
pub struct CommandExecutor {
    context: CommandContext,
    input_processor: CommandInputProcessor,
}

impl CommandExecutor {
    /// Create a new command executor
    pub fn new(context: CommandContext) -> Self {
        Self {
            context,
            input_processor: CommandInputProcessor::new(),
        }
    }

    /// Process user input and potentially execute command
    pub async fn process_input(
        &mut self,
        input: &str,
        manager: &mut PtyManager,
        handle: &PtyHandle,
    ) -> Result<Option<CommandBlock>> {
        // Process input through the input processor
        for ch in input.chars() {
            match self.input_processor.process_char(ch) {
                InputResult::CommandReady(command) => {
                    // Execute the command
                    let command_block = self.context.execute_command(manager, handle, &command).await?;
                    return Ok(Some(command_block));
                },
                InputResult::EmptyCommand => {
                    return Ok(None);
                },
                _ => {
                    // Continue processing input
                }
            }
        }

        Ok(None)
    }

    /// Get current input text
    pub fn current_input(&self) -> &str {
        self.input_processor.current_command()
    }

    /// Get command history
    pub fn history(&self) -> &[String] {
        self.context.get_history()
    }

    /// Get context reference
    pub fn context(&self) -> &CommandContext {
        &self.context
    }

    /// Get mutable context reference
    pub fn context_mut(&mut self) -> &mut CommandContext {
        &mut self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ShellType;

    #[test]
    fn test_command_context_creation() {
        let context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        assert_eq!(context.working_directory(), &PathBuf::from("/tmp"));
        assert!(context.get_history().is_empty());
    }

    #[test]
    fn test_command_validation() {
        let context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        assert!(context.prepare_command("echo hello").is_ok());
        assert!(context.prepare_command("").is_err());
        assert!(context.prepare_command("rm -rf /").is_err());
    }

    #[test]
    fn test_command_completion_check() {
        let context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        assert!(context.is_complete_command("echo hello"));
        assert!(!context.is_complete_command("echo hello \\"));
        assert!(!context.is_complete_command("echo 'hello"));
    }

    #[test]
    fn test_history_management() {
        let mut context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        context.add_to_history("cmd1".to_string());
        context.add_to_history("cmd2".to_string());

        assert_eq!(context.get_history().len(), 2);
        assert_eq!(context.get_last_command(), Some(&"cmd2".to_string()));
    }

    #[test]
    fn test_bash_history_expansion() {
        let mut context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        context.add_to_history("echo hello".to_string());

        let result = context.prepare_command("!!").unwrap();
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn test_command_executor_creation() {
        let context = CommandContext::new(
            PathBuf::from("/tmp"),
            ShellType::Bash,
        );

        let executor = CommandExecutor::new(context);
        assert!(executor.current_input().is_empty());
        assert!(executor.history().is_empty());
    }
}
