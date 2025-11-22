//! Platform abstraction layer
//!
//! This module provides a unified interface for platform-specific operations,
//! abstracting away differences between Unix, Windows, and other platforms.

mod traits;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use traits::*;

/// Platform implementation factory
pub struct Platform;

impl Platform {
    /// Get the platform-specific signal operations
    pub fn signals() -> Box<dyn SignalOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixSignals::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsSignals::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }

    /// Get the platform-specific process tree operations
    pub fn process_tree() -> Box<dyn ProcessTreeOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixProcessTree::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsProcessTree::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }

    /// Get the platform-specific memory operations
    pub fn memory() -> Box<dyn MemoryOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixMemory::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsMemory::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }

    /// Get the platform-specific filesystem operations
    pub fn filesystem() -> Box<dyn FilesystemOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixFilesystem::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsFilesystem::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }

    /// Get the platform-specific path operations
    pub fn paths() -> Box<dyn PathOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixPaths::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsPaths::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }

    /// Get the platform-specific shell operations
    pub fn shell() -> Box<dyn ShellOps> {
        #[cfg(unix)]
        {
            Box::new(unix::UnixShell::new())
        }

        #[cfg(windows)]
        {
            Box::new(windows::WindowsShell::new())
        }

        #[cfg(not(any(unix, windows)))]
        {
            compile_error!("Unsupported platform");
        }
    }
}
