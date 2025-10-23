//! Terminal State Management
//!
//! Manages the overall state of the terminal emulator, including
//! current mode, cursor position, screen buffer, and terminal settings.

use chrono::{DateTime, Utc};
use crate::models::{TerminalSession, CommandBlock, OutputLine};
use crate::models::output_line::AnsiCode;

/// Terminal emulator state
#[derive(Debug)]
pub struct TerminalState {
    /// Current terminal session
    pub session: TerminalSession,
    /// Current cursor position
    pub cursor: Cursor,
    /// Terminal dimensions
    pub dimensions: TerminalDimensions,
    /// Current terminal mode
    pub mode: TerminalMode,
    /// Screen buffer state
    pub buffer: ScreenBuffer,
    /// Command history
    pub command_history: Vec<CommandBlock>,
    /// Current command being typed
    pub current_command: String,
    /// Output lines being accumulated
    pub pending_output: Vec<OutputLine>,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    /// Row position (0-based)
    pub row: usize,
    /// Column position (0-based)
    pub col: usize,
    /// Whether cursor is visible
    pub visible: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalDimensions {
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
    /// Character width in pixels
    pub char_width: usize,
    /// Character height in pixels
    pub char_height: usize,
}

impl Default for TerminalDimensions {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            char_width: 8,
            char_height: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalMode {
    /// Normal text input/output
    Normal,
    /// Escape sequence processing
    Escape,
    /// Control sequence processing
    ControlSequence,
    /// Application keypad mode
    ApplicationKeypad,
    /// Alternate screen buffer
    AlternateScreen,
}

impl Default for TerminalMode {
    fn default() -> Self {
        TerminalMode::Normal
    }
}

#[derive(Debug)]
pub struct ScreenBuffer {
    /// Lines of text in the buffer
    pub lines: Vec<BufferLine>,
    /// Scrollback history
    pub scrollback: Vec<BufferLine>,
    /// Maximum scrollback lines
    pub max_scrollback: usize,
    /// Current scroll position
    pub scroll_position: usize,
}

impl ScreenBuffer {
    /// Create a new screen buffer
    pub fn new(max_scrollback: usize) -> Self {
        Self {
            lines: Vec::new(),
            scrollback: Vec::new(),
            max_scrollback,
            scroll_position: 0,
        }
    }

    /// Add a line to the buffer
    pub fn add_line(&mut self, line: BufferLine) {
        self.lines.push(line);

        // If we exceed the screen height, move lines to scrollback
        while self.lines.len() > 24 { // Assuming 24 rows for now
            if let Some(line) = self.lines.first().cloned() {
                self.lines.remove(0);
                self.scrollback.push(line);
            } else {
                break;
            }
        }

        // Maintain scrollback limit
        while self.scrollback.len() > self.max_scrollback {
            self.scrollback.remove(0);
        }
    }

    /// Get total number of lines (screen + scrollback)
    pub fn total_lines(&self) -> usize {
        self.lines.len() + self.scrollback.len()
    }

    /// Get line at absolute position
    pub fn get_line(&self, index: usize) -> Option<&BufferLine> {
        if index < self.scrollback.len() {
            self.scrollback.get(index)
        } else {
            self.lines.get(index - self.scrollback.len())
        }
    }

    /// Clear the screen buffer
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_position = 0;
    }
}

impl Default for ScreenBuffer {
    fn default() -> Self {
        Self::new(100000) // Default 100k lines of scrollback for unlimited output
    }
}

#[derive(Debug, Clone)]
pub struct BufferLine {
    /// Text content of the line
    pub text: String,
    /// ANSI formatting codes
    pub ansi_codes: Vec<AnsiCode>,
    /// Line number in the buffer
    pub line_number: usize,
    /// Timestamp when line was created
    pub timestamp: DateTime<Utc>,
    /// Whether this line wraps to the next line
    pub wrapped: bool,
}

impl BufferLine {
    /// Create a new buffer line
    pub fn new(text: String, line_number: usize) -> Self {
        Self {
            text,
            ansi_codes: Vec::new(),
            line_number,
            timestamp: Utc::now(),
            wrapped: false,
        }
    }

    /// Add ANSI code to the line
    pub fn add_ansi_code(&mut self, code: AnsiCode) {
        self.ansi_codes.push(code);
    }

    /// Check if line has ANSI formatting
    pub fn has_formatting(&self) -> bool {
        !self.ansi_codes.is_empty()
    }
}

impl TerminalState {
    /// Create a new terminal state
    pub fn new(session: TerminalSession) -> Self {
        Self {
            session,
            cursor: Cursor::default(),
            dimensions: TerminalDimensions::default(),
            mode: TerminalMode::default(),
            buffer: ScreenBuffer::default(),
            command_history: Vec::new(),
            current_command: String::new(),
            pending_output: Vec::new(),
            last_activity: Utc::now(),
        }
    }

    /// Update cursor position
    pub fn set_cursor(&mut self, row: usize, col: usize) {
        self.cursor.row = row;
        self.cursor.col = col;
        self.update_activity();
    }

    /// Move cursor relatively
    pub fn move_cursor(&mut self, delta_row: isize, delta_col: isize) {
        let new_row = (self.cursor.row as isize + delta_row).max(0) as usize;
        let new_col = (self.cursor.col as isize + delta_col).max(0) as usize;

        self.cursor.row = new_row.min(self.dimensions.rows - 1);
        self.cursor.col = new_col.min(self.dimensions.cols - 1);
        self.update_activity();
    }

    /// Set terminal mode
    pub fn set_mode(&mut self, mode: TerminalMode) {
        self.mode = mode;
        self.update_activity();
    }

    /// Set terminal dimensions
    pub fn set_dimensions(&mut self, rows: usize, cols: usize) {
        self.dimensions.rows = rows;
        self.dimensions.cols = cols;
        self.update_activity();
    }

