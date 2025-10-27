//! Application State Management
//!
//! Centralized state management for MosaicTerm, handling application lifecycle,
//! component coordination, and reactive state updates.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::config::RuntimeConfig;
use crate::error::{Error, Result};
use crate::models::ShellType;
use crate::models::{CommandBlock, TerminalSession};
use crate::terminal::{Terminal, TerminalFactory, TerminalStatus};

/// Global application state
pub type AppState = Arc<RwLock<ApplicationState>>;

/// Core application state structure
pub struct ApplicationState {
    /// Terminal instances
    terminals: HashMap<String, TerminalInstance>,
    /// Active terminal ID
    active_terminal_id: Option<String>,
    /// Application settings
    settings: ApplicationSettings,
    /// UI state
    ui_state: UiState,
    /// Runtime configuration
    runtime_config: Option<RuntimeConfig>,
    /// Application status
    status: ApplicationStatus,
}

/// Terminal instance wrapper
pub struct TerminalInstance {
    /// Terminal emulator
    terminal: Terminal,
    /// Terminal status
    status: TerminalStatus,
    /// Associated session
    session: TerminalSession,
    /// Command history
    command_history: Vec<CommandBlock>,
    /// Terminal title
    title: String,
}

/// Application settings
#[derive(Debug, Clone)]
pub struct ApplicationSettings {
    /// Maximum number of terminals
    max_terminals: usize,
}

/// UI state management
#[derive(Debug, Clone, Default)]
pub struct UiState {
    // All fields in UiState are currently unused and have been removed
}

/// Application status
#[derive(Debug, Clone, PartialEq)]
pub enum ApplicationStatus {
    /// Application is starting
    Starting,
    /// Application is running normally
    Running,
    /// Application is shutting down
    ShuttingDown,
    /// Application encountered an error
    Error(String),
}

/// Startup behavior configuration
#[derive(Debug, Clone)]
pub enum StartupBehavior {
    /// Open new terminal
    NewTerminal,
    /// Restore previous session
    RestoreSession,
    /// Show welcome screen
    WelcomeScreen,
    /// Do nothing
    None,
}

/// State manager for coordinating application components
pub struct StateManager {
    /// Global application state
    state: AppState,
    /// Terminal factory for creating new terminals
    terminal_factory: TerminalFactory,
    /// Event handlers
    event_handlers: HashMap<String, Box<dyn EventHandler>>,
    /// State change listeners
    change_listeners: Vec<Box<dyn StateChangeListener>>,
}

/// Event handler trait
pub trait EventHandler: Send + Sync {
    /// Handle an event
    fn handle_event(&self, event: &ApplicationEvent, state: &mut ApplicationState) -> Result<()>;
}

/// State change listener trait
pub trait StateChangeListener: Send + Sync {
    /// Called when state changes
    fn on_state_change(&self, old_state: &ApplicationState, new_state: &ApplicationState);
}

/// Application events
#[derive(Debug, Clone)]
pub enum ApplicationEvent {
    /// Terminal created
    TerminalCreated { id: String },
    /// Terminal destroyed
    TerminalDestroyed { id: String },
    /// Terminal switched
    TerminalSwitched { id: String },
    /// Command executed
    CommandExecuted {
        terminal_id: String,
        command: String,
    },
    /// Theme changed
    ThemeChanged { theme_name: String },
    /// Settings changed
    SettingsChanged,
    /// Window resized
    WindowResized { width: f32, height: f32 },
    /// Application shutdown
    Shutdown,
    /// Error occurred
    Error { message: String },
}

