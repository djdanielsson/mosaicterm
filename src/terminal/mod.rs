//! Terminal Emulation Core
//!
//! Main terminal emulator that integrates all terminal components
//! for processing commands and managing terminal state.

pub mod ansi_parser;
pub mod input;
pub mod output;
pub mod prompt;
pub mod state;

// Re-exports for convenience
pub use ansi_parser::{AnsiAttribute, AnsiColor, AnsiParser, ParsedText};
pub use input::{validation, CommandInputProcessor, InputResult};
pub use output::{segmentation, BufferStats, OutputChunk, OutputProcessor, StreamType};
pub use prompt::{utils as prompt_utils, CommandCompletionDetector, PromptDetector};
pub use state::{
    BufferLine, Cursor, ScreenBuffer, TerminalDimensions, TerminalMode, TerminalState,
    TerminalStatus,
};

use crate::error::Result;
use crate::models::{OutputLine, ShellType, TerminalSession};
use crate::pty::{PtyHandle, PtyManagerV2};
use chrono::Utc;
use std::sync::Arc;

/// Main terminal emulator
pub struct Terminal {
    /// Terminal state
    state: TerminalState,
    /// Input processor
    input_processor: CommandInputProcessor,
    /// Output processor
    output_processor: OutputProcessor,
    /// Prompt detector
    prompt_detector: PromptDetector,
    /// Completion detector
    completion_detector: CommandCompletionDetector,
    /// PTY manager reference (V2 with per-terminal locking)
    pty_manager: Arc<PtyManagerV2>,
    /// Current PTY handle
    pty_handle: Option<PtyHandle>,
}

impl Terminal {
    /// Create a new terminal emulator
    pub fn new(session: TerminalSession, pty_manager: Arc<PtyManagerV2>) -> Self {
        Self {
            state: TerminalState::new(session),
            input_processor: CommandInputProcessor::new(),
            output_processor: OutputProcessor::new(),
            prompt_detector: PromptDetector::new(),
            completion_detector: CommandCompletionDetector::new(),
            pty_manager,
            pty_handle: None,
        }
    }

    /// Create terminal with specific shell type
    ///
    /// # Arguments
    ///
    /// * `session` - Terminal session configuration
    /// * `shell_type` - Type of shell (Bash, Zsh, Fish, etc.)
    /// * `pty_manager` - Shared PTY manager for process coordination
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mosaicterm::terminal::Terminal;
    /// use mosaicterm::models::ShellType;
    /// use mosaicterm::pty::PtyManager;
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: TerminalSession is an internal type, create terminal directly
    /// let pty_manager = Arc::new(Mutex::new(PtyManager::new()));
    /// // Terminal is typically created internally by MosaicTermApp
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_shell(
        session: TerminalSession,
        shell_type: ShellType,
        pty_manager: Arc<PtyManagerV2>,
    ) -> Self {
        let mut terminal = Self::new(session, pty_manager);
        terminal.prompt_detector = PromptDetector::with_shell(shell_type);
        terminal
    }