    /// Add output line to pending output
    pub fn add_output_line(&mut self, line: OutputLine) {
        self.pending_output.push(line);
        self.update_activity();
    }

    /// Clear pending output (after processing)
    pub fn clear_pending_output(&mut self) {
        self.pending_output.clear();
        self.update_activity();
    }

    /// Add command to history
    pub fn add_command_to_history(&mut self, command: CommandBlock) {
        self.command_history.push(command);
        self.update_activity();
    }

    /// Set current command being typed
    pub fn set_current_command(&mut self, command: String) {
        self.current_command = command;
        self.update_activity();
    }

    /// Get the last command from history
    pub fn get_last_command(&self) -> Option<&CommandBlock> {
        self.command_history.last()
    }

    /// Check if terminal has pending output
    pub fn has_pending_output(&self) -> bool {
        !self.pending_output.is_empty()
    }

    /// Get pending output count
    pub fn pending_output_count(&self) -> usize {
        self.pending_output.len()
    }

    /// Update last activity timestamp
    fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Reset terminal to initial state
    pub fn reset(&mut self) {
        self.cursor = Cursor::default();
        self.mode = TerminalMode::Normal;
        self.buffer.clear();
        self.current_command.clear();
        self.pending_output.clear();
        self.update_activity();
    }

    /// Get terminal status summary
    pub fn status(&self) -> TerminalStatus {
        TerminalStatus {
            mode: self.mode,
            cursor_position: (self.cursor.row, self.cursor.col),
            buffer_size: self.buffer.total_lines(),
            pending_output: self.pending_output_count(),
            last_activity: self.last_activity,
        }
    }
}

/// Terminal status summary
#[derive(Debug, Clone)]
pub struct TerminalStatus {
    /// Current terminal mode
    pub mode: TerminalMode,
    /// Cursor position (row, col)
    pub cursor_position: (usize, usize),
    /// Total buffer size
    pub buffer_size: usize,
    /// Number of pending output lines
    pub pending_output: usize,
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TerminalSession;

    #[test]
    fn test_terminal_state_creation() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let state = TerminalState::new(session);

        assert_eq!(state.cursor.row, 0);
        assert_eq!(state.cursor.col, 0);
        assert_eq!(state.dimensions.rows, 24);
        assert_eq!(state.dimensions.cols, 80);
        assert_eq!(state.mode, TerminalMode::Normal);
    }

    #[test]
    fn test_cursor_movement() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        state.set_cursor(5, 10);
        assert_eq!(state.cursor.row, 5);
        assert_eq!(state.cursor.col, 10);

        state.move_cursor(2, 3);
        assert_eq!(state.cursor.row, 7);
        assert_eq!(state.cursor.col, 13);
    }

    #[test]
    fn test_cursor_bounds() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        // Test upper bounds
        state.move_cursor(100, 100);
        assert_eq!(state.cursor.row, 23); // rows - 1
        assert_eq!(state.cursor.col, 79); // cols - 1

        // Test lower bounds
        state.move_cursor(-50, -50);
        assert_eq!(state.cursor.row, 0);
        assert_eq!(state.cursor.col, 0);
    }

    #[test]
    fn test_terminal_mode_changes() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        state.set_mode(TerminalMode::Escape);
        assert_eq!(state.mode, TerminalMode::Escape);

        state.set_mode(TerminalMode::ApplicationKeypad);
        assert_eq!(state.mode, TerminalMode::ApplicationKeypad);
    }

    #[test]
    fn test_screen_buffer_operations() {
        let mut buffer = ScreenBuffer::new(100);

        let line1 = BufferLine::new("Line 1".to_string(), 0);
        let line2 = BufferLine::new("Line 2".to_string(), 1);

        buffer.add_line(line1);
        buffer.add_line(line2);

        assert_eq!(buffer.lines.len(), 2);
        assert_eq!(buffer.total_lines(), 2);

        if let Some(line) = buffer.get_line(0) {
            assert_eq!(line.text, "Line 1");
        }
    }

    #[test]
    fn test_buffer_line_creation() {
        let line = BufferLine::new("Test line".to_string(), 5);

        assert_eq!(line.text, "Test line");
        assert_eq!(line.line_number, 5);
        assert!(!line.has_formatting());
        assert!(!line.wrapped);
    }

    #[test]
    fn test_terminal_dimensions() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        state.set_dimensions(30, 120);
        assert_eq!(state.dimensions.rows, 30);
        assert_eq!(state.dimensions.cols, 120);
    }

    #[test]
    fn test_pending_output_management() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        assert!(!state.has_pending_output());
        assert_eq!(state.pending_output_count(), 0);

        // Note: This would normally create actual OutputLine instances
        // For this test, we'll just check the methods work
        state.clear_pending_output();
        assert_eq!(state.pending_output_count(), 0);
    }

    #[test]
    fn test_terminal_status() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let state = TerminalState::new(session);
        let status = state.status();

        assert_eq!(status.mode, TerminalMode::Normal);
        assert_eq!(status.cursor_position, (0, 0));
        assert_eq!(status.pending_output, 0);
    }

    #[test]
    fn test_terminal_reset() {
        let session = TerminalSession::new(crate::TerminalShellType::Bash, std::path::PathBuf::from("/bin/bash"));
        let mut state = TerminalState::new(session);

        state.set_cursor(10, 20);
        state.set_mode(TerminalMode::Escape);
        state.set_current_command("test command".to_string());

        state.reset();

        assert_eq!(state.cursor.row, 0);
        assert_eq!(state.cursor.col, 0);
        assert_eq!(state.mode, TerminalMode::Normal);
        assert!(state.current_command.is_empty());
    }
}
