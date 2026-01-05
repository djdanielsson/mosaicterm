//! PTY Event System
//!
//! Provides an event-driven architecture for PTY output handling.
//! Instead of polling for output, subscribers receive events when output is available.

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

/// Events emitted by the PTY event system
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// Raw output data from the PTY
    Output {
        /// The PTY handle ID
        handle_id: String,
        /// Raw output bytes
        data: Vec<u8>,
    },
    /// PTY process has exited
    ProcessExited {
        /// The PTY handle ID
        handle_id: String,
        /// Exit code (if available)
        exit_code: Option<i32>,
    },
    /// Error occurred during PTY operation
    Error {
        /// The PTY handle ID
        handle_id: String,
        /// Error message
        message: String,
    },
    /// PTY was created
    Created {
        /// The PTY handle ID
        handle_id: String,
        /// Process ID
        pid: Option<u32>,
    },
    /// PTY was terminated
    Terminated {
        /// The PTY handle ID
        handle_id: String,
    },
}

/// Subscription handle for receiving PTY events
pub struct PtyEventSubscription {
    receiver: broadcast::Receiver<PtyEvent>,
}

impl PtyEventSubscription {
    /// Receive the next event, waiting if necessary
    pub async fn recv(&mut self) -> Option<PtyEvent> {
        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                tracing::warn!("PTY event subscriber lagged by {} events", count);
                // Try to receive the next available event
                self.receiver.recv().await.ok()
            }
        }
    }

    /// Try to receive an event without blocking
    pub fn try_recv(&mut self) -> Option<PtyEvent> {
        match self.receiver.try_recv() {
            Ok(event) => Some(event),
            Err(broadcast::error::TryRecvError::Empty) => None,
            Err(broadcast::error::TryRecvError::Closed) => None,
            Err(broadcast::error::TryRecvError::Lagged(count)) => {
                tracing::warn!("PTY event subscriber lagged by {} events", count);
                self.try_recv() // Try again after clearing lag
            }
        }
    }
}

/// PTY Event Bus for publishing and subscribing to PTY events
#[derive(Clone)]
pub struct PtyEventBus {
    sender: broadcast::Sender<PtyEvent>,
    /// Active subscribers count (for monitoring)
    active_subscribers: Arc<RwLock<usize>>,
}

impl PtyEventBus {
    /// Create a new event bus with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            active_subscribers: Arc::new(RwLock::new(0)),
        }
    }

    /// Subscribe to PTY events
    pub async fn subscribe(&self) -> PtyEventSubscription {
        let receiver = self.sender.subscribe();
        let mut count = self.active_subscribers.write().await;
        *count += 1;
        PtyEventSubscription { receiver }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: PtyEvent) {
        // Ignore errors - they just mean no subscribers are active
        let _ = self.sender.send(event);
    }

    /// Get the number of active subscribers
    pub async fn subscriber_count(&self) -> usize {
        *self.active_subscribers.read().await
    }
}

impl Default for PtyEventBus {
    fn default() -> Self {
        Self::new(256) // Default capacity for buffering events
    }
}

/// PTY Output Watcher - monitors PTY output and emits events
pub struct PtyOutputWatcher {
    event_bus: PtyEventBus,
    /// Channel for stopping the watcher
    stop_tx: Option<mpsc::Sender<()>>,
}

impl PtyOutputWatcher {
    /// Create a new output watcher with the given event bus
    pub fn new(event_bus: PtyEventBus) -> Self {
        Self {
            event_bus,
            stop_tx: None,
        }
    }

