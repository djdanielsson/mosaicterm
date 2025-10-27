//! Event Handling and Message Passing
//!
//! Asynchronous event system for MosaicTerm, enabling loose coupling between
//! components through message passing and event-driven architecture.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use tracing::{debug, info};

use crate::error::{Error, Result};

/// Event bus for application-wide message passing
pub struct EventBus {
    /// Event channels for different event types
    channels: HashMap<String, EventChannel>,
    /// Global broadcast channel for system events
    broadcast_tx: broadcast::Sender<SystemEvent>,
    /// Shutdown signal
    shutdown_tx: tokio::sync::mpsc::UnboundedSender<()>,
}

/// Event channel for a specific event type
pub struct EventChannel {
    /// Sender for this event type
    sender: mpsc::UnboundedSender<EventEnvelope>,
}

/// Event envelope with metadata
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    /// Event ID
    pub id: String,
    /// Event type
    pub event_type: String,
    /// Event payload
    pub payload: EventPayload,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Source component
    pub source: String,
    /// Target component (optional)
    pub target: Option<String>,
    /// Correlation ID for request/response patterns
    pub correlation_id: Option<String>,
}

/// Event payload types
#[derive(Debug, Clone)]
pub enum EventPayload {
    /// Terminal events
    Terminal(TerminalEvent),
    /// UI events
    Ui(UiEvent),
    /// Command events
    Command(CommandEvent),
    /// Configuration events
    Config(ConfigEvent),
    /// System events
    System(SystemEvent),
    /// Custom events with JSON payload
    Custom(serde_json::Value),
}

/// Terminal-specific events
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal created
    Created { id: String },
    /// Terminal destroyed
    Destroyed { id: String },
    /// Terminal switched to
    Switched { id: String },
    /// Terminal output received
    Output { id: String, data: Vec<u8> },
    /// Terminal command executed
    CommandExecuted { id: String, command: String, success: bool },
    /// Terminal title changed
    TitleChanged { id: String, title: String },
    /// Terminal bell triggered
    Bell { id: String },
    /// Terminal resized
    Resized { id: String, cols: u16, rows: u16 },
}

/// UI-specific events
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Theme changed
    ThemeChanged { theme_name: String },
    /// Window resized
    WindowResized { width: f32, height: f32 },
    /// Key pressed
    KeyPressed { key: String, modifiers: Vec<String> },
    /// Mouse clicked
    MouseClicked { x: f32, y: f32, button: String },
    /// Menu item selected
    MenuSelected { menu: String, item: String },
    /// Context menu shown
    ContextMenu { x: f32, y: f32, items: Vec<String> },
    /// Search query changed
    SearchChanged { query: String },
    /// Sidebar toggled
    SidebarToggled { visible: bool },
}

/// Command-specific events
#[derive(Debug, Clone)]
pub enum CommandEvent {
    /// Command started
    Started { id: String, command: String },
    /// Command completed
    Completed { id: String, command: String, exit_code: Option<i32> },
    /// Command failed
    Failed { id: String, command: String, error: String },
    /// Command interrupted
    Interrupted { id: String, command: String },
    /// Command history updated
    HistoryUpdated { terminal_id: String },
    /// Command suggestion requested
    SuggestionRequested { partial: String },
    /// Command suggestion provided
    SuggestionProvided { partial: String, suggestions: Vec<String> },
}

/// Configuration events
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration loaded
    Loaded { path: String },
    /// Configuration saved
    Saved { path: String },
    /// Configuration changed
    Changed { key: String, old_value: String, new_value: String },
    /// Configuration reset
    Reset,
    /// Configuration validation failed
    ValidationFailed { errors: Vec<String> },
}

/// System-wide events
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// Application started
    ApplicationStarted,
    /// Application shutting down
    ApplicationShutdown,
    /// Memory warning
    MemoryWarning { used: u64, available: u64 },
    /// Disk space warning
    DiskSpaceWarning { path: String, used: u64, available: u64 },
    /// Network status changed
    NetworkChanged { connected: bool },
    /// System theme changed
    SystemThemeChanged { is_dark: bool },
    /// Error occurred
    Error { message: String, component: String },
    /// Performance metrics updated
    PerformanceMetrics { fps: f32, memory_mb: f32 },
}

/// Event handler trait
pub trait EventHandler: Send + Sync {
    /// Handle an event
    fn handle_event(&self, envelope: &EventEnvelope) -> Result<()>;

    /// Get handler name
    fn name(&self) -> &str;
}

/// Event processor for background event handling
pub struct EventProcessor {
    /// Event bus reference
    event_bus: Arc<EventBus>,
    /// Registered event handlers
    handlers: HashMap<String, Vec<Box<dyn EventHandler>>>,
    /// Processing task handle
    processing_task: Option<tokio::task::JoinHandle<()>>,
}