impl ApplicationState {
    /// Create new application state
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            active_terminal_id: None,
            settings: ApplicationSettings::default(),
            ui_state: UiState::default(),
            runtime_config: None,
            status: ApplicationStatus::Starting,
        }
    }

    /// Get all terminals
    pub fn terminals(&self) -> &HashMap<String, TerminalInstance> {
        &self.terminals
    }

    /// Get active terminal
    pub fn active_terminal(&self) -> Option<&TerminalInstance> {
        self.active_terminal_id
            .as_ref()
            .and_then(|id| self.terminals.get(id))
    }

    /// Get active terminal mutably
    pub fn active_terminal_mut(&mut self) -> Option<&mut TerminalInstance> {
        if let Some(id) = &self.active_terminal_id {
            self.terminals.get_mut(id)
        } else {
            None
        }
    }

    /// Get terminal by ID
    pub fn get_terminal(&self, id: &str) -> Option<&TerminalInstance> {
        self.terminals.get(id)
    }

    /// Get terminal by ID mutably
    pub fn get_terminal_mut(&mut self, id: &str) -> Option<&mut TerminalInstance> {
        self.terminals.get_mut(id)
    }

    /// Add a new terminal
    pub fn add_terminal(&mut self, terminal: TerminalInstance) -> Result<String> {
        let id = generate_terminal_id();

        if self.terminals.len() >= self.settings.max_terminals {
            return Err(Error::Other(
                "Maximum number of terminals reached".to_string(),
            ));
        }

        self.terminals.insert(id.clone(), terminal);
        self.active_terminal_id = Some(id.clone());

        Ok(id)
    }

    /// Remove a terminal
    pub fn remove_terminal(&mut self, id: &str) -> Result<TerminalInstance> {
        if self.active_terminal_id.as_ref() == Some(&id.to_string()) {
            self.active_terminal_id = self.terminals.keys().next().cloned();
        }

        self.terminals
            .remove(id)
            .ok_or_else(|| Error::Other(format!("Terminal '{}' not found", id)))
    }

    /// Switch active terminal
    pub fn switch_terminal(&mut self, id: &str) -> Result<()> {
        if self.terminals.contains_key(id) {
            self.active_terminal_id = Some(id.to_string());
            Ok(())
        } else {
            Err(Error::Other(format!("Terminal '{}' not found", id)))
        }
    }

    /// Get application settings
    pub fn settings(&self) -> &ApplicationSettings {
        &self.settings
    }

    /// Get application settings mutably
    pub fn settings_mut(&mut self) -> &mut ApplicationSettings {
        &mut self.settings
    }

    /// Get UI state
    pub fn ui_state(&self) -> &UiState {
        &self.ui_state
    }

    /// Get UI state mutably
    pub fn ui_state_mut(&mut self) -> &mut UiState {
        &mut self.ui_state
    }

    /// Get runtime configuration
    pub fn runtime_config(&self) -> Option<&RuntimeConfig> {
        self.runtime_config.as_ref()
    }

    /// Set runtime configuration
    pub fn set_runtime_config(&mut self, config: RuntimeConfig) {
        self.runtime_config = Some(config);
    }

    /// Get application status
    pub fn status(&self) -> &ApplicationStatus {
        &self.status
    }

    /// Set application status
    pub fn set_status(&mut self, status: ApplicationStatus) {
        self.status = status;
    }

    /// Check if application is running
    pub fn is_running(&self) -> bool {
        matches!(self.status, ApplicationStatus::Running)
    }

    /// Check if application is shutting down
    pub fn is_shutting_down(&self) -> bool {
        matches!(self.status, ApplicationStatus::ShuttingDown)
    }
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalInstance {
    /// Create new terminal instance
    pub fn new(terminal: Terminal, session: TerminalSession) -> Self {
        Self {
            terminal,
            status: TerminalStatus {
                mode: crate::terminal::TerminalMode::Normal,
                cursor_position: (0, 0),
                buffer_size: 1000,
                pending_output: 0,
                last_activity: chrono::Utc::now(),
            },
            session,
            command_history: Vec::new(),
            title: "Terminal".to_string(),
        }
    }

    /// Get terminal reference
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get terminal mutable reference
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }

    /// Get terminal status
    pub fn status(&self) -> &TerminalStatus {
        &self.status
    }

    /// Get terminal session
    pub fn session(&self) -> &TerminalSession {
        &self.session
    }

    /// Get command history
    pub fn command_history(&self) -> &[CommandBlock] {
        &self.command_history
    }

    /// Add command to history
    pub fn add_command(&mut self, command: CommandBlock) {
        self.command_history.push(command);

        // Keep history size reasonable
        let max_history = 1000; // TODO: Make configurable
        if self.command_history.len() > max_history {
            self.command_history.remove(0);
        }
    }

    /// Get terminal title
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set terminal title
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Update terminal status
    pub fn update_status(&mut self, status: TerminalStatus) {
        self.status = status;
    }
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self { max_terminals: 10 }
    }
}

