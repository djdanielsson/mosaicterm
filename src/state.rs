//! Application State Management
//!
//! Simplified state management for MosaicTerm, handling application lifecycle
//! and basic status tracking.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Global application state (thread-safe)
pub type AppState = Arc<RwLock<ApplicationState>>;

/// Minimal application state structure
#[derive(Debug, Clone, Default)]
pub struct ApplicationState {
    /// Application status
    pub status: ApplicationStatus,
}

impl ApplicationState {
    /// Create new application state
    pub fn new() -> Self {
        Self {
            status: ApplicationStatus::Starting,
        }
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

/// Application status
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ApplicationStatus {
    /// Application is starting
    #[default]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_state_creation() {
        let state = ApplicationState::new();
        assert!(matches!(state.status, ApplicationStatus::Starting));
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
    fn test_state_is_running() {
        let mut state = ApplicationState::new();
        assert!(!state.is_running());

        state.set_status(ApplicationStatus::Running);
        assert!(state.is_running());
        assert!(!state.is_shutting_down());
    }

    #[test]
    fn test_state_is_shutting_down() {
        let mut state = ApplicationState::new();
        state.set_status(ApplicationStatus::ShuttingDown);
        assert!(state.is_shutting_down());
        assert!(!state.is_running());
    }
}
