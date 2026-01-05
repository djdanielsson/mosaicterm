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

/// Default commands suitable for direct execution (non-interactive, quick)
pub const DEFAULT_DIRECT_EXECUTION_COMMANDS: &[&str] = &[
    "ls",
    "pwd",
    "echo",
    "cat",
    "grep",
    "find",
    "wc",
    "sort",
    "head",
    "tail",
    "whoami",
    "date",
    "df",
    "du",
    "ps",
    "uname",
    "which",
    "whereis",
    "file",
    "stat",
    "tree",
    "curl",
    "wget",
    "basename",
    "dirname",
    "realpath",
    "env",
    "printenv",
    "hostname",
    "id",
    "groups",
    "touch",
    "mkdir",
    "rmdir",
    "cp",
    "mv",
    "ln",
    "chmod",
    "chown",
    "md5sum",
    "sha256sum",
    "wc",
    "cut",
    "tr",
    "sed",
    "awk",
    "xargs",
    "diff",
    "comm",
    "uniq",
    "tee",
];

/// Default commands that require interactive PTY mode
pub const DEFAULT_PTY_MODE_COMMANDS: &[&str] = &[
    "vim", "nvim", "nano", "emacs", "less", "more", "top", "htop", "btop", "ssh", "telnet", "ftp",
    "sftp", "mysql", "psql", "sqlite3", "mongo", "node", "python", "python3", "ruby", "irb", "lua",
    "perl", "php", "bash", "zsh", "fish", "sh", "ksh", "csh", "tcsh", "dash", "man", "info",
    "screen", "tmux", "docker", "kubectl",
];

/// Direct command execution without shell/PTY overhead
pub struct DirectExecutor {
    /// Current working directory
    working_dir: PathBuf,
    /// Environment variables
    env_vars: HashMap<String, String>,
    /// Default timeout for commands
    default_timeout: Duration,
    /// Commands suitable for direct execution
    direct_execution_commands: Vec<String>,
    /// Commands requiring PTY mode
    pty_mode_commands: Vec<String>,
}

impl DirectExecutor {
    /// Create new direct executor with default command lists
    pub fn new() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env_vars: HashMap::new(),
            default_timeout: Duration::from_secs(30),
            direct_execution_commands: DEFAULT_DIRECT_EXECUTION_COMMANDS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            pty_mode_commands: DEFAULT_PTY_MODE_COMMANDS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Create executor with custom command lists
    pub fn with_commands(direct_commands: Vec<String>, pty_commands: Vec<String>) -> Self {
        Self {
            direct_execution_commands: direct_commands,
            pty_mode_commands: pty_commands,
            ..Self::new()
        }
    }

    /// Add a command to the direct execution list
    pub fn add_direct_command(&mut self, cmd: &str) {
        if !self.direct_execution_commands.contains(&cmd.to_string()) {
            self.direct_execution_commands.push(cmd.to_string());
        }
    }

    /// Add a command to the PTY mode list
    pub fn add_pty_command(&mut self, cmd: &str) {
        if !self.pty_mode_commands.contains(&cmd.to_string()) {
            self.pty_mode_commands.push(cmd.to_string());
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
            return Err(Error::EmptyCommand);
        }

        let (cmd, args) = (parts[0], &parts[1..]);

        // Execute with timeout
        let execution_result = timeout(self.default_timeout, self.run_command(cmd, args)).await;

        match execution_result {
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Add stdout lines
                for line in stdout.lines() {
                    if !line.trim().is_empty() {
                        command_block.add_output_line(OutputLine::new(line));
                    }
                }

                // Add stderr lines if any
                for line in stderr.lines() {
                    if !line.trim().is_empty() {
                        command_block.add_output_line(OutputLine::new(format!("stderr: {}", line)));
                    }
                }

                // Mark as completed with exit code
                let duration = Duration::from_millis(100); // Small duration for direct commands
                command_block.mark_completed(duration);

                if exit_code != 0 {
                    command_block.add_output_line(OutputLine::new(format!(
                        "Command exited with code: {}",
                        exit_code
                    )));
                }
            }
            Ok(Err(e)) => {
                command_block.add_output_line(OutputLine::new(format!("Error: {}", e)));
                command_block.mark_completed(Duration::from_millis(0));
            }
            Err(_) => {
                command_block.add_output_line(OutputLine::new("Command timed out"));
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
            .map_err(|e| Error::CommandSpawnFailed {
                command: cmd.to_string(),
                reason: e.to_string(),
            })?;

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

    /// Check if command should use direct execution (instance method using configured list)
    pub fn should_use_direct_execution(&self, command: &str) -> bool {
        let cmd = command.split_whitespace().next().unwrap_or("");
        self.direct_execution_commands.iter().any(|c| c == cmd)
    }

    /// Check if command requires PTY mode (instance method using configured list)
    pub fn requires_pty_mode(&self, command: &str) -> bool {
        let cmd = command.split_whitespace().next().unwrap_or("");
        self.pty_mode_commands.iter().any(|c| c == cmd)
    }

    /// Static check using default command list (for backward compatibility)
    pub fn check_direct_execution(command: &str) -> bool {
        let cmd = command.split_whitespace().next().unwrap_or("");
        DEFAULT_DIRECT_EXECUTION_COMMANDS.contains(&cmd)
    }
}

impl Default for DirectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Static check if command requires PTY mode (uses default list)
pub fn requires_pty_mode(command: &str) -> bool {
    let cmd = command.split_whitespace().next().unwrap_or("");
    DEFAULT_PTY_MODE_COMMANDS.contains(&cmd)
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
        let executor = DirectExecutor::new();
        assert!(executor.should_use_direct_execution("ls -la"));
        assert!(executor.should_use_direct_execution("pwd"));
        assert!(!executor.should_use_direct_execution("vim file.txt"));
    }

    #[test]
    fn test_requires_pty_mode() {
        let executor = DirectExecutor::new();
        assert!(executor.requires_pty_mode("vim file.txt"));
        assert!(executor.requires_pty_mode("ssh user@host"));
        assert!(!executor.requires_pty_mode("ls -la"));
    }

    #[test]
    fn test_custom_commands() {
        let mut executor = DirectExecutor::new();

        // Add custom direct command
        executor.add_direct_command("mycommand");
        assert!(executor.should_use_direct_execution("mycommand"));

        // Add custom PTY command
        executor.add_pty_command("myinteractive");
        assert!(executor.requires_pty_mode("myinteractive"));
    }

    #[test]
    fn test_static_check() {
        // Test backward-compatible static method
        assert!(DirectExecutor::check_direct_execution("ls"));
        assert!(!DirectExecutor::check_direct_execution("vim"));
    }

    #[test]
    fn test_static_requires_pty() {
        assert!(requires_pty_mode("vim"));
        assert!(requires_pty_mode("ssh"));
        assert!(!requires_pty_mode("echo"));
    }
}
