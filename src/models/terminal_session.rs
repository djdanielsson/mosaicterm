//! Terminal Session Model
//!
//! Represents a running terminal session with its PTY process.
//! This model manages the overall terminal session state,
//! including command history and session lifecycle.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use super::{CommandBlock, PtyProcess, ShellType};

/// State of the terminal session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionState {
    /// Session is being initialized
    #[default]
    Initializing,
    /// Session is active and ready for commands
    Active,
    /// Session is being terminated
    Terminating,
    /// Session has been terminated
    Terminated,
}

/// Represents a running terminal session with its PTY process
#[derive(Debug, Clone)]
pub struct TerminalSession {
    /// Session identifier
    pub id: String,

    /// Handle to the PTY process
    pub pty_process: PtyProcess,

    /// Type of shell being used
    pub shell_type: ShellType,

    /// Current working directory
    pub working_directory: PathBuf,

    /// Environment variables
    pub environment: HashMap<String, String>,

    /// When session started
    pub start_time: DateTime<Utc>,

    /// Current session state
    pub state: SessionState,

    /// Command history for this session
    pub command_history: Vec<CommandBlock>,

    /// Maximum number of commands to keep in history
    pub max_history_size: usize,
}

impl TerminalSession {
    /// Create a new terminal session
    pub fn new(shell_type: ShellType, working_directory: PathBuf) -> Self {
        Self::with_max_history(shell_type, working_directory, 1000)
    }

    /// Create a new terminal session with specified max history size
    pub fn with_max_history(
        shell_type: ShellType,
        working_directory: PathBuf,
        max_history_size: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pty_process: PtyProcess::new(String::new(), Vec::new()),
            shell_type,
            working_directory,
            environment: std::env::vars().collect(),
            start_time: Utc::now(),
            state: SessionState::Initializing,
            command_history: Vec::new(),
            max_history_size,
        }
    }

    /// Create a new terminal session with custom environment
    pub fn with_environment(
        shell_type: ShellType,
        working_directory: PathBuf,
        environment: HashMap<String, String>,
    ) -> Self {
        let mut session = Self::new(shell_type, working_directory);
        session.environment = environment;
        session
    }

    /// Mark the session as active (PTY is ready)
    pub fn mark_active(&mut self) {
        self.state = SessionState::Active;
    }

    /// Mark the session as terminating
    pub fn mark_terminating(&mut self) {
        self.state = SessionState::Terminating;
    }

    /// Mark the session as terminated
    pub fn mark_terminated(&mut self) {
        self.state = SessionState::Terminated;
    }

    /// Check if the session is active
    pub fn is_active(&self) -> bool {
        matches!(self.state, SessionState::Active)
    }

    /// Check if the session is terminated
    pub fn is_terminated(&self) -> bool {
        matches!(self.state, SessionState::Terminated)
    }

    /// Add a command block to the history
    pub fn add_command_block(&mut self, block: CommandBlock) {
        self.command_history.push(block);

        // Enforce history size limit
        if self.command_history.len() > self.max_history_size {
            self.command_history.remove(0); // Remove oldest
        }
    }

    /// Get the current command block (most recent)
    pub fn current_command(&self) -> Option<&CommandBlock> {
        self.command_history.last()
    }

    /// Get a command block by index
    pub fn get_command(&self, index: usize) -> Option<&CommandBlock> {
        self.command_history.get(index)
    }

    /// Get the number of commands in history
    pub fn command_count(&self) -> usize {
        self.command_history.len()
    }

    /// Clear the command history
    pub fn clear_history(&mut self) {
        self.command_history.clear();
    }

    /// Get commands within a time range
    pub fn get_commands_in_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Vec<&CommandBlock> {
        self.command_history
            .iter()
            .filter(|block| block.timestamp >= start_time && block.timestamp <= end_time)
            .collect()
    }

    /// Get successful commands
    pub fn get_successful_commands(&self) -> Vec<&CommandBlock> {
        self.command_history
            .iter()
            .filter(|block| block.is_successful())
            .collect()
    }

    /// Get failed commands
    pub fn get_failed_commands(&self) -> Vec<&CommandBlock> {
        self.command_history
            .iter()
            .filter(|block| block.is_failed())
            .collect()
    }

    /// Get the session duration
    pub fn session_duration(&self) -> std::time::Duration {
        Utc::now()
            .signed_duration_since(self.start_time)
            .to_std()
            .unwrap_or_default()
    }

    /// Get session statistics
    pub fn get_statistics(&self) -> SessionStatistics {
        let total_commands = self.command_history.len();
        let successful_commands = self.get_successful_commands().len();
        let failed_commands = self.get_failed_commands().len();
        let running_commands = self
            .command_history
            .iter()
            .filter(|b| b.is_running())
            .count();

        let avg_execution_time = if total_commands > 0 {
            let total_time: std::time::Duration = self
                .command_history
                .iter()
                .filter_map(|b| b.execution_time)
                .sum();
            total_time / total_commands as u32
        } else {
            std::time::Duration::from_secs(0)
        };

        SessionStatistics {
            total_commands,
            successful_commands,
            failed_commands,
            running_commands,
            avg_execution_time,
            session_duration: self.session_duration(),
        }
    }

    /// Export session history as JSON
    pub fn export_history_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.command_history)
    }

    /// Export session history as plain text
    pub fn export_history_text(&self) -> String {
        let mut output = format!("Terminal Session: {}\n", self.id);
        output.push_str(&format!("Shell: {:?}\n", self.shell_type));
        output.push_str(&format!("Started: {}\n\n", self.start_time));

        for (i, block) in self.command_history.iter().enumerate() {
            output.push_str(&format!("Command {}: {}\n", i + 1, block.command));
            output.push_str(&format!("Status: {:?}\n", block.status));
            output.push_str(&format!("Output:\n{}\n", block.get_plain_output()));
            output.push_str(&format!("Duration: {:?}\n", block.execution_time));
            output.push_str("---\n");
        }

        output
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub total_commands: usize,
    pub successful_commands: usize,
    pub failed_commands: usize,
    pub running_commands: usize,
    pub avg_execution_time: std::time::Duration,
    pub session_duration: std::time::Duration,
}

