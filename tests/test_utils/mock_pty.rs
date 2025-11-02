//! Mock PTY Implementation for Testing

use mosaicterm::error::{Error, Result};
use mosaicterm::pty::{PtyHandle, PtyInfo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Mock PTY process for testing
#[derive(Debug, Clone)]
pub struct MockPtyProcess {
    pub handle: PtyHandle,
    pub command: String,
    pub output_queue: Vec<Vec<u8>>,
    pub input_received: Vec<Vec<u8>>,
    pub is_running: bool,
    pub exit_code: Option<i32>,
}

impl MockPtyProcess {
    /// Create a new mock PTY process
    pub fn new(command: String) -> Self {
        Self {
            handle: PtyHandle::new(),
            command,
            output_queue: Vec::new(),
            input_received: Vec::new(),
            is_running: true,
            exit_code: None,
        }
    }

    /// Queue output to be read
    pub fn queue_output(&mut self, data: Vec<u8>) {
        self.output_queue.push(data);
    }

    /// Queue text output
    pub fn queue_text(&mut self, text: &str) {
        self.queue_output(text.as_bytes().to_vec());
    }

    /// Simulate process termination
    pub fn terminate(&mut self, exit_code: i32) {
        self.is_running = false;
        self.exit_code = Some(exit_code);
    }

    /// Get all input received
    pub fn get_input(&self) -> Vec<Vec<u8>> {
        self.input_received.clone()
    }

    /// Get input as string
    pub fn get_input_text(&self) -> Vec<String> {
        self.input_received
            .iter()
            .map(|bytes| String::from_utf8_lossy(bytes).to_string())
            .collect()
    }
}

/// Type alias for send input callback to reduce complexity
type SendInputCallback = Box<dyn Fn(&str, &[u8]) + Send + Sync>;

/// Mock PTY Manager for testing
pub struct MockPtyManager {
    processes: Arc<Mutex<HashMap<String, MockPtyProcess>>>,
    pub send_input_callback: Option<SendInputCallback>,
}

impl MockPtyManager {
    /// Create a new mock PTY manager
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            send_input_callback: None,
        }
    }

    /// Spawn a new mock process
    pub fn spawn(&mut self, command: String) -> Result<PtyHandle> {
        let process = MockPtyProcess::new(command);
        let handle = process.handle.clone();

        let mut processes = self.processes.lock().unwrap();
        processes.insert(handle.id.clone(), process);

        Ok(handle)
    }

    /// Get a process by handle
    pub fn get_process(&self, handle: &PtyHandle) -> Option<MockPtyProcess> {
        let processes = self.processes.lock().unwrap();
        processes.get(&handle.id).cloned()
    }

    /// Send input to a process
    pub fn send_input(&mut self, handle: &PtyHandle, data: &[u8]) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();
        let process = processes
            .get_mut(&handle.id)
            .ok_or_else(|| Error::PtyHandleNotFound {
                handle_id: handle.id.clone(),
            })?;

        process.input_received.push(data.to_vec());

        // Call callback if set
        if let Some(ref callback) = self.send_input_callback {
            callback(&handle.id, data);
        }

        Ok(())
    }

    /// Read output from a process
    pub fn try_read_output(&mut self, handle: &PtyHandle) -> Result<Vec<u8>> {
        let mut processes = self.processes.lock().unwrap();
        let process = processes
            .get_mut(&handle.id)
            .ok_or_else(|| Error::PtyHandleNotFound {
                handle_id: handle.id.clone(),
            })?;

        if process.output_queue.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(process.output_queue.remove(0))
        }
    }

    /// Queue output for a process
    pub fn queue_output(&mut self, handle: &PtyHandle, data: Vec<u8>) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();
        let process = processes
            .get_mut(&handle.id)
            .ok_or_else(|| Error::PtyHandleNotFound {
                handle_id: handle.id.clone(),
            })?;

        process.output_queue.push(data);
        Ok(())
    }

    /// Queue text output for a process
    pub fn queue_text(&mut self, handle: &PtyHandle, text: &str) -> Result<()> {
        self.queue_output(handle, text.as_bytes().to_vec())
    }

    /// Terminate a process
    pub fn terminate(&mut self, handle: &PtyHandle, exit_code: i32) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();
        let process = processes
            .get_mut(&handle.id)
            .ok_or_else(|| Error::PtyHandleNotFound {
                handle_id: handle.id.clone(),
            })?;

        process.terminate(exit_code);
        Ok(())
    }

    /// Get process info
    pub fn get_info(&self, handle: &PtyHandle) -> Result<PtyInfo> {
        let processes = self.processes.lock().unwrap();
        let process = processes
            .get(&handle.id)
            .ok_or_else(|| Error::PtyHandleNotFound {
                handle_id: handle.id.clone(),
            })?;

        Ok(PtyInfo {
            id: handle.id.clone(),
            pid: Some(12345), // Mock PID
            command: process.command.clone(),
            working_directory: PathBuf::from("/tmp"),
            start_time: chrono::Utc::now(),
            is_alive: process.is_running,
        })
    }

    /// Get count of active processes
    pub fn active_count(&self) -> usize {
        let processes = self.processes.lock().unwrap();
        processes.values().filter(|p| p.is_running).count()
    }

    /// Clean up terminated processes
    pub fn cleanup_terminated(&mut self) -> usize {
        let mut processes = self.processes.lock().unwrap();
        let before_count = processes.len();

        processes.retain(|_, p| p.is_running);

        before_count - processes.len()
    }

    /// Set a callback for when input is sent
    pub fn set_input_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, &[u8]) + Send + Sync + 'static,
    {
        self.send_input_callback = Some(Box::new(callback));
    }
}

