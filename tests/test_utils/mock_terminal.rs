//! Mock Terminal Implementation for Testing

use mosaicterm::error::Result;
use mosaicterm::models::OutputLine;
use mosaicterm::pty::PtyHandle;
use std::path::PathBuf;

/// Mock Terminal for testing
pub struct MockTerminal {
    pub pty_handle: Option<PtyHandle>,
    pub output_lines: Vec<OutputLine>,
    pub working_directory: PathBuf,
    pub command_history: Vec<String>,
}

impl MockTerminal {
    /// Create a new mock terminal
    pub fn new() -> Self {
        Self {
            pty_handle: None,
            output_lines: Vec::new(),
            working_directory: PathBuf::from("/tmp"),
            command_history: Vec::new(),
        }
    }

    /// Set the PTY handle
    pub fn set_pty_handle(&mut self, handle: PtyHandle) {
        self.pty_handle = Some(handle);
    }

    /// Get the PTY handle
    pub fn pty_handle(&self) -> Option<&PtyHandle> {
        self.pty_handle.as_ref()
    }

    /// Add output line
    pub fn add_output_line(&mut self, line: OutputLine) {
        self.output_lines.push(line);
    }

    /// Get all output lines
    pub fn get_output_lines(&self) -> &[OutputLine] {
        &self.output_lines
    }

    /// Clear output
    pub fn clear_output(&mut self) {
        self.output_lines.clear();
    }

    /// Add command to history
    pub fn add_to_history(&mut self, command: String) {
        self.command_history.push(command);
    }

    /// Get command history
    pub fn get_history(&self) -> &[String] {
        &self.command_history
    }

    /// Set working directory
    pub fn set_working_directory(&mut self, dir: PathBuf) {
        self.working_directory = dir;
    }

    /// Get working directory
    pub fn get_working_directory(&self) -> &PathBuf {
        &self.working_directory
    }
}

impl Default for MockTerminal {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock Terminal Factory for testing
pub struct MockTerminalFactory {
    terminals: Vec<MockTerminal>,
}

impl MockTerminalFactory {
    /// Create a new mock terminal factory
    pub fn new() -> Self {
        Self {
            terminals: Vec::new(),
        }
    }

    /// Create a new terminal
    pub fn create_terminal(&mut self) -> Result<MockTerminal> {
        let terminal = MockTerminal::new();
        self.terminals.push(terminal.clone());
        Ok(terminal)
    }

    /// Get count of created terminals
    pub fn terminal_count(&self) -> usize {
        self.terminals.len()
    }
}

impl Default for MockTerminalFactory {
    fn default() -> Self {
        Self::new()
    }
}

// Need to implement Clone for MockTerminal for the factory to work
impl Clone for MockTerminal {
    fn clone(&self) -> Self {
        Self {
            pty_handle: self.pty_handle.clone(),
            output_lines: Vec::new(), // Don't clone output for factory
            working_directory: self.working_directory.clone(),
            command_history: Vec::new(), // Don't clone history for factory
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_terminal_creation() {
        let terminal = MockTerminal::new();
        assert_eq!(terminal.output_lines.len(), 0);
        assert_eq!(terminal.command_history.len(), 0);
        assert!(terminal.pty_handle.is_none());
    }

    #[test]
    fn test_mock_terminal_output() {
        let mut terminal = MockTerminal::new();
        let line = OutputLine::with_line_number("test output", 1);

        terminal.add_output_line(line);
        assert_eq!(terminal.output_lines.len(), 1);
        assert_eq!(terminal.output_lines[0].text, "test output");
    }

    #[test]
    fn test_mock_terminal_history() {
        let mut terminal = MockTerminal::new();
        terminal.add_to_history("ls".to_string());
        terminal.add_to_history("pwd".to_string());

        assert_eq!(terminal.command_history.len(), 2);
        assert_eq!(terminal.command_history[0], "ls");
        assert_eq!(terminal.command_history[1], "pwd");
    }

    #[test]
    fn test_mock_terminal_factory() {
        let mut factory = MockTerminalFactory::new();

        let _terminal1 = factory.create_terminal().unwrap();
        let _terminal2 = factory.create_terminal().unwrap();

        assert_eq!(factory.terminal_count(), 2);
    }
}