    /// Start watching a PTY handle for output
    /// Returns a handle that can be used to stop watching
    pub fn start_watching(
        &mut self,
        handle_id: String,
        mut output_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> WatchHandle {
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);
        self.stop_tx = Some(stop_tx.clone());

        let event_bus = self.event_bus.clone();
        let hid = handle_id.clone();

        // Spawn a task that watches the PTY output
        tokio::spawn(async move {
            tracing::debug!("Started watching PTY output for handle: {}", hid);

            loop {
                tokio::select! {
                    // Check for stop signal
                    _ = stop_rx.recv() => {
                        tracing::debug!("Stopping PTY output watcher for handle: {}", hid);
                        break;
                    }
                    // Wait for output
                    output = output_rx.recv() => {
                        match output {
                            Some(data) if !data.is_empty() => {
                                event_bus.publish(PtyEvent::Output {
                                    handle_id: hid.clone(),
                                    data,
                                });
                            }
                            Some(_) => {
                                // Empty data, continue watching
                            }
                            None => {
                                // Channel closed, process likely exited
                                tracing::debug!("PTY output channel closed for handle: {}", hid);
                                event_bus.publish(PtyEvent::ProcessExited {
                                    handle_id: hid.clone(),
                                    exit_code: None,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        });

        WatchHandle { stop_tx }
    }

    /// Get a reference to the event bus
    pub fn event_bus(&self) -> &PtyEventBus {
        &self.event_bus
    }
}

/// Handle for stopping a PTY output watcher
pub struct WatchHandle {
    stop_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    /// Stop watching the PTY output
    pub async fn stop(self) {
        let _ = self.stop_tx.send(()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_subscribe() {
        let bus = PtyEventBus::new(16);

        // Subscribe before publishing
        let mut sub = bus.subscribe().await;

        // Publish an event
        bus.publish(PtyEvent::Output {
            handle_id: "test-1".to_string(),
            data: b"hello".to_vec(),
        });

        // Receive the event
        let event = sub.recv().await.expect("Should receive event");
        match event {
            PtyEvent::Output { handle_id, data } => {
                assert_eq!(handle_id, "test-1");
                assert_eq!(data, b"hello");
            }
            _ => panic!("Expected Output event"),
        }
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = PtyEventBus::new(16);

        let mut sub1 = bus.subscribe().await;
        let mut sub2 = bus.subscribe().await;

        bus.publish(PtyEvent::Created {
            handle_id: "test-2".to_string(),
            pid: Some(12345),
        });

        // Both subscribers should receive the event
        let event1 = sub1.recv().await.expect("Sub1 should receive event");
        let event2 = sub2.recv().await.expect("Sub2 should receive event");

        match (event1, event2) {
            (
                PtyEvent::Created {
                    handle_id: h1,
                    pid: p1,
                },
                PtyEvent::Created {
                    handle_id: h2,
                    pid: p2,
                },
            ) => {
                assert_eq!(h1, "test-2");
                assert_eq!(h2, "test-2");
                assert_eq!(p1, Some(12345));
                assert_eq!(p2, Some(12345));
            }
            _ => panic!("Expected Created events"),
        }
    }

    #[tokio::test]
    async fn test_try_recv_empty() {
        let bus = PtyEventBus::new(16);
        let mut sub = bus.subscribe().await;

        // Should return None when no events available
        assert!(sub.try_recv().is_none());
    }

    #[tokio::test]
    async fn test_output_watcher() {
        let event_bus = PtyEventBus::new(16);
        let mut watcher = PtyOutputWatcher::new(event_bus.clone());

        // Create a channel to simulate PTY output
        let (tx, rx) = mpsc::unbounded_channel();

        // Subscribe to events
        let mut sub = event_bus.subscribe().await;

        // Start watching
        let handle = watcher.start_watching("test-3".to_string(), rx);

        // Simulate PTY output
        tx.send(b"output line 1".to_vec()).unwrap();

        // Give the async task time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Should receive the event
        let event = sub.try_recv().expect("Should receive event");
        match event {
            PtyEvent::Output { handle_id, data } => {
                assert_eq!(handle_id, "test-3");
                assert_eq!(data, b"output line 1");
            }
            _ => panic!("Expected Output event"),
        }

        // Stop watching
        handle.stop().await;
    }
}
