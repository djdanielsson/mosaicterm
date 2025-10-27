//! Core data models for MosaicTerm
//!
//! This module contains all the core data structures that represent
//! the domain entities in MosaicTerm, including command blocks,
//! terminal sessions, PTY processes, and configuration.

pub mod command_block;
pub mod config;
pub mod output_line;
pub mod pty_process;
pub mod shell_type;
pub mod terminal_session;

// Re-exports for convenience
pub use command_block::{CommandBlock, ExecutionStatus};
pub use config::Config;
pub use output_line::OutputLine;
pub use pty_process::PtyProcess;
pub use shell_type::ShellType;
pub use terminal_session::TerminalSession;