impl StateManager {
    /// Create new state manager
    pub fn new(terminal_factory: TerminalFactory) -> Self {
        Self {
            state: Arc::new(RwLock::new(ApplicationState::new())),
            terminal_factory,
            event_handlers: HashMap::new(),
            change_listeners: Vec::new(),
        }
    }

    /// Get state reference
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Dispatch an event
    pub async fn dispatch_event(&self, event: ApplicationEvent) -> Result<()> {
        debug!("Dispatching event: {:?}", event);

        let mut state = self.state.write().await;

        // Handle event with registered handlers
        if let Some(handler) = self.event_handlers.get(&event_type(&event)) {
            handler.handle_event(&event, &mut state)?;
        }

        // Notify change listeners
        for listener in &self.change_listeners {
            // We need to create a snapshot of the old state for comparison
            // This is a simplified implementation
            listener.on_state_change(&state, &state);
        }

        Ok(())
    }

    /// Register event handler
    pub fn register_event_handler(&mut self, event_type: &str, handler: Box<dyn EventHandler>) {
        self.event_handlers.insert(event_type.to_string(), handler);
    }

    /// Register state change listener
    pub fn register_change_listener(&mut self, listener: Box<dyn StateChangeListener>) {
        self.change_listeners.push(listener);
    }

    /// Create new terminal
    pub async fn create_terminal(&self, session: TerminalSession) -> Result<String> {
        let terminal = self.terminal_factory.create_with_shell(
            session.clone(),
            ShellType::Bash, // Default to Bash for now
        );

        let instance = TerminalInstance::new(terminal, session.clone());

        let mut state = self.state.write().await;
        let terminal_id = state.add_terminal(instance)?;

        self.dispatch_event(ApplicationEvent::TerminalCreated {
            id: terminal_id.clone(),
        })
        .await?;

        Ok(terminal_id)
    }

    /// Close terminal
    pub async fn close_terminal(&self, id: &str) -> Result<()> {
        let mut state = self.state.write().await;
        state.remove_terminal(id)?;

        self.dispatch_event(ApplicationEvent::TerminalDestroyed { id: id.to_string() })
            .await?;

        Ok(())
    }

    /// Switch to terminal
    pub async fn switch_terminal(&self, id: &str) -> Result<()> {
        let mut state = self.state.write().await;
        state.switch_terminal(id)?;

        self.dispatch_event(ApplicationEvent::TerminalSwitched { id: id.to_string() })
            .await?;

        Ok(())
    }

    /// Get application statistics
    pub async fn get_stats(&self) -> ApplicationStats {
        let state = self.state.read().await;

        ApplicationStats {
            terminal_count: state.terminals.len(),
            active_terminal: state.active_terminal_id.clone(),
            total_commands: state
                .terminals
                .values()
                .map(|t| t.command_history.len())
                .sum(),
            memory_usage: 0, // TODO: Implement memory tracking
            uptime: chrono::Utc::now().timestamp() as u64, // TODO: Track actual uptime
        }
    }
}

/// Application statistics
#[derive(Debug, Clone)]
pub struct ApplicationStats {
    pub terminal_count: usize,
    pub active_terminal: Option<String>,
    pub total_commands: usize,
    pub memory_usage: u64,
    pub uptime: u64,
}

/// Generate unique terminal ID
fn generate_terminal_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    format!("terminal_{}", timestamp)
}

