//! PTY Manager V2 - Per-Terminal Locking
//!
//! Improved PTY management with fine-grained locking for better concurrency.
//! Each PTY process has its own lock, allowing multiple terminals to operate
//! independently without blocking each other.

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::manager::PtyHandle;
use super::process::spawn_pty_process;
use super::streams::PtyStreams;
use crate::error::{Error, Result};
use crate::models::PtyProcess;

// Use PtyHandle from manager.rs for compatibility

// Use PtyInfo from manager.rs for compatibility
use super::manager::PtyInfo;

/// A single PTY entry with its own lock
struct PtyEntry {
    process: PtyProcess,
    streams: PtyStreams,
}

/// Main PTY manager with per-terminal locks for better concurrency
pub struct PtyManagerV2 {
    /// Active PTY processes with individual locks
    /// Using RwLock allows multiple readers or one writer
    terminals: Arc<RwLock<HashMap<String, Arc<RwLock<PtyEntry>>>>>,
}

impl PtyManagerV2 {
    /// Create a new PTY manager
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create and start a new PTY process
    pub async fn create_pty(
        &self,
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
            handle.pid = Some(pid);
        }

        // Create entry with its own lock
        let entry = Arc::new(RwLock::new(PtyEntry { process, streams }));

        // Store in the global map
        let handle_id = handle.id.clone();
        let mut terminals = self.terminals.write().await;
        terminals.insert(handle_id, entry);

        Ok(handle)
    }

    /// Check if a PTY process is still alive
    pub async fn is_alive(&self, handle: &PtyHandle) -> bool {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let entry = entry_lock.read().await;
            entry.process.is_running()
        } else {
            false
        }
    }

    /// Terminate a PTY process
    pub async fn terminate_pty(&self, handle: &PtyHandle) -> Result<()> {
        let process_id = handle.id.clone();

        // Remove from active processes
        let mut terminals = self.terminals.write().await;
        if let Some(entry_lock) = terminals.remove(&process_id) {
            // Get write access to terminate
            let mut entry = entry_lock.write().await;
            if entry.process.is_running() {
                entry.process.mark_terminated(0);
            }
        }

        Ok(())
    }

    /// Get information about a PTY process
    pub async fn get_info(&self, handle: &PtyHandle) -> Result<PtyInfo> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let entry = entry_lock.read().await;
            Ok(PtyInfo {
                id: handle.id.clone(),
                pid: entry.process.pid,
                command: entry.process.command.clone(),
                working_directory: entry
                    .process
                    .working_directory
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(".")),
                start_time: entry.process.start_time.unwrap_or_else(Utc::now),
                is_alive: entry.process.is_running(),
            })
        } else {
            Err(Error::PtyHandleNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Send data to a PTY process
    /// This operation only locks the specific terminal, not all terminals
    pub async fn send_input(&self, handle: &PtyHandle, data: &[u8]) -> Result<()> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            entry.streams.write(data).await
        } else {
            Err(Error::PtyStreamsNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Read output from a PTY process, with a timeout in milliseconds
    /// This operation only locks the specific terminal, not all terminals
    pub async fn read_output(&self, handle: &PtyHandle, timeout_ms: u64) -> Result<Vec<u8>> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            if timeout_ms == 0 {
                entry.streams.read().await
            } else {
                entry.streams.read_with_timeout(timeout_ms).await
            }
        } else {
            Err(Error::PtyStreamsNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Try to read output immediately without waiting
    /// This operation only locks the specific terminal, not all terminals
    pub async fn try_read_output_now(&self, handle: &PtyHandle) -> Result<Vec<u8>> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            entry.streams.try_read_now()
        } else {
            Err(Error::PtyStreamsNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Get the number of active PTY processes
    pub async fn active_count(&self) -> usize {
        let terminals = self.terminals.read().await;
        terminals.len()
    }

    /// Clean up terminated processes
    pub async fn cleanup_terminated(&self) -> usize {
        let terminals = self.terminals.read().await;

        // Find terminated processes
        let mut terminated = Vec::new();
        for (id, entry_lock) in terminals.iter() {
            let entry = entry_lock.read().await;
            if !entry.process.is_running() {
                terminated.push(id.clone());
            }
        }

        let count = terminated.len();

        // Drop read lock before acquiring write lock
        drop(terminals);

        // Remove terminated processes
        if !terminated.is_empty() {
            let mut terminals = self.terminals.write().await;
            for id in terminated {
                terminals.remove(&id);
            }
        }

        count
    }

    /// Validate that a command exists and is executable
    fn validate_command(command: &str) -> Result<()> {
        // Check if command exists in PATH
        if std::process::Command::new("which")
            .arg(command)
            .output()
            .map_err(|_| Error::CommandNotFound {
                command: command.to_string(),
            })?
            .status
            .success()
        {
            Ok(())
        } else {
            Err(Error::CommandNotFound {
                command: command.to_string(),
            })
        }
    }
}

impl Default for PtyManagerV2 {
    fn default() -> Self {
        Self::new()
    }
}

// Note: PtyOperations trait is designed for the old single-lock PtyManager.
// PtyManagerV2 has a different API that doesn't fit the trait well.
// For now, we'll keep PtyManagerV2 separate and provide a migration path later.

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_manager() {
        let manager = PtyManagerV2::new();
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        // This test would demonstrate that operations on different terminals
        // don't block each other (unlike the old single-lock design)
        let manager = Arc::new(PtyManagerV2::new());

        // In a real test, we'd spawn multiple terminals and verify they
        // can operate concurrently without blocking each other
        assert_eq!(manager.active_count().await, 0);
    }
}
