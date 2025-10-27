//! Modern command execution without PTY/prompt detection
//!
//! This module provides clean, fast command execution by directly spawning
//! processes instead of relying on shell prompt parsing.

use crate::error::{Error, Result};
use crate::models::{CommandBlock, OutputLine};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Direct command execution without shell/PTY overhead
pub struct DirectExecutor {
    /// Current working directory
    working_dir: PathBuf,
    /// Environment variables
    env_vars: HashMap<String, String>,
    /// Default timeout for commands
    default_timeout: Duration,
}

impl DirectExecutor {
    /// Create new direct executor
    pub fn new() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env_vars: HashMap::new(),
            default_timeout: Duration::from_secs(30),
        }
    }

    /// Execute a command directly and return results immediately
    pub async fn execute_command(&self, command_str: &str) -> Result<CommandBlock> {
        let mut command_block =
            CommandBlock::new(command_str.to_string(), self.working_dir.clone());
        command_block.mark_running();

        // Parse command and arguments
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err(Error::Other("Empty command".to_string()));
        }

        let (cmd, args) = (parts[0], &parts[1..]);

        // Execute with timeout
        let execution_result = timeout(self.default_timeout, self.run_command(cmd, args)).await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Add stdout lines
                for line in stdout.lines() {
                    if !line.trim().is_empty() {
                        command_block.add_output_line(OutputLine {
                            text: line.to_string(),
                            ansi_codes: vec![],
                            line_number: 0,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }

                // Add stderr lines if any
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        command_block.add_output_line(OutputLine {
                            text: format!("stderr: {}", line),
                            ansi_codes: vec![],
                            line_number: 0,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }

                // Mark as completed with exit code
                let duration = Duration::from_millis(100); // Small duration for direct commands
                command_block.mark_completed(duration);

                if exit_code != 0 {
                    command_block.add_output_line(OutputLine {
                        text: format!("Command exited with code: {}", exit_code),
                        ansi_codes: vec![],
                        line_number: 0,
                        timestamp: chrono::Utc::now(),
                    });
                }
            }
            Ok(Err(e)) => {
                command_block.add_output_line(OutputLine {
                    text: format!("Error: {}", e),
                    ansi_codes: vec![],
                    line_number: 0,
                    timestamp: chrono::Utc::now(),
                });
                command_block.mark_completed(Duration::from_millis(0));
            }
            Err(_) => {
                command_block.add_output_line(OutputLine {
                    text: "Command timed out".to_string(),
                    ansi_codes: vec![],
                    line_number: 0,
                    timestamp: chrono::Utc::now(),
                });
                command_block.mark_completed(Duration::from_millis(0));
            }
        }

        Ok(command_block)
    }

    /// Run command and capture output
    async fn run_command(&self, cmd: &str, args: &[&str]) -> Result<(String, String, i32)> {
        let output = Command::new(cmd)
            .args(args)
            .current_dir(&self.working_dir)
            .envs(&self.env_vars)
            .output()
            .await
            .map_err(|e| Error::Other(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    /// Set working directory
    pub fn set_working_dir(&mut self, dir: PathBuf) {
        self.working_dir = dir;
    }

    /// Set environment variable
    pub fn set_env(&mut self, key: String, value: String) {
        self.env_vars.insert(key, value);
    }

    /// Check if command should use direct execution
    pub fn should_use_direct_execution(command: &str) -> bool {
        let cmd = command.split_whitespace().next().unwrap_or("");

        // Commands that work well with direct execution
        matches!(
            cmd,
            "ls" | "pwd"
                | "echo"
                | "cat"
                | "grep"
                | "find"
                | "wc"
                | "sort"
                | "head"
                | "tail"
                | "whoami"
                | "date"
                | "df"
                | "du"
                | "ps"
                | "uname"
                | "which"
                | "whereis"
                | "file"
                | "stat"
                | "tree"
                | "curl"
                | "wget"
        )
    }
}

impl Default for DirectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Commands that require interactive PTY mode
pub fn requires_pty_mode(command: &str) -> bool {
    let cmd = command.split_whitespace().next().unwrap_or("");

    matches!(
        cmd,
        "vim"
            | "nano"
            | "emacs"
            | "less"
            | "more"
            | "top"
            | "htop"
            | "ssh"
            | "telnet"
            | "ftp"
            | "mysql"
            | "psql"
            | "node"
            | "python"
            | "bash"
            | "zsh"
            | "fish"
            | "sh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_direct_execution() {
        let executor = DirectExecutor::new();
        let result = executor.execute_command("echo hello").await;
        assert!(result.is_ok());

        let command_block = result.unwrap();
        assert_eq!(command_block.command, "echo hello");
        assert!(!command_block.output.is_empty());
    }

    #[test]
    fn test_should_use_direct_execution() {
        assert!(DirectExecutor::should_use_direct_execution("ls -la"));
        assert!(DirectExecutor::should_use_direct_execution("pwd"));
        assert!(!DirectExecutor::should_use_direct_execution("vim file.txt"));
    }
}
