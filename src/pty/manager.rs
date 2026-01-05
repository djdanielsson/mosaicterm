//! PTY Manager
//!
//! Improved PTY management with fine-grained locking for better concurrency.
//! Each PTY process has its own lock, allowing multiple terminals to operate
//! independently without blocking each other.
//!
//! ## Event-Driven Mode
//!
//! The manager can optionally publish events to a `PtyEventBus` for
//! event-driven output handling. This eliminates the need for polling.
//!
//! ```ignore
//! use mosaicterm::pty::{PtyManager, PtyEventBus};
//!
//! let event_bus = PtyEventBus::new(256);
//! let manager = PtyManager::with_event_bus(event_bus.clone());
//!
//! // Subscribe to events
//! let mut sub = event_bus.subscribe().await;
//!
//! // Events will be published when output is available
//! ```

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::events::{PtyEvent, PtyEventBus};
use super::process::{spawn_pty_process, validate_command};
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

/// A single PTY entry with its own lock
struct PtyEntry {
    process: PtyProcess,
    streams: PtyStreams,
}

/// Main PTY manager with per-terminal locks for better concurrency
pub struct PtyManager {
    /// Active PTY processes with individual locks
    /// Using RwLock allows multiple readers or one writer
    terminals: Arc<RwLock<HashMap<String, Arc<RwLock<PtyEntry>>>>>,
    /// Optional event bus for event-driven output handling
    event_bus: Option<PtyEventBus>,
}

impl PtyManager {
    /// Create a new PTY manager
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
            event_bus: None,
        }
    }

    /// Create a new PTY manager with an event bus for event-driven output handling
    pub fn with_event_bus(event_bus: PtyEventBus) -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Some(event_bus),
        }
    }

    /// Get a reference to the event bus (if configured)
    pub fn event_bus(&self) -> Option<&PtyEventBus> {
        self.event_bus.as_ref()
    }

    /// Check if event-driven mode is enabled
    pub fn is_event_driven(&self) -> bool {
        self.event_bus.is_some()
    }

    /// Publish an event to the event bus (if configured)
    fn publish_event(&self, event: PtyEvent) {
        if let Some(bus) = &self.event_bus {
            bus.publish(event);
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
        validate_command(command)?;

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
        terminals.insert(handle_id.clone(), entry);

        // Publish creation event
        self.publish_event(PtyEvent::Created {
            handle_id,
            pid: handle.pid,
        });

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

        // Publish termination event
        self.publish_event(PtyEvent::Terminated {
            handle_id: handle.id.clone(),
        });

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
    ///
    /// Note: In event-driven mode, prefer subscribing to the event bus instead
    /// of polling with this method.
    pub async fn read_output(&self, handle: &PtyHandle, timeout_ms: u64) -> Result<Vec<u8>> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            let data = if timeout_ms == 0 {
                entry.streams.read().await?
            } else {
                entry.streams.read_with_timeout(timeout_ms).await?
            };

            // Publish output event if we have data and event bus is configured
            if !data.is_empty() {
                self.publish_event(PtyEvent::Output {
                    handle_id: handle.id.clone(),
                    data: data.clone(),
                });
            }

            Ok(data)
        } else {
            Err(Error::PtyStreamsNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Try to read output immediately without waiting
    /// This operation only locks the specific terminal, not all terminals
    ///
    /// Note: In event-driven mode, prefer subscribing to the event bus instead
    /// of polling with this method.
    pub async fn try_read_output_now(&self, handle: &PtyHandle) -> Result<Vec<u8>> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            let data = entry.streams.try_read_now()?;

            // Publish output event if we have data and event bus is configured
            if !data.is_empty() {
                self.publish_event(PtyEvent::Output {
                    handle_id: handle.id.clone(),
                    data: data.clone(),
                });
            }

            Ok(data)
        } else {
            Err(Error::PtyStreamsNotFound {
                handle_id: handle.id.to_string(),
            })
        }
    }

    /// Drain all pending output from the PTY channel (discard it)
    /// Used when switching contexts (e.g., ending SSH session) to avoid stale output
    pub async fn drain_output(&self, handle: &PtyHandle) -> Result<usize> {
        let terminals = self.terminals.read().await;
        if let Some(entry_lock) = terminals.get(&handle.id) {
            let mut entry = entry_lock.write().await;
            Ok(entry.streams.drain_output())
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
    async fn test_create_manager() {
        let manager = PtyManager::new();
        assert_eq!(manager.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        // This test would demonstrate that operations on different terminals
        // don't block each other (unlike the old single-lock design)
        let manager = Arc::new(PtyManager::new());

        // In a real test, we'd spawn multiple terminals and verify they
        // can operate concurrently without blocking each other
        assert_eq!(manager.active_count().await, 0);
    }
}