    /// Initialize terminal session with PTY
    ///
    /// Spawns a new shell process in a pseudoterminal and prepares it for command execution.
    /// The shell is started with minimal configuration to ensure predictable behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - PTY creation fails
    /// - Shell process cannot be spawned
    /// - Initial shell setup fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use mosaicterm::terminal::Terminal;
    /// use mosaicterm::pty::PtyManagerV2;
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: Terminal initialization is handled internally by MosaicTermApp
    /// let pty_manager = Arc::new(PtyManagerV2::new());
    /// // Terminal sessions are managed automatically
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize_session(&mut self) -> Result<()> {
        // PtyManagerV2 is already async and thread-safe, no lock needed
        let pty_manager = &*self.pty_manager;

        // Create PTY process for the terminal session
        let session = &self.state.session;

        // Determine shell command and args based on shell type
        // Allow RC files to load (enables venv, nvm, conda, direnv, etc.)
        // but disable line editor features to prevent interactive behaviors
        let (shell_command, shell_args) = match session.shell_type {
            crate::models::ShellType::Bash => (
                "bash".to_string(),
                vec![
                    // Remove --norc and --noprofile to allow RC files to load
                    // This enables venv, nvm, conda, and other environment tools
                    "--noediting".to_string(), // Keep to prevent line editor interference
                ],
            ),
            crate::models::ShellType::Zsh => (
                "zsh".to_string(),
                vec![
                    // Remove -f flag to allow .zshrc to load
                    "+Z".to_string(), // Keep +Z to disable ZLE (line editor)
                ],
            ),
            crate::models::ShellType::Fish => (
                "fish".to_string(),
                vec![], // Allow config loading for fish
            ),
            crate::models::ShellType::Ksh => ("ksh".to_string(), vec![]),
            crate::models::ShellType::Csh => ("csh".to_string(), vec![]),
            crate::models::ShellType::Tcsh => ("tcsh".to_string(), vec![]),
            crate::models::ShellType::Dash => ("dash".to_string(), vec![]),
            crate::models::ShellType::PowerShell => (
                "powershell".to_string(),
                vec![], // Allow profile loading for PowerShell
            ),
            crate::models::ShellType::Cmd => ("cmd".to_string(), vec![]),
            crate::models::ShellType::Other => ("sh".to_string(), vec![]), // Fallback to basic shell
        };

        let handle = pty_manager
            .create_pty(
                &shell_command,
                &shell_args,
                &session.environment,
                Some(session.working_directory.as_path()),
            )
            .await?;

        // Store PTY handle and log
        let handle_id = handle.id.clone();
        self.pty_handle = Some(handle);

        info!(
            "Terminal session initialized with PTY handle: {}",
            handle_id
        );
        Ok(())
    }

    /// Initialize terminal with existing PTY handle
    pub fn initialize_with_pty(&mut self, handle: PtyHandle) -> Result<()> {
        let handle_id = handle.id.clone();
        self.pty_handle = Some(handle);
        info!(
            "Terminal initialized with existing PTY handle: {}",
            handle_id
        );
        Ok(())
    }

    /// Check if terminal session is initialized
    pub fn is_initialized(&self) -> bool {
        self.pty_handle.is_some()
    }

    /// Get PTY handle if available
    pub fn pty_handle(&self) -> Option<&PtyHandle> {
        self.pty_handle.as_ref()
    }

    /// Process command input
    pub async fn process_input(&mut self, input: &str) -> Result<InputResult> {
        // Process each character
        for ch in input.chars() {
            let result = self.input_processor.process_char(ch);

            match result {
                InputResult::CommandReady(command) => {
                    return self.execute_command(&command).await;
                }
                InputResult::EscapeSequence => {
                    // Handle escape sequences if more input is available
                    // For now, just continue processing
                    continue;
                }
                _ => {
                    // Update terminal state with current command
                    self.state
                        .set_current_command(self.input_processor.current_command().to_string());
                }
            }
        }

        Ok(InputResult::NoOp)
    }

    /// Process escape sequence
    pub fn process_escape_sequence(&mut self, sequence: &str) -> InputResult {
        self.input_processor.process_escape_sequence(sequence)
    }

    /// Execute a command
    async fn execute_command(&mut self, command: &str) -> Result<InputResult> {
        // Validate command
        input::validation::validate_command(command)?;

        // Create command block
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let command_block = self
            .input_processor
            .create_command_block(command, &working_dir);

        // Add to history
        self.state.add_command_to_history(command_block.clone());

        // Send to PTY if available
        if let Some(handle) = &self.pty_handle {
            // PtyManagerV2 is already async and thread-safe, no lock needed
            let manager = &*self.pty_manager;
            self.input_processor
                .send_command(manager, handle, command)
                .await?;
        }

        Ok(InputResult::CommandReady(command.to_string()))
    }

    /// Process output from PTY
    pub async fn process_output(
        &mut self,
        data: &[u8],
        stream_type: StreamType,
    ) -> Result<Vec<OutputLine>> {
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type,
            is_complete: false, // Will be determined by prompt detection
        };