impl EventBus {
    /// Create new event bus
    pub fn new() -> Result<Self> {
        let (broadcast_tx, _) = broadcast::channel(100);
        let (shutdown_tx, _) = tokio::sync::mpsc::unbounded_channel::<()>();

        // Initialize shutdown receiver (we'll use it later)
        let _shutdown_rx = tokio::sync::mpsc::unbounded_channel::<()>().1;

        Ok(Self {
            channels: HashMap::new(),
            broadcast_tx,
            shutdown_tx,
        })
    }

    /// Publish an event
    pub fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        debug!("Publishing event: {} from {}", envelope.event_type, envelope.source);

        // Publish to specific event channel
        if let Some(channel) = self.channels.get(&envelope.event_type) {
            channel.sender.send(envelope.clone())
                .map_err(|e| Error::Other(format!("Failed to send event: {}", e)))?;
        }

        // Publish to broadcast channel for system events
        if let EventPayload::System(_) = &envelope.payload {
            let _ = self.broadcast_tx.send(envelope.payload.clone().into_system_event());
        }

        Ok(())
    }


    /// Get broadcast receiver for system events
    pub fn subscribe_broadcast(&self) -> broadcast::Receiver<SystemEvent> {
        self.broadcast_tx.subscribe()
    }

    /// Shutdown the event bus
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown_tx.send(())
            .map_err(|e| Error::Other(format!("Failed to send shutdown signal: {}", e)))
    }

    /// Create event envelope
    pub fn create_envelope(
        event_type: &str,
        payload: EventPayload,
        source: &str,
    ) -> EventEnvelope {
        EventEnvelope {
            id: generate_event_id(),
            event_type: event_type.to_string(),
            payload,
            timestamp: chrono::Utc::now(),
            source: source.to_string(),
            target: None,
            correlation_id: None,
        }
    }
}

impl EventEnvelope {
    /// Create response envelope for request-reply pattern
    pub fn create_response(&self, payload: EventPayload, source: &str) -> EventEnvelope {
        EventEnvelope {
            id: generate_event_id(),
            event_type: format!("{}_response", self.event_type),
            payload,
            timestamp: chrono::Utc::now(),
            source: source.to_string(),
            target: Some(self.source.clone()),
            correlation_id: self.correlation_id.clone(),
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, id: String) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Set target
    pub fn with_target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }
}

impl EventPayload {
    /// Convert to system event if applicable
    fn into_system_event(self) -> SystemEvent {
        match self {
            EventPayload::System(event) => event,
            _ => SystemEvent::Error {
                message: "Non-system event sent to broadcast".to_string(),
                component: "event_bus".to_string(),
            },
        }
    }
}

impl EventProcessor {
    /// Create new event processor
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            handlers: HashMap::new(),
            processing_task: None,
        }
    }

    /// Register event handler
    pub fn register_handler(&mut self, event_type: &str, handler: Box<dyn EventHandler>) {
        self.handlers
            .entry(event_type.to_string())
            .or_default()
            .push(handler);
    }

    /// Start processing events
    pub fn start(&mut self) -> Result<()> {
        let _event_bus = Arc::clone(&self.event_bus);

        let task = tokio::spawn(async move {
            info!("Event processor started");

            // Simplified event processing - in a real implementation,
            // this would handle multiple event types and handlers
            info!("Event processor stopped");
        });

        self.processing_task = Some(task);
        Ok(())
    }

    /// Stop processing events
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(task) = self.processing_task.take() {
            task.abort();
            let _ = task.await;
        }
        Ok(())
    }
}


/// Generate unique event ID
fn generate_event_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    format!("evt_{}", timestamp)
}

/// Event builder for fluent event creation
pub struct EventBuilder {
    event_type: String,
    payload: EventPayload,
    source: String,
    target: Option<String>,
    correlation_id: Option<String>,
}

