//! PTY Operations Abstraction
//!
//! This module provides an abstraction layer for PTY operations,
//! allowing for easier testing and decoupling of the application
//! from direct PTY management.

use crate::error::Result;
use crate::pty::{PtyHandle, PtyInfo};
use async_trait::async_trait;

/// Trait defining the operations that can be performed on PTY processes
///
/// This abstraction allows for:
/// - Easier unit testing with mock implementations
/// - Decoupling of the application from direct PTY access
/// - Better separation of concerns
/// - Potential for future alternative implementations (e.g., remote PTY)
#[async_trait]
pub trait PtyOperations: Send + Sync {
    /// Send input data to a PTY process
    ///
    /// # Arguments
    /// * `handle` - The PTY handle to send input to
    /// * `data` - The input data to send
    ///
    /// # Errors
    /// Returns an error if the PTY is not found or if sending fails
    async fn send_input(&mut self, handle: &PtyHandle, data: &[u8]) -> Result<()>;

    /// Attempt to read output from a PTY process (non-blocking)
    ///
    /// # Arguments
    /// * `handle` - The PTY handle to read from
    ///
    /// # Returns
    /// The output data, or an empty Vec if no data is available
    ///
    /// # Errors
    /// Returns an error if the PTY is not found
    fn try_read_output_now(&mut self, handle: &PtyHandle) -> Result<Vec<u8>>;

    /// Get information about a PTY process
    ///
    /// # Arguments
    /// * `handle` - The PTY handle to get info for
    ///
    /// # Returns
    /// The PTY information
    ///
    /// # Errors
    /// Returns an error if the PTY is not found
    fn get_info(&self, handle: &PtyHandle) -> Result<PtyInfo>;

    /// Get the count of active PTY processes
    ///
    /// # Returns
    /// The number of active PTY processes
    fn active_count(&self) -> usize;

    /// Clean up terminated PTY processes
    ///
    /// Removes PTY processes that have exited from the manager.
    /// Returns the number of processes cleaned up.
    fn cleanup_terminated(&mut self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// Mock implementation of PtyOperations for testing
    pub struct MockPtyOperations {
        /// Store input sent to PTYs
        pub input_log: Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>>,
        /// Store output to return from PTYs
        pub output_queue: Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>>,
        /// Store PTY information
        pub pty_info: Arc<Mutex<HashMap<String, PtyInfo>>>,
        /// Count of active PTYs
        pub active_count: Arc<Mutex<usize>>,
    }

    impl MockPtyOperations {
        /// Create a new mock PTY operations instance
        pub fn new() -> Self {
            Self {
                input_log: Arc::new(Mutex::new(HashMap::new())),
                output_queue: Arc::new(Mutex::new(HashMap::new())),
                pty_info: Arc::new(Mutex::new(HashMap::new())),
                active_count: Arc::new(Mutex::new(0)),
            }
        }

        /// Add a PTY to the mock
        pub fn add_pty(&mut self, handle: &PtyHandle, info: PtyInfo) {
            let mut infos = self.pty_info.lock().unwrap();
            infos.insert(handle.id.clone(), info);
            let mut count = self.active_count.lock().unwrap();
            *count += 1;
        }

        /// Queue output for a PTY
        pub fn queue_output(&mut self, handle: &PtyHandle, data: Vec<u8>) {
            let mut queue = self.output_queue.lock().unwrap();
            queue
                .entry(handle.id.clone())
                .or_insert_with(Vec::new)
                .push(data);
        }

        /// Get the input log for a PTY
        pub fn get_input(&self, handle: &PtyHandle) -> Vec<Vec<u8>> {
            let log = self.input_log.lock().unwrap();
            log.get(&handle.id).cloned().unwrap_or_default()
        }
    }

    impl Default for MockPtyOperations {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl PtyOperations for MockPtyOperations {
        async fn send_input(&mut self, handle: &PtyHandle, data: &[u8]) -> Result<()> {
            let mut log = self.input_log.lock().unwrap();
            log.entry(handle.id.clone())
                .or_insert_with(Vec::new)
                .push(data.to_vec());
            Ok(())
        }

        fn try_read_output_now(&mut self, handle: &PtyHandle) -> Result<Vec<u8>> {
            let mut queue = self.output_queue.lock().unwrap();
            if let Some(outputs) = queue.get_mut(&handle.id) {
                if !outputs.is_empty() {
                    return Ok(outputs.remove(0));
                }
            }
            Ok(Vec::new())
        }

        fn get_info(&self, handle: &PtyHandle) -> Result<PtyInfo> {
            let infos = self.pty_info.lock().unwrap();
            infos
                .get(&handle.id)
                .cloned()
                .ok_or_else(|| crate::error::Error::PtyHandleNotFound {
                    handle_id: handle.id.clone(),
                })
        }

        fn active_count(&self) -> usize {
            *self.active_count.lock().unwrap()
        }

        fn cleanup_terminated(&mut self) -> usize {
            // For mock, just return 0
            0
        }
    }
}

