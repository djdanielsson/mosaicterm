//! PTY Process Model
//!
//! Manages the pseudoterminal process lifecycle and I/O streams.
//! This model represents the low-level PTY process with its
//! stdin, stdout, and stderr streams.

// Note: AsyncRead and AsyncWrite will be used when implementing actual PTY I/O
// use tokio::io::{AsyncRead, AsyncWrite};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Represents the state of a PTY process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PtyState {
    /// Process has been created but not started
    #[default]
    Created,
    /// Process is currently running
    Running,
    /// Process has terminated
    Terminated,
}

/// Manages the pseudoterminal process lifecycle
#[derive(Debug, Clone)]
pub struct PtyProcess {
    /// OS process identifier
    pub pid: Option<u32>,

    /// Current state of the process
    pub state: PtyState,

    /// When the process was started
    pub start_time: Option<DateTime<Utc>>,

    /// When the process terminated (if applicable)
    pub end_time: Option<DateTime<Utc>>,

    /// Exit code (if process has terminated)
    pub exit_code: Option<i32>,

    /// Command that was executed
    pub command: String,

    /// Arguments passed to the command
    pub args: Vec<String>,
}

impl PtyProcess {
    /// Create a new PTY process in the Created state
    pub fn new(command: String, args: Vec<String>) -> Self {
        Self {
            pid: None,
            state: PtyState::Created,
            start_time: None,
            end_time: None,
            exit_code: None,
            command,
            args,
        }
    }

    /// Mark the process as started with the given PID
    pub fn mark_started(&mut self, pid: u32) {
        self.pid = Some(pid);
        self.state = PtyState::Running;
        self.start_time = Some(Utc::now());
    }

    /// Mark the process as terminated with the given exit code
    pub fn mark_terminated(&mut self, exit_code: i32) {
        self.state = PtyState::Terminated;
        self.end_time = Some(Utc::now());
        self.exit_code = Some(exit_code);
    }

    /// Check if the process is currently running
    pub fn is_running(&self) -> bool {
        matches!(self.state, PtyState::Running)
    }

    /// Check if the process has terminated
    pub fn is_terminated(&self) -> bool {
        matches!(self.state, PtyState::Terminated)
    }

    /// Check if the process was created but not started
    pub fn is_created(&self) -> bool {
        matches!(self.state, PtyState::Created)
    }

    /// Get the execution duration if the process has terminated
    pub fn execution_duration(&self) -> Option<std::time::Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => {
                Some(end.signed_duration_since(start).to_std().unwrap_or_default())
            }
            _ => None,
        }
    }

    /// Check if the process exited successfully (exit code 0)
    pub fn exited_successfully(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Get a display string for the process
    pub fn display_string(&self) -> String {
        let state_str = match self.state {
            PtyState::Created => "Created",
            PtyState::Running => "Running",
            PtyState::Terminated => "Terminated",
        };

        let pid_str = self.pid.map_or("N/A".to_string(), |pid| pid.to_string());

        format!("{} [{}] - {} {} {}",
                self.command,
                pid_str,
                state_str,
                self.args.join(" "),
                self.exit_code.map_or(String::new(), |code| format!("(exit: {})", code)))
    }
}

impl Default for PtyProcess {
    fn default() -> Self {
        Self::new(String::new(), Vec::new())
    }
}

impl std::fmt::Display for PtyProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_process_creation() {
        let command = "/bin/echo".to_string();
        let args = vec!["hello".to_string()];

        let process = PtyProcess::new(command.clone(), args.clone());

        assert_eq!(process.command, command);
        assert_eq!(process.args, args);
        assert!(process.is_created());
        assert!(process.pid.is_none());
        assert!(process.start_time.is_none());
        assert!(process.end_time.is_none());
        assert!(process.exit_code.is_none());
    }

    #[test]
    fn test_pty_process_state_transitions() {
        let mut process = PtyProcess::new("/bin/sleep".to_string(), vec!["1".to_string()]);

        // Test created -> running
        process.mark_started(12345);
        assert!(process.is_running());
        assert_eq!(process.pid, Some(12345));
        assert!(process.start_time.is_some());
        assert!(process.end_time.is_none());

        // Test running -> terminated
        process.mark_terminated(0);
        assert!(process.is_terminated());
        assert_eq!(process.exit_code, Some(0));
        assert!(process.end_time.is_some());
        assert!(process.exited_successfully());
    }

    #[test]
    fn test_pty_process_execution_duration() {
        let mut process = PtyProcess::new("test".to_string(), vec![]);

        // No duration if not started
        assert!(process.execution_duration().is_none());

        // No duration if not finished
        process.mark_started(123);
        assert!(process.execution_duration().is_none());

        // Duration available when finished
        std::thread::sleep(std::time::Duration::from_millis(10));
        process.mark_terminated(0);
        assert!(process.execution_duration().is_some());
        assert!(process.execution_duration().unwrap() >= std::time::Duration::from_millis(10));
    }

    #[test]
    fn test_pty_process_display_string() {
        let process = PtyProcess::new("/bin/ls".to_string(), vec!["-la".to_string()]);
        let display = process.display_string();

        assert!(display.contains("/bin/ls"));
        assert!(display.contains("-la"));
        assert!(display.contains("Created"));
        assert!(display.contains("N/A"));
    }

    #[test]
    fn test_pty_process_with_exit_code() {
        let mut process = PtyProcess::new("failing_command".to_string(), vec![]);

        process.mark_started(456);
        process.mark_terminated(42);

        assert!(!process.exited_successfully());
        assert_eq!(process.exit_code, Some(42));

        let display = process.display_string();
        assert!(display.contains("(exit: 42)"));
    }
}