        let lines = self.output_processor.process_chunk(chunk)?;

        // Check for command completion
        if self.completion_detector.is_command_complete(&lines) {
            // Mark last command as completed
            if let Some(_command) = self.state.get_last_command() {
                // In a real implementation, you'd update the command status
                // based on exit codes and output analysis
            }

            // Flush remaining lines
            let mut all_lines = lines;
            all_lines.extend(self.output_processor.flush_lines());
            return Ok(all_lines);
        }

        Ok(lines)
    }

    /// Set PTY handle
    pub fn set_pty_handle(&mut self, handle: PtyHandle) {
        self.pty_handle = Some(handle);
    }

    /// Get current terminal state
    pub fn state(&self) -> &TerminalState {
        &self.state
    }

    /// Get mutable terminal state
    pub fn state_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }

    /// Get current command being typed
    pub fn current_command(&self) -> &str {
        self.input_processor.current_command()
    }

    /// Get command history
    pub fn command_history(&self) -> &[String] {
        self.input_processor.history()
    }

    /// Clear terminal
    pub fn clear(&mut self) {
        self.state.reset();
        self.output_processor.clear();
        self.input_processor.clear_current_command();
    }

    /// Get terminal status
    pub fn status(&self) -> TerminalStatus {
        self.state.status()
    }

    /// Check if terminal has pending output
    pub fn has_pending_output(&self) -> bool {
        self.output_processor.has_pending_lines()
    }

    /// Get pending output lines
    pub fn pending_output_lines(&self) -> usize {
        self.output_processor.processed_line_count()
    }

    /// Flush all pending output
    pub fn flush_output(&mut self) -> Vec<OutputLine> {
        self.output_processor.flush_lines()
    }

    /// Take only ready (newline-terminated) output lines without flushing partial line
    pub fn take_ready_output_lines(&mut self) -> Vec<OutputLine> {
        self.output_processor.take_ready_lines()
    }

    /// Resize terminal
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.state.set_dimensions(rows, cols);
    }

    /// Get detected shell type
    pub fn detected_shell(&self) -> ShellType {
        self.prompt_detector.current_shell()
    }

    /// Set custom prompt pattern
    pub fn set_custom_prompt_pattern(&mut self, pattern: &str) -> Result<()> {
        self.prompt_detector.add_custom_pattern(pattern)
    }

    /// Add custom completion pattern
    pub fn add_completion_pattern(&mut self, pattern: &str) -> Result<()> {
        self.completion_detector.add_completion_pattern(pattern)
    }

    /// Get buffer statistics
    pub fn buffer_stats(&self) -> BufferStats {
        self.output_processor.buffer_stats()
    }

    /// Read available output from the PTY
    pub async fn read_output(&mut self) -> Result<Vec<OutputLine>> {
        if let Some(pty_handle) = &self.pty_handle {
            // PtyManagerV2 is already async and thread-safe, no lock needed
            let manager = &*self.pty_manager;

            // Read raw output from PTY
            let raw_output = manager.read_output(pty_handle, 100).await?;

            if raw_output.is_empty() {
                return Ok(Vec::new());
            }

            // Process the raw output through the output processor
            let chunk = OutputChunk {
                data: raw_output,
                timestamp: chrono::Utc::now(),
                stream_type: crate::terminal::output::StreamType::Stdout,
                is_complete: false,
            };

            // Process the chunk and return the lines
            self.output_processor.process_chunk(chunk)
        } else {
            Err(crate::error::Error::NoPtyHandleAvailable)
        }
    }

    /// Check if there's pending PTY output to read
    pub async fn has_pending_pty_output(&mut self) -> Result<bool> {
        if let Some(pty_handle) = &self.pty_handle {
            // PtyManagerV2 is already async and thread-safe, no lock needed
            let manager = &*self.pty_manager;
            let output = manager.read_output(pty_handle, 10).await?;
            Ok(!output.is_empty())
        } else {
            Ok(false)
        }
    }

    /// Get the current working directory for this terminal session
    pub fn get_working_directory(&self) -> &std::path::Path {
        &self.state.session.working_directory
    }

    /// Set the current working directory for this terminal session
    pub fn set_working_directory(&mut self, path: std::path::PathBuf) {
        self.state.session.working_directory = path;
    }
}