impl EventBuilder {
    /// Create new event builder
    pub fn new(event_type: &str, payload: EventPayload, source: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload,
            source: source.to_string(),
            target: None,
            correlation_id: None,
        }
    }

    /// Set target component
    pub fn target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }

    /// Build the event envelope
    pub fn build(self) -> EventEnvelope {
        EventEnvelope {
            id: generate_event_id(),
            event_type: self.event_type,
            payload: self.payload,
            timestamp: chrono::Utc::now(),
            source: self.source,
            target: self.target,
            correlation_id: self.correlation_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_envelope_creation() {
        let payload = EventPayload::System(SystemEvent::ApplicationStarted);
        let envelope = EventBus::create_envelope("test", payload, "test_component");

        assert_eq!(envelope.event_type, "test");
        assert_eq!(envelope.source, "test_component");
        assert!(envelope.id.starts_with("evt_"));
        assert!(envelope.correlation_id.is_none());
        assert!(envelope.target.is_none());
    }

    #[test]
    fn test_event_builder() {
        let payload = EventPayload::System(SystemEvent::ApplicationStarted);
        let envelope = EventBuilder::new("test", payload, "component")
            .target("target_component")
            .correlation_id("corr_123")
            .build();

        assert_eq!(envelope.event_type, "test");
        assert_eq!(envelope.source, "component");
        assert_eq!(envelope.target, Some("target_component".to_string()));
        assert_eq!(envelope.correlation_id, Some("corr_123".to_string()));
    }

    #[test]
    fn test_generate_event_id() {
        let id1 = generate_event_id();
        let id2 = generate_event_id();

        assert_ne!(id1, id2);
        assert!(id1.starts_with("evt_"));
        assert!(id2.starts_with("evt_"));
    }

    #[test]
    fn test_terminal_event_variants() {
        assert!(matches!(TerminalEvent::Created { id: "test".to_string() },
                        TerminalEvent::Created { .. }));
        assert!(matches!(TerminalEvent::Destroyed { id: "test".to_string() },
                        TerminalEvent::Destroyed { .. }));
        assert!(matches!(TerminalEvent::Output { id: "test".to_string(), data: vec![] },
                        TerminalEvent::Output { .. }));
    }

    #[test]
    fn test_ui_event_variants() {
        assert!(matches!(UiEvent::ThemeChanged { theme_name: "dark".to_string() },
                        UiEvent::ThemeChanged { .. }));
        assert!(matches!(UiEvent::WindowResized { width: 800.0, height: 600.0 },
                        UiEvent::WindowResized { .. }));
        assert!(matches!(UiEvent::KeyPressed { key: "a".to_string(), modifiers: vec![] },
                        UiEvent::KeyPressed { .. }));
    }

    #[test]
    fn test_command_event_variants() {
        assert!(matches!(CommandEvent::Started { id: "test".to_string(), command: "ls".to_string() },
                        CommandEvent::Started { .. }));
        assert!(matches!(CommandEvent::Completed { id: "test".to_string(), command: "ls".to_string(), exit_code: Some(0) },
                        CommandEvent::Completed { .. }));
        assert!(matches!(CommandEvent::Failed { id: "test".to_string(), command: "invalid".to_string(), error: "error".to_string() },
                        CommandEvent::Failed { .. }));
    }

    #[test]
    fn test_config_event_variants() {
        assert!(matches!(ConfigEvent::Loaded { path: "/config".to_string() },
                        ConfigEvent::Loaded { .. }));
        assert!(matches!(ConfigEvent::Changed { key: "theme".to_string(), old_value: "light".to_string(), new_value: "dark".to_string() },
                        ConfigEvent::Changed { .. }));
        assert!(matches!(ConfigEvent::ValidationFailed { errors: vec!["error".to_string()] },
                        ConfigEvent::ValidationFailed { .. }));
    }

    #[test]
    fn test_system_event_variants() {
        assert!(matches!(SystemEvent::ApplicationStarted, SystemEvent::ApplicationStarted));
        assert!(matches!(SystemEvent::ApplicationShutdown, SystemEvent::ApplicationShutdown));
        assert!(matches!(SystemEvent::MemoryWarning { used: 100, available: 1000 },
                        SystemEvent::MemoryWarning { .. }));
        assert!(matches!(SystemEvent::Error { message: "test".to_string(), component: "test".to_string() },
                        SystemEvent::Error { .. }));
    }

    #[test]
    fn test_event_payload_variants() {
        let terminal_event = TerminalEvent::Created { id: "test".to_string() };
        let ui_event = UiEvent::ThemeChanged { theme_name: "dark".to_string() };
        let command_event = CommandEvent::Started { id: "test".to_string(), command: "ls".to_string() };
        let config_event = ConfigEvent::Loaded { path: "/config".to_string() };
        let system_event = SystemEvent::ApplicationStarted;
        let custom_event = serde_json::json!({"test": "value"});

        assert!(matches!(EventPayload::Terminal(terminal_event), EventPayload::Terminal(_)));
        assert!(matches!(EventPayload::Ui(ui_event), EventPayload::Ui(_)));
        assert!(matches!(EventPayload::Command(command_event), EventPayload::Command(_)));
        assert!(matches!(EventPayload::Config(config_event), EventPayload::Config(_)));
        assert!(matches!(EventPayload::System(system_event), EventPayload::System(_)));
        assert!(matches!(EventPayload::Custom(custom_event), EventPayload::Custom(_)));
    }

    #[test]
    fn test_event_envelope_response() {
        let original = EventEnvelope {
            id: "original".to_string(),
            event_type: "request".to_string(),
            payload: EventPayload::System(SystemEvent::ApplicationStarted),
            timestamp: chrono::Utc::now(),
            source: "client".to_string(),
            target: None,
            correlation_id: Some("corr_123".to_string()),
        };

        let response = original.create_response(
            EventPayload::System(SystemEvent::ApplicationShutdown),
            "server"
        );

        assert_eq!(response.event_type, "request_response");
        assert_eq!(response.source, "server");
        assert_eq!(response.target, Some("client".to_string()));
        assert_eq!(response.correlation_id, Some("corr_123".to_string()));
    }
}