impl Default for TerminalSession {
    fn default() -> Self {
        Self::new(
            ShellType::default(),
            std::env::current_dir().unwrap_or_default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_terminal_session_creation() {
        let shell_type = ShellType::Zsh;
        let working_dir = PathBuf::from("/tmp");

        let session = TerminalSession::new(shell_type, working_dir.clone());

        assert_eq!(session.shell_type, shell_type);
        assert_eq!(session.working_directory, working_dir);
        assert_eq!(session.state, SessionState::Initializing);
        assert!(session.command_history.is_empty());
        assert!(session.start_time <= Utc::now());
    }

    #[test]
    fn test_terminal_session_state_transitions() {
        let mut session = TerminalSession::new(ShellType::Bash, PathBuf::from("/tmp"));

        // Test initializing -> active
        session.mark_active();
        assert!(session.is_active());

        // Test active -> terminating -> terminated
        session.mark_terminating();
        assert!(!session.is_active());
        assert!(!session.is_terminated());

        session.mark_terminated();
        assert!(session.is_terminated());
    }

    #[test]
    fn test_command_history_management() {
        let mut session = TerminalSession::new(ShellType::Zsh, PathBuf::from("/tmp"));
        session.max_history_size = 3;

        // Add commands
        let cmd1 = CommandBlock::new("echo 'first'".to_string(), PathBuf::from("/tmp"));
        let cmd2 = CommandBlock::new("echo 'second'".to_string(), PathBuf::from("/tmp"));
        let cmd3 = CommandBlock::new("echo 'third'".to_string(), PathBuf::from("/tmp"));
        let cmd4 = CommandBlock::new("echo 'fourth'".to_string(), PathBuf::from("/tmp"));

        session.add_command_block(cmd1);
        session.add_command_block(cmd2);
        session.add_command_block(cmd3);
        session.add_command_block(cmd4);

        assert_eq!(session.command_count(), 3); // Should be limited to max_history_size
        assert_eq!(session.current_command().unwrap().command, "echo 'fourth'");
    }

    #[test]
    fn test_session_statistics() {
        let mut session = TerminalSession::new(ShellType::Zsh, PathBuf::from("/tmp"));

        // Add some command blocks
        let mut cmd1 = CommandBlock::new("echo 'success'".to_string(), PathBuf::from("/tmp"));
        cmd1.mark_completed(Duration::from_millis(100));

        let mut cmd2 = CommandBlock::new("failing_cmd".to_string(), PathBuf::from("/tmp"));
        cmd2.mark_failed(Duration::from_millis(50), 1);

        session.add_command_block(cmd1);
        session.add_command_block(cmd2);

        let stats = session.get_statistics();

        assert_eq!(stats.total_commands, 2);
        assert_eq!(stats.successful_commands, 1);
        assert_eq!(stats.failed_commands, 1);
        assert_eq!(stats.running_commands, 0);
        assert!(stats.avg_execution_time >= Duration::from_millis(75));
    }

    #[test]
    fn test_environment_handling() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let session =
            TerminalSession::with_environment(ShellType::Bash, PathBuf::from("/tmp"), env.clone());

        assert_eq!(
            session.environment.get("TEST_VAR"),
            Some(&"test_value".to_string())
        );
    }

    #[test]
    fn test_shell_type_variants() {
        assert_eq!(ShellType::Zsh, ShellType::Zsh);
        assert_eq!(ShellType::Bash, ShellType::Bash);
        assert_eq!(ShellType::Fish, ShellType::Fish);

        let custom = ShellType::Other;
        assert_eq!(custom, ShellType::Other);
    }
}