/// Terminal factory for creating terminals with different configurations
#[derive(Clone)]
pub struct TerminalFactory {
    pty_manager: Arc<PtyManagerV2>,
}

impl TerminalFactory {
    /// Create a new terminal factory
    pub fn new(pty_manager: Arc<PtyManagerV2>) -> Self {
        Self { pty_manager }
    }

    /// Create a terminal with auto-detected shell
    pub fn create_auto(&self, session: TerminalSession) -> Terminal {
        Terminal::new(session, self.pty_manager.clone())
    }

    /// Create a terminal with specific shell
    pub fn create_with_shell(&self, session: TerminalSession, shell_type: ShellType) -> Terminal {
        Terminal::with_shell(session, shell_type, self.pty_manager.clone())
    }

    /// Create a bash terminal
    pub fn create_bash(&self, session: TerminalSession) -> Terminal {
        self.create_with_shell(session, ShellType::Bash)
    }

    /// Create a zsh terminal
    pub fn create_zsh(&self, session: TerminalSession) -> Terminal {
        self.create_with_shell(session, ShellType::Zsh)
    }

    /// Create a fish terminal
    pub fn create_fish(&self, session: TerminalSession) -> Terminal {
        self.create_with_shell(session, ShellType::Fish)
    }

    /// Create and initialize a terminal session
    pub async fn create_and_initialize(&self, session: TerminalSession) -> Result<Terminal> {
        let mut terminal = self.create_auto(session);
        terminal.initialize_session().await?;
        Ok(terminal)
    }

    /// Create and initialize a terminal with specific shell
    pub async fn create_and_initialize_with_shell(
        &self,
        session: TerminalSession,
        shell_type: ShellType,
    ) -> Result<Terminal> {
        let mut terminal = self.create_with_shell(session, shell_type);
        terminal.initialize_session().await?;
        Ok(terminal)
    }

    /// Create and initialize a bash terminal
    pub async fn create_and_initialize_bash(&self, session: TerminalSession) -> Result<Terminal> {
        self.create_and_initialize_with_shell(session, ShellType::Bash)
            .await
    }

    /// Create and initialize a zsh terminal
    pub async fn create_and_initialize_zsh(&self, session: TerminalSession) -> Result<Terminal> {
        self.create_and_initialize_with_shell(session, ShellType::Zsh)
            .await
    }

    /// Create and initialize a fish terminal
    pub async fn create_and_initialize_fish(&self, session: TerminalSession) -> Result<Terminal> {
        self.create_and_initialize_with_shell(session, ShellType::Fish)
            .await
    }
}

/// Terminal utilities
pub mod utils {
    use super::*;

    /// Create a default terminal session
    pub fn create_default_session() -> TerminalSession {
        use crate::models::ShellType;

        let shell_path = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let shell_type = match shell_path.as_str() {
            "/bin/bash" | "/usr/bin/bash" => ShellType::Bash,
            "/bin/zsh" | "/usr/bin/zsh" => ShellType::Zsh,
            "/bin/fish" | "/usr/bin/fish" => ShellType::Fish,
            _ => ShellType::Other,
        };

        TerminalSession::new(shell_type, std::path::PathBuf::from(shell_path))
    }

    /// Check if terminal is ready for input
    pub fn is_terminal_ready(terminal: &Terminal) -> bool {
        terminal.pty_handle.is_some() && !terminal.has_pending_output()
    }