impl Default for MockPtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_pty_process_creation() {
        let process = MockPtyProcess::new("echo test".to_string());
        assert_eq!(process.command, "echo test");
        assert!(process.is_running);
        assert_eq!(process.exit_code, None);
    }

    #[test]
    fn test_mock_pty_process_output() {
        let mut process = MockPtyProcess::new("test".to_string());
        process.queue_text("Hello, World!");

        assert_eq!(process.output_queue.len(), 1);
        assert_eq!(
            String::from_utf8_lossy(&process.output_queue[0]),
            "Hello, World!"
        );
    }

    #[test]
    fn test_mock_pty_manager_spawn() {
        let mut manager = MockPtyManager::new();
        let handle = manager.spawn("ls -la".to_string()).unwrap();

        assert!(manager.get_process(&handle).is_some());
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_mock_pty_manager_input() {
        let mut manager = MockPtyManager::new();
        let handle = manager.spawn("cat".to_string()).unwrap();

        manager.send_input(&handle, b"test input").unwrap();

        let process = manager.get_process(&handle).unwrap();
        assert_eq!(process.input_received.len(), 1);
        assert_eq!(process.get_input_text()[0], "test input");
    }

    #[test]
    fn test_mock_pty_manager_output() {
        let mut manager = MockPtyManager::new();
        let handle = manager.spawn("echo".to_string()).unwrap();

        manager.queue_text(&handle, "output text").unwrap();

        let output = manager.try_read_output(&handle).unwrap();
        assert_eq!(String::from_utf8_lossy(&output), "output text");
    }

    #[test]
    fn test_mock_pty_manager_terminate() {
        let mut manager = MockPtyManager::new();
        let handle = manager.spawn("sleep 100".to_string()).unwrap();

        assert_eq!(manager.active_count(), 1);

        manager.terminate(&handle, 0).unwrap();

        let process = manager.get_process(&handle).unwrap();
        assert!(!process.is_running);
        assert_eq!(process.exit_code, Some(0));
    }

    #[test]
    fn test_mock_pty_manager_cleanup() {
        let mut manager = MockPtyManager::new();
        let handle1 = manager.spawn("cmd1".to_string()).unwrap();
        let _handle2 = manager.spawn("cmd2".to_string()).unwrap();

        manager.terminate(&handle1, 0).unwrap();

        let cleaned = manager.cleanup_terminated();
        assert_eq!(cleaned, 1);
        assert_eq!(manager.active_count(), 1);
    }
}
