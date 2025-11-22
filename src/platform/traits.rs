//! Platform-specific operation traits
//!
//! These traits define the interface for platform-specific operations,
//! allowing for clean abstraction and easier testing.

use crate::error::Result;
use std::path::PathBuf;

/// Platform-specific signal operations
#[async_trait::async_trait]
pub trait SignalOps: Send + Sync {
    /// Send an interrupt signal (Ctrl+C equivalent)
    async fn send_interrupt(&self, pid: u32) -> Result<()>;

    /// Send a termination signal (graceful shutdown)
    async fn send_terminate(&self, pid: u32) -> Result<()>;

    /// Send a kill signal (forceful termination)
    async fn send_kill(&self, pid: u32) -> Result<()>;

    /// Check if a process is still running
    fn is_process_running(&self, pid: u32) -> bool;
}

/// Platform-specific process tree operations
pub trait ProcessTreeOps: Send + Sync {
    /// Get all child process IDs of a given parent PID
    fn get_child_pids(&self, parent_pid: u32) -> Result<Vec<u32>>;

    /// Kill a process and all its descendants
    fn kill_process_tree(&self, root_pid: u32) -> Result<()>;
}

/// Platform-specific memory operations
pub trait MemoryOps: Send + Sync {
    /// Get current memory usage in bytes
    fn get_current_memory(&self) -> Result<usize>;

    /// Get peak memory usage in bytes
    fn get_peak_memory(&self) -> Result<usize>;
}

/// Platform-specific filesystem operations
pub trait FilesystemOps: Send + Sync {
    /// Check if a file is executable
    fn is_executable(&self, path: &std::path::Path) -> bool;

    /// Find a command in PATH
    fn find_command(&self, command: &str) -> Result<Option<PathBuf>>;
}

/// Platform-specific path operations
pub trait PathOps: Send + Sync {
    /// Get configuration directory
    fn config_dir(&self) -> Result<PathBuf>;

    /// Get data directory
    fn data_dir(&self) -> Result<PathBuf>;

    /// Get cache directory
    fn cache_dir(&self) -> Result<PathBuf>;
}

/// Platform-specific shell operations
pub trait ShellOps: Send + Sync {
    /// Get default shell path
    fn default_shell(&self) -> PathBuf;

    /// Detect available shells
    fn detect_shells(&self) -> Vec<(String, PathBuf)>;
}