    /// Get terminal info as string
    pub fn terminal_info(terminal: &Terminal) -> String {
        let status = terminal.status();
        let shell_name = prompt_utils::shell_type_name(terminal.detected_shell());

        format!(
            "Terminal: {}x{}, Mode: {:?}, Shell: {}, Cursor: ({}, {}), Pending: {}",
            status.cursor_position.1,
            status.cursor_position.0,
            status.mode,
            shell_name,
            status.cursor_position.0,
            status.cursor_position.1,
            status.pending_output
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::PtyManagerV2;

    fn create_test_session() -> TerminalSession {
        TerminalSession::new(
            crate::TerminalShellType::Bash,
            std::path::PathBuf::from("/bin/bash"),
        )
    }

    fn create_test_pty_manager() -> Arc<PtyManagerV2> {
        Arc::new(PtyManagerV2::new())
    }

    #[test]
    fn test_terminal_creation() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let terminal = Terminal::new(session, pty_manager);

        assert_eq!(terminal.current_command(), "");
        assert_eq!(terminal.command_history().len(), 0);
        assert!(!terminal.has_pending_output());
    }

    #[test]
    fn test_terminal_with_shell() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let terminal = Terminal::with_shell(session, ShellType::Zsh, pty_manager);

        assert_eq!(terminal.detected_shell(), ShellType::Zsh);
    }

    #[test]
    fn test_terminal_factory() {
        let pty_manager = create_test_pty_manager();
        let factory = TerminalFactory::new(pty_manager);

        let session = create_test_session();
        let terminal = factory.create_bash(session);

        assert_eq!(terminal.detected_shell(), ShellType::Bash);
    }

    #[tokio::test]
    async fn test_process_simple_input() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let mut terminal = Terminal::new(session, pty_manager);

        let result = terminal.process_input("echo hello").await.unwrap();

        // Should not be ready yet (no newline)
        assert!(
            matches!(result, InputResult::NoOp),
            "Expected NoOp for incomplete command, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_process_command_with_newline() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let mut terminal = Terminal::new(session, pty_manager);

        let result = terminal.process_input("echo hello\n").await.unwrap();

        match result {
            InputResult::CommandReady(cmd) => assert_eq!(cmd, "echo hello"),
            other => panic!("Expected CommandReady, got {:?}", other),
        }

        assert_eq!(terminal.command_history().len(), 1);
    }

    #[test]
    fn test_terminal_clear() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let mut terminal = Terminal::new(session, pty_manager);

        // Set some state
        terminal
            .input_processor
            .set_current_command("test".to_string());

        terminal.clear();

        assert_eq!(terminal.current_command(), "");
    }

    #[test]
    fn test_terminal_resize() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let mut terminal = Terminal::new(session, pty_manager);

        terminal.resize(50, 120);

        // Check dimensions were updated
        assert_eq!(terminal.state.dimensions.rows, 50);
        assert_eq!(terminal.state.dimensions.cols, 120);
        // Cursor position should remain unchanged
        assert_eq!(terminal.state.cursor.row, 0);
        assert_eq!(terminal.state.cursor.col, 0);
    }

    #[test]
    fn test_utils_create_default_session() {
        let session = utils::create_default_session();
        assert!(!session.id.is_empty());
    }

    #[test]
    fn test_utils_terminal_info() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let terminal = Terminal::new(session, pty_manager);

        let info = utils::terminal_info(&terminal);
        assert!(info.contains("Terminal:"));
        assert!(info.contains("Mode:"));
        assert!(info.contains("Shell:"));
    }

    #[test]
    fn test_terminal_status() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let terminal = Terminal::new(session, pty_manager);

        let status = terminal.status();
        assert_eq!(status.mode, TerminalMode::Normal);
        assert_eq!(status.cursor_position, (0, 0));
        assert_eq!(status.pending_output, 0);
    }

    #[test]
    fn test_buffer_stats() {
        let session = create_test_session();
        let pty_manager = create_test_pty_manager();
        let terminal = Terminal::new(session, pty_manager);

        let stats = terminal.buffer_stats();
        assert_eq!(stats.raw_buffer_size, 0);
        assert_eq!(stats.processed_lines, 0);
        assert_eq!(stats.current_line_length, 0);
        assert_eq!(stats.ansi_codes_count, 0);
    }
}
