//! Centralized State Management
//!
//! This module provides a single source of truth for all application state,
//! eliminating duplication across `MosaicTermApp`, `Terminal`, and `TerminalSession`.
//!
//! ## Design Goals
//!
//! 1. **Single Source of Truth:** All state in one place
//! 2. **Clear Ownership:** Explicit state ownership and borrowing
//! 3. **Testability:** Easy to mock and test in isolation
//! 4. **Thread-Safe:** Can be safely shared across threads
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │      MosaicTermApp (UI)             │
//! │                                     │
//! │  ┌──────────────────────────────┐  │
//! │  │    StateManager              │  │
//! │  │  - command_history           │  │
//! │  │  - terminal_state            │  │
//! │  │  - pty_state                 │  │
//! │  │  - ui_state                  │  │
//! │  └──────────────────────────────┘  │
//! │                                     │
//! │  Terminal      PTY Manager          │
//! │  (stateless)   (processes only)     │
//! └─────────────────────────────────────┘
//! ```

use crate::models::{CommandBlock, OutputLine};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Centralized state manager - single source of truth for all application state
#[derive(Debug, Clone)]
pub struct StateManager {
    /// Active terminal sessions (keyed by session ID)
    sessions: HashMap<String, SessionState>,
    /// Currently active session ID
    active_session_id: Option<String>,
    /// Global application state
    app_state: ApplicationState,
    /// Application statistics and metrics
    statistics: AppStatistics,
}

/// State for a single terminal session
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Session identifier
    pub id: String,
    /// Command history for this session
    pub command_history: Vec<CommandBlock>,
    /// Current working directory
    pub working_directory: PathBuf,
    /// Previous working directory (for `cd -`)
    pub previous_directory: Option<PathBuf>,
    /// PTY handle ID (if active)
    pub pty_handle_id: Option<String>,
    /// Shell type
    pub shell_type: crate::models::ShellType,
    /// Session start time
    pub start_time: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Session status
    pub status: SessionStatus,
    /// Pending output lines (not yet added to a command block)
    pub pending_output: Vec<OutputLine>,
    /// Current command being typed
    pub current_input: String,
    /// Input history for up/down arrow navigation
    pub input_history: Vec<String>,
    /// Input history navigation index
    pub input_history_index: Option<usize>,
    /// Last command execution time (for timeout detection)
    pub last_command_time: Option<std::time::Instant>,
    /// Maximum history size
    pub max_history_size: usize,
}

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    /// Session is initializing
    Initializing,
    /// Session is active and ready
    Active,
    /// Session is terminating
    Terminating,
    /// Session has terminated
    Terminated,
}

/// Global application state (UI-related)
#[derive(Debug, Clone)]
pub struct ApplicationState {
    /// Terminal initialization status
    pub terminal_ready: bool,
    /// Whether initialization has been attempted
    pub initialization_attempted: bool,
    /// Status message to display
    pub status_message: Option<String>,
    /// Loading state
    pub is_loading: bool,
    /// Loading message
    pub loading_message: String,
    /// Loading animation frame
    pub loading_frame: usize,
    /// Last tab press time (for double-tab completion)
    pub last_tab_press: Option<std::time::Instant>,
    /// Whether completion was just applied
    pub completion_just_applied: bool,
    /// Error dialog to display (title, message)
    pub error_dialog: Option<ErrorDialog>,
}

/// Error dialog information
#[derive(Debug, Clone)]
pub struct ErrorDialog {
    /// Dialog title
    pub title: String,
    /// Error message
    pub message: String,
    /// Whether the error is critical (affects styling)
    pub is_critical: bool,
}

/// Application statistics and metrics
#[derive(Debug, Clone)]
pub struct AppStatistics {
    /// Application start time
    pub start_time: DateTime<Utc>,
    /// Total commands executed
    pub total_commands: usize,
    /// Total successful commands
    pub successful_commands: usize,
    /// Total failed commands
    pub failed_commands: usize,
    /// Total cancelled commands
    pub cancelled_commands: usize,
    /// Total output lines processed
    pub total_output_lines: usize,
    /// Peak memory usage (bytes)
    pub peak_memory_bytes: usize,
    /// Current memory usage (bytes)
    pub current_memory_bytes: usize,
}