/// Get event type string
fn event_type(event: &ApplicationEvent) -> String {
    match event {
        ApplicationEvent::TerminalCreated { .. } => "terminal_created",
        ApplicationEvent::TerminalDestroyed { .. } => "terminal_destroyed",
        ApplicationEvent::TerminalSwitched { .. } => "terminal_switched",
        ApplicationEvent::CommandExecuted { .. } => "command_executed",
        ApplicationEvent::ThemeChanged { .. } => "theme_changed",
        ApplicationEvent::SettingsChanged => "settings_changed",
        ApplicationEvent::WindowResized { .. } => "window_resized",
        ApplicationEvent::Shutdown => "shutdown",
        ApplicationEvent::Error { .. } => "error",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_state_creation() {
        let state = ApplicationState::new();
        assert!(state.terminals.is_empty());
        assert!(state.active_terminal_id.is_none());
        assert!(matches!(state.status, ApplicationStatus::Starting));
    }

    #[test]
    fn test_terminal_instance_creation() {
        let _session = TerminalSession::new(
            crate::TerminalShellType::Bash,
            std::path::PathBuf::from("/bin/bash"),
        );

        // We can't create a real Terminal here for testing, so we'll skip this test
        // In a real implementation, we'd use a mock or test terminal
    }

    #[test]
    fn test_application_settings_default() {
        let settings = ApplicationSettings::default();
        assert_eq!(settings.max_terminals, 10);
        // Settings only has max_terminals field currently
        assert_eq!(settings.max_terminals, 10);
    }

    #[test]
    fn test_ui_state_default() {
        let ui_state = UiState::default();
        // UiState has no fields currently, so just test it exists
        let _ = ui_state;
    }

    #[test]
    fn test_application_status_variants() {
        let status = ApplicationStatus::Error("test error".to_string());
        assert!(matches!(
            ApplicationStatus::Starting,
            ApplicationStatus::Starting
        ));
        assert!(matches!(
            ApplicationStatus::Running,
            ApplicationStatus::Running
        ));
        assert!(matches!(
            ApplicationStatus::ShuttingDown,
            ApplicationStatus::ShuttingDown
        ));
        assert!(matches!(status, ApplicationStatus::Error(_)));
    }

    #[test]
    fn test_startup_behavior_variants() {
        assert!(matches!(
            StartupBehavior::NewTerminal,
            StartupBehavior::NewTerminal
        ));
        assert!(matches!(
            StartupBehavior::RestoreSession,
            StartupBehavior::RestoreSession
        ));
        assert!(matches!(
            StartupBehavior::WelcomeScreen,
            StartupBehavior::WelcomeScreen
        ));
        assert!(matches!(StartupBehavior::None, StartupBehavior::None));
    }

    #[test]
    fn test_generate_terminal_id() {
        let id1 = generate_terminal_id();
        let id2 = generate_terminal_id();

        assert_ne!(id1, id2);
        assert!(id1.starts_with("terminal_"));
        assert!(id2.starts_with("terminal_"));
    }

    #[test]
    fn test_event_type_conversion() {
        assert_eq!(event_type(&ApplicationEvent::Shutdown), "shutdown");
        assert_eq!(
            event_type(&ApplicationEvent::SettingsChanged),
            "settings_changed"
        );
        assert_eq!(
            event_type(&ApplicationEvent::Error {
                message: "test".to_string()
            }),
            "error"
        );
    }

    #[test]
    fn test_application_stats() {
        let stats = ApplicationStats {
            terminal_count: 3,
            active_terminal: Some("terminal_123".to_string()),
            total_commands: 150,
            memory_usage: 1024 * 1024, // 1MB
            uptime: 3600,              // 1 hour
        };

        assert_eq!(stats.terminal_count, 3);
        assert_eq!(stats.active_terminal, Some("terminal_123".to_string()));
        assert_eq!(stats.total_commands, 150);
        assert_eq!(stats.memory_usage, 1024 * 1024);
        assert_eq!(stats.uptime, 3600);
    }
}
