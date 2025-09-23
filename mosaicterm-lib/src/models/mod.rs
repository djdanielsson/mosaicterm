//! Core data models for MosaicTerm
//!
//! This module contains all the core data structures that represent
//! the domain entities in MosaicTerm, including command blocks,
//! terminal sessions, PTY processes, and configuration.

pub mod command_block;
pub mod terminal_session;
pub mod pty_process;
pub mod output_line;
pub mod config;

// Re-exports for convenience
pub use command_block::{CommandBlock, ExecutionStatus};
pub use terminal_session::{TerminalSession, ShellType};
pub use pty_process::PtyProcess;
pub use output_line::OutputLine;
pub use config::Config;
