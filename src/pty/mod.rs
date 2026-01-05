//! Pseudoterminal (PTY) Management
//!
//! This module provides cross-platform pseudoterminal support for MosaicTerm,
//! handling process spawning, I/O streams, and signal management.
//!
//! ## PTY Manager
//!
//! Use `PtyManager` for all PTY operations. It provides per-terminal
//! locking and a fully async API.
//!
//! ## Event-Driven Architecture
//!
//! The `events` module provides an event-driven architecture for PTY output.
//! Instead of polling, subscribers receive events when output is available.
//!
//! ```ignore
//! use mosaicterm::pty::{PtyEventBus, PtyEvent};
//!
//! let event_bus = PtyEventBus::new(256);
//! let mut subscription = event_bus.subscribe().await;
//!
//! // Handle events
//! while let Some(event) = subscription.recv().await {
//!     match event {
//!         PtyEvent::Output { handle_id, data } => { /* process output */ }
//!         PtyEvent::ProcessExited { handle_id, exit_code } => { /* handle exit */ }
//!         _ => {}
//!     }
//! }
//! ```

pub mod events;
pub mod manager;
pub mod operations;
pub mod process;
pub mod process_tree;
pub mod signals;
pub mod streams;

// Re-exports for convenience
pub use events::{PtyEvent, PtyEventBus, PtyEventSubscription, PtyOutputWatcher, WatchHandle};
pub use manager::{PtyHandle, PtyInfo, PtyManager};
pub use operations::PtyOperations;
pub use process::{
    get_default_shell, get_user_shell, spawn_pty_process, validate_command, SpawnConfig,
};
pub use signals::{utils, Signal, SignalConfig, SignalHandler};
pub use streams::{PtyStreams, StreamConfig, StreamStats};