impl Default for AppStatistics {
    fn default() -> Self {
        Self {
            start_time: Utc::now(),
            total_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
            cancelled_commands: 0,
            total_output_lines: 0,
            peak_memory_bytes: 0,
            current_memory_bytes: 0,
        }
    }
}

impl AppStatistics {
    /// Get uptime as a Duration
    pub fn uptime(&self) -> std::time::Duration {
        let seconds = (Utc::now() - self.start_time).num_seconds();
        std::time::Duration::from_secs(seconds.max(0) as u64)
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> i64 {
        (Utc::now() - self.start_time).num_seconds()
    }
    
    /// Get uptime as a formatted string
    pub fn uptime_formatted(&self) -> String {
        let seconds = self.uptime_seconds();
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;
        
        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, secs)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}s", secs)
        }
    }
    
    /// Update memory usage statistics
    pub fn update_memory(&mut self) {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            // Get memory usage on macOS using ps
            if let Ok(output) = Command::new("ps")
                .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
            {
                if let Ok(memory_str) = String::from_utf8(output.stdout) {
                    if let Ok(memory_kb) = memory_str.trim().parse::<usize>() {
                        self.current_memory_bytes = memory_kb * 1024;
                        if self.current_memory_bytes > self.peak_memory_bytes {
                            self.peak_memory_bytes = self.current_memory_bytes;
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "linux")]
        {
            // Read from /proc/self/status on Linux
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(value) = line.split_whitespace().nth(1) {
                            if let Ok(memory_kb) = value.parse::<usize>() {
                                self.current_memory_bytes = memory_kb * 1024;
                                if self.current_memory_bytes > self.peak_memory_bytes {
                                    self.peak_memory_bytes = self.current_memory_bytes;
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // Windows memory tracking would require winapi crate
            // For now, set to 0 as placeholder
            self.current_memory_bytes = 0;
            self.peak_memory_bytes = 0;
        }
    }
    
    /// Format memory size as human-readable string
    pub fn format_memory(bytes: usize) -> String {
        const KB: usize = 1024;
        const MB: usize = KB * 1024;
        const GB: usize = MB * 1024;
        
        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            terminal_ready: false,
            initialization_attempted: false,
            status_message: None,
            is_loading: false,
            loading_message: String::new(),
            loading_frame: 0,
            last_tab_press: None,
            completion_just_applied: false,
            error_dialog: None,
        }
    }
}

impl StateManager {
    /// Create a new state manager
    pub fn new() -> Self {
        let mut manager = Self {
            sessions: HashMap::new(),
            active_session_id: None,
            app_state: ApplicationState::default(),
            statistics: AppStatistics::default(),
        };
        
        // Create a default session
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let _session_id = manager.create_session(working_dir, crate::models::ShellType::Bash);
        
        manager
    }

    /// Create a new session
    pub fn create_session(
        &mut self,
        working_directory: PathBuf,
        shell_type: crate::models::ShellType,
    ) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session = SessionState {
            id: session_id.clone(),
            command_history: Vec::new(),
            working_directory,
            previous_directory: None,
            pty_handle_id: None,
            shell_type,
            start_time: Utc::now(),
            last_activity: Utc::now(),
            status: SessionStatus::Initializing,
            pending_output: Vec::new(),
            current_input: String::new(),
            input_history: Vec::new(),
            input_history_index: None,
            last_command_time: None,
            max_history_size: 1000,
        };

        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        session_id
    }

    /// Get the active session (mutable)
    pub fn active_session_mut(&mut self) -> Option<&mut SessionState> {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.sessions.get_mut(id))
    }

    /// Get the active session (immutable)
    pub fn active_session(&self) -> Option<&SessionState> {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.sessions.get(id))
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Option<&SessionState> {
        self.sessions.get(session_id)
    }

    /// Get a session by ID (mutable)
    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut SessionState> {
        self.sessions.get_mut(session_id)
    }

    /// Set the active session
    pub fn set_active_session(&mut self, session_id: String) -> bool {
        if self.sessions.contains_key(&session_id) {
            self.active_session_id = Some(session_id);
            true
        } else {
            false
        }
    }

    /// Get all session IDs
    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.keys().cloned().collect()
    }

    /// Remove a session
    pub fn remove_session(&mut self, session_id: &str) -> bool {
        if self.sessions.remove(session_id).is_some() {
            // If this was the active session, clear it
            if self.active_session_id.as_ref() == Some(&session_id.to_string()) {
                self.active_session_id = None;
            }
            true
        } else {
            false
        }
    }

    /// Get application state
    pub fn app_state(&self) -> &ApplicationState {
        &self.app_state
    }

    /// Get application state (mutable)
    pub fn app_state_mut(&mut self) -> &mut ApplicationState {
        &mut self.app_state
    }

    // Convenience methods for common app state operations

    /// Set status message
    pub fn set_status_message(&mut self, message: Option<String>) {
        self.app_state.status_message = message;
    }

    /// Get status message
    pub fn status_message(&self) -> Option<String> {
        self.app_state.status_message.clone()
    }

    /// Set terminal ready state
    pub fn set_terminal_ready(&mut self, ready: bool) {
        self.app_state.terminal_ready = ready;
    }

    /// Check if terminal is ready
    pub fn is_terminal_ready(&self) -> bool {
        self.app_state.terminal_ready
    }

    /// Set initialization attempted
    pub fn set_initialization_attempted(&mut self, attempted: bool) {
        self.app_state.initialization_attempted = attempted;
    }

    /// Check if initialization was attempted
    pub fn is_initialization_attempted(&self) -> bool {
        self.app_state.initialization_attempted
    }

    /// Set loading state
    pub fn set_loading(&mut self, is_loading: bool, message: Option<String>) {
        self.app_state.is_loading = is_loading;
        if let Some(msg) = message {
            self.app_state.loading_message = msg;
        }
    }

    /// Check if loading
    pub fn is_loading(&self) -> bool {
        self.app_state.is_loading
    }

    /// Get loading message
    pub fn loading_message(&self) -> &str {
        &self.app_state.loading_message
    }

    /// Increment loading frame (for animation)
    pub fn increment_loading_frame(&mut self) {
        self.app_state.loading_frame = self.app_state.loading_frame.wrapping_add(1);
    }

    /// Get loading frame
    pub fn loading_frame(&self) -> usize {
        self.app_state.loading_frame
    }

    /// Set last tab press time
    pub fn set_last_tab_press(&mut self, time: Option<std::time::Instant>) {
        self.app_state.last_tab_press = time;
    }

    /// Get last tab press time
    pub fn last_tab_press(&self) -> Option<std::time::Instant> {
        self.app_state.last_tab_press
    }

    /// Set completion just applied flag
    pub fn set_completion_just_applied(&mut self, applied: bool) {
        self.app_state.completion_just_applied = applied;
    }

    /// Check if completion was just applied
    pub fn completion_just_applied(&self) -> bool {
        self.app_state.completion_just_applied
    }

    /// Show an error dialog
    pub fn show_error(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        is_critical: bool,
    ) {
        self.app_state.error_dialog = Some(ErrorDialog {
            title: title.into(),
            message: message.into(),
            is_critical,
        });
    }

    /// Get the current error dialog
    pub fn error_dialog(&self) -> Option<&ErrorDialog> {
        self.app_state.error_dialog.as_ref()
    }

    /// Clear the error dialog
    pub fn clear_error(&mut self) {
        self.app_state.error_dialog = None;
    }

    /// Get command history for active session
    pub fn command_history(&self) -> Option<&Vec<CommandBlock>> {
        self.active_session().map(|s| &s.command_history)
    }

    /// Get command history for active session (mutable)
    pub fn command_history_mut(&mut self) -> Option<&mut Vec<CommandBlock>> {
        self.active_session_mut().map(|s| &mut s.command_history)
    }

    /// Get command history (returns empty vec if no active session)
    pub fn get_command_history(&self) -> Vec<CommandBlock> {
        self.command_history()
            .map(|history| history.clone())
            .unwrap_or_default()
    }

    /// Add an output line to a specific command block
    pub fn add_output_line(&mut self, block_id: &str, line: OutputLine) {
        if let Some(session) = self.active_session_mut() {
            if let Some(block) = session
                .command_history
                .iter_mut()
                .find(|b| b.id == block_id)
            {
                block.add_output_line(line);
                // Track output line in statistics
                self.statistics.total_output_lines += 1;
            }
        }
    }

    /// Add a command block to the active session
    pub fn add_command_block(&mut self, block: CommandBlock) {
        if let Some(session) = self.active_session_mut() {
            session.command_history.push(block.clone());
            // Track statistics when command is added (initially running)
            self.statistics.total_commands += 1;
        }
    }

    /// Update command block status
    pub fn update_command_block_status(
        &mut self,
        block_id: &str,
        status: crate::models::ExecutionStatus,
    ) {
        if let Some(session) = self.active_session_mut() {
            if let Some(block) = session
                .command_history
                .iter_mut()
                .find(|b| b.id == block_id)
            {
                let old_status = block.status.clone();
                block.status = status.clone();
                
                // Track status change in statistics (only count final status once)
                match (old_status, &status) {
                    (crate::models::ExecutionStatus::Running, crate::models::ExecutionStatus::Completed) => {
                        self.statistics.successful_commands += 1;
                    }
                    (crate::models::ExecutionStatus::Running, crate::models::ExecutionStatus::Failed) => {
                        self.statistics.failed_commands += 1;
                    }
                    (crate::models::ExecutionStatus::Running, crate::models::ExecutionStatus::Cancelled) => {
                        self.statistics.cancelled_commands += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Set last command execution time for active session
    pub fn set_last_command_time(&mut self, _time: DateTime<Utc>) {
        // Note: We track this in session state as Instant, not DateTime
        // This is a compatibility shim during migration
        if let Some(session) = self.active_session_mut() {
            session.last_command_time = Some(std::time::Instant::now());
        }
    }

    /// Get previous directory from active session
    pub fn get_previous_directory(&self) -> Option<PathBuf> {
        self.active_session()
            .and_then(|s| s.previous_directory.clone())
    }

    /// Set previous directory for active session
    pub fn set_previous_directory(&mut self, dir: Option<PathBuf>) {
        if let Some(session) = self.active_session_mut() {
            session.previous_directory = dir;
        }
    }
    
    // Statistics methods
    
    /// Get statistics
    pub fn statistics(&self) -> &AppStatistics {
        &self.statistics
    }
    
    /// Get statistics (mutable)
    pub fn statistics_mut(&mut self) -> &mut AppStatistics {
        &mut self.statistics
    }
    
    /// Increment command counter
    pub fn increment_command_count(&mut self, status: crate::models::ExecutionStatus) {
        self.statistics.total_commands += 1;
        match status {
            crate::models::ExecutionStatus::Completed => {
                self.statistics.successful_commands += 1;
            }
            crate::models::ExecutionStatus::Failed => {
                self.statistics.failed_commands += 1;
            }
            crate::models::ExecutionStatus::Cancelled => {
                self.statistics.cancelled_commands += 1;
            }
            _ => {}
        }
    }
    
    /// Increment output line counter
    pub fn increment_output_lines(&mut self, count: usize) {
        self.statistics.total_output_lines += count;
    }
    
    /// Update memory statistics
    pub fn update_memory_stats(&mut self) {
        self.statistics.update_memory();
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// Add a command block to the history
    pub fn add_command_block(&mut self, block: CommandBlock) {
        self.command_history.push(block);
        self.last_activity = Utc::now();

        // Enforce history size limit
        if self.command_history.len() > self.max_history_size {
            self.command_history.remove(0);
        }
    }

    /// Get the most recent command block
    pub fn current_command_block(&self) -> Option<&CommandBlock> {
        self.command_history.last()
    }

    /// Get the most recent command block (mutable)
    pub fn current_command_block_mut(&mut self) -> Option<&mut CommandBlock> {
        self.command_history.last_mut()
    }

    /// Change working directory
    pub fn change_directory(&mut self, new_dir: PathBuf) {
        self.previous_directory = Some(self.working_directory.clone());
        self.working_directory = new_dir;
        self.last_activity = Utc::now();
    }

    /// Go to previous directory (for `cd -`)
    pub fn go_to_previous_directory(&mut self) -> Option<PathBuf> {
        if let Some(prev_dir) = self.previous_directory.take() {
            let current = self.working_directory.clone();
            self.working_directory = prev_dir.clone();
            self.previous_directory = Some(current);
            self.last_activity = Utc::now();
            Some(prev_dir)
        } else {
            None
        }
    }

    /// Add output line to pending output
    pub fn add_pending_output(&mut self, line: OutputLine) {
        self.pending_output.push(line);
        self.last_activity = Utc::now();
    }

    /// Take all pending output lines
    pub fn take_pending_output(&mut self) -> Vec<OutputLine> {
        let output = std::mem::take(&mut self.pending_output);
        self.last_activity = Utc::now();
        output
    }

    /// Clear pending output
    pub fn clear_pending_output(&mut self) {
        self.pending_output.clear();
        self.last_activity = Utc::now();
    }

    /// Add command to input history
    pub fn add_to_input_history(&mut self, command: String) {
        if !command.is_empty() {
            self.input_history.push(command);
            self.input_history_index = None;
            self.last_activity = Utc::now();
        }
    }

    /// Navigate input history up (older)
    pub fn navigate_history_up(&mut self) -> Option<&str> {
        if self.input_history.is_empty() {
            return None;
        }

        let new_index = match self.input_history_index {
            None => Some(self.input_history.len() - 1),
            Some(idx) if idx > 0 => Some(idx - 1),
            Some(idx) => Some(idx),
        };

        self.input_history_index = new_index;
        new_index.map(|i| self.input_history[i].as_str())
    }

    /// Navigate input history down (newer)
    pub fn navigate_history_down(&mut self) -> Option<&str> {
        match self.input_history_index {
            None => None,
            Some(idx) if idx < self.input_history.len() - 1 => {
                let new_idx = idx + 1;
                self.input_history_index = Some(new_idx);
                Some(&self.input_history[new_idx])
            }
            Some(_) => {
                self.input_history_index = None;
                None
            }
        }
    }

    /// Get session statistics
    pub fn statistics(&self) -> SessionStatistics {
        let total_commands = self.command_history.len();
        let successful = self
            .command_history
            .iter()
            .filter(|b| b.is_successful())
            .count();
        let failed = self
            .command_history
            .iter()
            .filter(|b| b.is_failed())
            .count();

        SessionStatistics {
            total_commands,
            successful_commands: successful,
            failed_commands: failed,
            session_duration: Utc::now()
                .signed_duration_since(self.start_time)
                .to_std()
                .unwrap_or_default(),
        }
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub total_commands: usize,
    pub successful_commands: usize,
    pub failed_commands: usize,
    pub session_duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CommandBlock, ShellType};

    #[test]
    fn test_create_session() {
        let mut manager = StateManager::new();
        let session_id = manager.create_session(PathBuf::from("/tmp"), ShellType::Bash);

        assert!(manager.active_session().is_some());
        assert_eq!(manager.active_session().unwrap().id, session_id);
    }

    #[test]
    fn test_command_history() {
        let mut manager = StateManager::new();
        let _session_id = manager.create_session(PathBuf::from("/tmp"), ShellType::Bash);

        let block = CommandBlock::new("ls".to_string(), PathBuf::from("/tmp"));

        if let Some(session) = manager.active_session_mut() {
            session.add_command_block(block.clone());
        }

        assert_eq!(manager.command_history().unwrap().len(), 1);
    }

    #[test]
    fn test_directory_navigation() {
        let mut manager = StateManager::new();
        let _session_id = manager.create_session(PathBuf::from("/tmp"), ShellType::Bash);

        if let Some(session) = manager.active_session_mut() {
            session.change_directory(PathBuf::from("/home"));
            assert_eq!(session.working_directory, PathBuf::from("/home"));
            assert_eq!(session.previous_directory, Some(PathBuf::from("/tmp")));

            let prev = session.go_to_previous_directory();
            assert_eq!(prev, Some(PathBuf::from("/tmp")));
            assert_eq!(session.working_directory, PathBuf::from("/tmp"));
        }
    }

    #[test]
    fn test_input_history_navigation() {
        let mut manager = StateManager::new();
        let _session_id = manager.create_session(PathBuf::from("/tmp"), ShellType::Bash);

        if let Some(session) = manager.active_session_mut() {
            session.add_to_input_history("ls".to_string());
            session.add_to_input_history("pwd".to_string());
            session.add_to_input_history("cd /tmp".to_string());

            // Navigate up
            assert_eq!(session.navigate_history_up(), Some("cd /tmp"));
            assert_eq!(session.navigate_history_up(), Some("pwd"));
            assert_eq!(session.navigate_history_up(), Some("ls"));
            assert_eq!(session.navigate_history_up(), Some("ls")); // At start

            // Navigate down
            assert_eq!(session.navigate_history_down(), Some("pwd"));
            assert_eq!(session.navigate_history_down(), Some("cd /tmp"));
            assert_eq!(session.navigate_history_down(), None); // At end
        }
    }
}
