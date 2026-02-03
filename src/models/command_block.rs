//! Command Block Model
//!
//! Represents a single executed command and its complete output.
//! This is a core domain entity that encapsulates command execution
//! results with ANSI formatting support.
//!
//! ## Security Note
//!
//! `CommandBlock` implements `Serialize` for internal use (testing, debugging),
//! but **should never be persisted to disk** in production. Command blocks may
//! contain sensitive output from commands. If persistence is needed in the future:
//!
//! - Sanitize sensitive fields before serialization
//! - Never serialize blocks from SSH sessions
//! - Implement opt-in persistence with explicit user consent
//! - Use encrypted storage for any persisted blocks
//!
//! Currently, only command strings (not full blocks) are persisted to the
//! history file (`~/.mosaicterm_history`).

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

use crate::models::OutputLine;

/// Execution status of a command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExecutionStatus {
    /// Command is pending execution
    #[default]
    Pending,
    /// Command is currently running
    Running,
    /// Command completed successfully
    Completed,
    /// Command failed with an error
    Failed,
    /// Command was cancelled by user
    Cancelled,
    /// Command is running in TUI fullscreen mode (no output captured)
    TuiMode,
}

/// Represents a single executed command and its complete output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBlock {
    /// Unique identifier for the block
    pub id: String,

    /// The command text that was executed
    pub command: String,

    /// Lines of output with ANSI formatting
    pub output: Vec<OutputLine>,

    /// When the command was executed (in local time)
    pub timestamp: DateTime<Local>,

    /// Success, failure, or running state
    pub status: ExecutionStatus,

    /// Directory where command was executed
    pub working_directory: PathBuf,

    /// How long the command took (None if still running)
    pub execution_time: Option<Duration>,

    /// Exit code from the command (None if still running)
    pub exit_code: Option<i32>,
}

impl CommandBlock {
    /// Create a new command block
    pub fn new(command: String, working_directory: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            command,
            output: Vec::new(),
            timestamp: Local::now(),
            status: ExecutionStatus::Pending,
            working_directory,
            execution_time: None,
            exit_code: None,
        }
    }

    /// Mark the command as started
    pub fn mark_running(&mut self) {
        self.status = ExecutionStatus::Running;
    }

    /// Mark the command as completed successfully
    pub fn mark_completed(&mut self, execution_time: Duration) {
        self.status = ExecutionStatus::Completed;
        self.execution_time = Some(execution_time);
        self.exit_code = Some(0);
    }

    /// Mark the command as failed
    pub fn mark_failed(&mut self, execution_time: Duration, exit_code: i32) {
        self.status = ExecutionStatus::Failed;
        self.execution_time = Some(execution_time);
        self.exit_code = Some(exit_code);
    }

    /// Mark the command as cancelled
    pub fn mark_cancelled(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.exit_code = Some(130); // Standard exit code for SIGINT
    }

    /// Mark the command as running in TUI fullscreen mode
    pub fn mark_tui_mode(&mut self) {
        self.status = ExecutionStatus::TuiMode;
    }

    /// Add output line to the block
    pub fn add_output_line(&mut self, line: OutputLine) {
        self.output.push(line);
    }

    /// Add multiple output lines to the block
    pub fn add_output_lines(&mut self, lines: Vec<OutputLine>) {
        self.output.extend(lines);
    }

    /// Get the total number of output lines
    pub fn output_line_count(&self) -> usize {
        self.output.len()
    }

    /// Check if the command is still running
    pub fn is_running(&self) -> bool {
        matches!(self.status, ExecutionStatus::Running)
    }

    /// Check if the command completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self.status, ExecutionStatus::Completed)
    }

    /// Check if the command failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, ExecutionStatus::Failed)
    }

    /// Get the plain text output (without ANSI codes)
    pub fn get_plain_output(&self) -> String {
        // Optimize: use references instead of cloning each string
        self.output
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get the formatted output with ANSI codes
    pub fn get_formatted_output(&self) -> String {
        self.output
            .iter()
            .map(|line| line.get_formatted_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for CommandBlock {
    fn default() -> Self {
        Self::new(String::new(), std::env::current_dir().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_command_block_creation() {
        let command = "echo 'hello'".to_string();
        let working_dir = PathBuf::from("/tmp");

        let block = CommandBlock::new(command.clone(), working_dir.clone());

        assert_eq!(block.command, command);
        assert_eq!(block.working_directory, working_dir);
        assert_eq!(block.status, ExecutionStatus::Pending);
        assert!(block.output.is_empty());
        assert!(block.execution_time.is_none());
        assert!(block.exit_code.is_none());
        assert!(!block.id.is_empty()); // UUID should be generated
    }

    #[test]
    fn test_command_block_state_transitions() {
        let mut block = CommandBlock::new("test".to_string(), PathBuf::from("/tmp"));

        // Test running state
        block.mark_running();
        assert_eq!(block.status, ExecutionStatus::Running);
        assert!(block.is_running());

        // Test completed state
        let duration = Duration::from_millis(100);
        block.mark_completed(duration);
        assert_eq!(block.status, ExecutionStatus::Completed);
        assert!(block.is_successful());
        assert_eq!(block.execution_time, Some(duration));
        assert_eq!(block.exit_code, Some(0));

        // Test failed state
        let mut block2 = CommandBlock::new("test2".to_string(), PathBuf::from("/tmp"));
        block2.mark_running();
        block2.mark_failed(duration, 1);
        assert_eq!(block2.status, ExecutionStatus::Failed);
        assert!(block2.is_failed());
        assert_eq!(block2.exit_code, Some(1));
    }

    #[test]
    fn test_output_management() {
        let mut block = CommandBlock::new("test".to_string(), PathBuf::from("/tmp"));

        let line1 = OutputLine::with_line_number("line 1", 0);
        let line2 = OutputLine::with_line_number("line 2", 1);

        block.add_output_line(line1);
        block.add_output_lines(vec![line2]);

        assert_eq!(block.output_line_count(), 2);
        assert_eq!(block.get_plain_output(), "line 1\nline 2");
    }
}
