//! PTY Manager
//!
//! Core PTY management system that handles creation, lifecycle,
//! and coordination of pseudoterminal processes.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

use super::process::spawn_pty_process;
use super::streams::PtyStreams;
use crate::error::{Error, Result};
use crate::models::PtyProcess;

/// Handle to a managed PTY process
#[derive(Debug, Clone)]
pub struct PtyHandle {
    /// Unique identifier for this PTY instance
    pub id: String,
    /// Process ID of the running process
    pub pid: Option<u32>,
}

impl PtyHandle {
    /// Create a new PTY handle
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pid: None,
        }
    }

    /// Set the process ID
    fn set_pid(&mut self, pid: u32) {
        self.pid = Some(pid);
    }
}

impl Default for PtyHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a PTY process
#[derive(Debug, Clone)]
pub struct PtyInfo {
    /// Handle identifier
    pub id: String,
    /// Process ID
    pub pid: Option<u32>,
    /// Command being executed
    pub command: String,
    /// Working directory
    pub working_directory: std::path::PathBuf,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// Current status
    pub is_alive: bool,
}

/// Main PTY manager that coordinates all PTY operations
pub struct PtyManager {
    /// Active PTY processes
    active_processes: HashMap<String, PtyProcess>,
    /// PTY streams for I/O operations
    streams: HashMap<String, PtyStreams>,
}

impl PtyManager {
    /// Create a new PTY manager
    pub fn new() -> Self {
        Self {
            active_processes: HashMap::new(),
            streams: HashMap::new(),
        }
    }

    /// Create and start a new PTY process
    pub async fn create_pty(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        working_directory: Option<&std::path::Path>,
    ) -> Result<PtyHandle> {
        // Validate command exists and is executable
        Self::validate_command(command)?;

        // Create PTY handle
        let mut handle = PtyHandle::new();

        // Spawn the PTY process
        let (process, streams) = spawn_pty_process(command, args, env, working_directory).await?;

        // Set the PID in the handle
        if let Some(pid) = process.pid {
            handle.set_pid(pid);
        }

        // Store the process and streams
        let handle_id = handle.id.clone();
        self.active_processes.insert(handle_id.clone(), process);
        self.streams.insert(handle_id, streams);

        Ok(handle)
    }

    /// Check if a PTY process is still alive
    pub fn is_alive(&self, handle: &PtyHandle) -> bool {
        if let Some(process) = self.active_processes.get(&handle.id) {
            process.is_running()
        } else {
            false
        }
    }

    /// Terminate a PTY process
    pub async fn terminate_pty(&mut self, handle: &PtyHandle) -> Result<()> {
        let process_id = handle.id.clone();

        // Remove from active processes (this will drop the process and terminate it)
        if let Some(mut process) = self.active_processes.remove(&process_id) {
            // Try graceful termination first
            if process.is_running() {
                // In a real implementation, we'd send SIGTERM here
                // For now, we'll just mark it as terminated
                process.mark_terminated(0);
            }
        }

        // Remove streams
        self.streams.remove(&process_id);

        Ok(())
    }

    /// Get information about a PTY process
    pub fn get_info(&self, handle: &PtyHandle) -> Result<PtyInfo> {
        if let Some(process) = self.active_processes.get(&handle.id) {
            Ok(PtyInfo {
                id: handle.id.clone(),
                pid: process.pid,
                command: process.command.clone(),
                working_directory: std::path::PathBuf::from("."), // TODO: Get actual working directory
                start_time: process.start_time.unwrap_or_else(Utc::now),
                is_alive: process.is_running(),
            })
        } else {
            Err(Error::Other(format!("PTY process {} not found", handle.id)))
        }
    }

    /// Send data to a PTY process
    pub async fn send_input(&mut self, handle: &PtyHandle, data: &[u8]) -> Result<()> {
        if let Some(streams) = self.streams.get_mut(&handle.id) {
            streams.write(data).await
        } else {
            Err(Error::Other(format!(
                "PTY streams for {} not found",
                handle.id
            )))
        }
    }

    /// Read output from a PTY process, with a timeout in milliseconds
    pub async fn read_output(&mut self, handle: &PtyHandle, timeout_ms: u64) -> Result<Vec<u8>> {
        if let Some(streams) = self.streams.get_mut(&handle.id) {
            if timeout_ms == 0 {
                streams.read().await
            } else {
                streams.read_with_timeout(timeout_ms).await
            }
        } else {
            Err(Error::Other(format!(
                "PTY streams for {} not found",
                handle.id
            )))
        }
    }

    /// Try to read output immediately without waiting
    pub fn try_read_output_now(&mut self, handle: &PtyHandle) -> Result<Vec<u8>> {
        if let Some(streams) = self.streams.get_mut(&handle.id) {
            streams.try_read_now()
        } else {
            Err(Error::Other(format!(
                "PTY streams for {} not found",
                handle.id
            )))
        }
    }

    /// Get the number of active PTY processes
    pub fn active_count(&self) -> usize {
        self.active_processes.len()
    }

    /// Clean up terminated processes
    pub fn cleanup_terminated(&mut self) {
        let terminated: Vec<String> = self
            .active_processes
            .iter()
            .filter(|(_, process)| !process.is_running())
            .map(|(id, _)| id.clone())
            .collect();

        for id in terminated {
            self.active_processes.remove(&id);
            self.streams.remove(&id);
        }
    }

    /// Validate that a command exists and is executable
    fn validate_command(command: &str) -> Result<()> {
        // Check if command exists in PATH
        if std::process::Command::new("which")
            .arg(command)
            .output()
            .map_err(|_| Error::Other(format!("Command '{}' not found in PATH", command)))?
            .status
            .success()
        {
            Ok(())
        } else {
            Err(Error::Other(format!(
                "Command '{}' not found or not executable",
                command
            )))
        }
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pty_manager_creation() {
        let manager = PtyManager::new();
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn test_command_validation() {
        // Test valid command
        assert!(PtyManager::validate_command("echo").is_ok());

        // Test invalid command
        assert!(PtyManager::validate_command("/nonexistent/command").is_err());
    }

    #[tokio::test]
    async fn test_pty_handle_creation() {
        let handle = PtyHandle::new();
        assert!(!handle.id.is_empty());
        assert!(handle.pid.is_none());
    }

    #[test]
    fn test_pty_info_creation() {
        let handle = PtyHandle::new();
        let info = PtyInfo {
            id: handle.id.clone(),
            pid: Some(12345),
            command: "echo".to_string(),
            working_directory: std::path::PathBuf::from("/tmp"),
            start_time: Utc::now(),
            is_alive: true,
        };

        assert_eq!(info.id, handle.id);
        assert_eq!(info.pid, Some(12345));
        assert_eq!(info.command, "echo");
        assert!(info.is_alive);
    }

    #[tokio::test]
    async fn test_manager_cleanup() {
        let mut manager = PtyManager::new();

        // Add a mock process (in real implementation this would be a running process)
        let handle = PtyHandle::new();
        let mut process = PtyProcess::new("test".to_string(), vec![]);
        process.mark_terminated(0);

        manager.active_processes.insert(handle.id.clone(), process);

        assert_eq!(manager.active_count(), 1);

        // Cleanup should remove terminated processes
        manager.cleanup_terminated();
        assert_eq!(manager.active_count(), 0);
    }
}
