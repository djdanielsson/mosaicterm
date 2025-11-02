//! Test Utilities and Mocks
//!
//! This module provides comprehensive test utilities and mock implementations
//! for testing MosaicTerm components.

pub mod mock_pty;
pub mod mock_terminal;
pub mod fixtures;

// Re-exports for convenience
pub use mock_pty::{MockPtyManager, MockPtyProcess};
pub use mock_terminal::{MockTerminal, MockTerminalFactory};
pub use fixtures::{create_test_config, create_test_command_block, create_test_output_line};

