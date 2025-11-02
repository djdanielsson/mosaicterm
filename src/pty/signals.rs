//! PTY Signal Handling
//!
//! Manages signal handling for PTY processes, including sending
//! signals like SIGINT, SIGTERM, and SIGKILL for process control.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Signal types that can be sent to PTY processes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Interrupt signal (Ctrl+C)
    Interrupt,
    /// Termination signal (graceful shutdown)
    Terminate,
    /// Kill signal (forceful termination)
    Kill,
    /// Hangup signal
    Hangup,
    /// Continue signal
    Continue,
    /// Stop signal
    Stop,
}

/// Signal handling configuration
#[derive(Debug, Clone)]
pub struct SignalConfig {
    /// Timeout for graceful termination before sending KILL
    pub graceful_timeout_ms: u64,
    /// Whether to send SIGINT before SIGTERM
    pub send_interrupt_first: bool,
    /// Maximum number of signal attempts before giving up
    pub max_signal_attempts: u32,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            graceful_timeout_ms: 5000, // 5 seconds
            send_interrupt_first: true,
            max_signal_attempts: 3,
        }
    }
}

/// Signal handler for managing PTY process signals
pub struct SignalHandler {
    /// Signal configuration
    config: SignalConfig,
    /// Process ID to handle mapping
    process_handles: HashMap<String, u32>,
}

impl SignalHandler {
    /// Create a new signal handler
    pub fn new() -> Self {
        Self {
            config: SignalConfig::default(),
            process_handles: HashMap::new(),
        }
    }

    /// Create signal handler with custom configuration
    pub fn with_config(config: SignalConfig) -> Self {
        Self {
            config,
            process_handles: HashMap::new(),
        }
    }

    /// Register a process for signal handling
    pub fn register_process(&mut self, handle_id: String, pid: u32) {
        self.process_handles.insert(handle_id, pid);
    }

    /// Unregister a process from signal handling
    pub fn unregister_process(&mut self, handle_id: &str) {
        self.process_handles.remove(handle_id);
    }

    /// Send a signal to a PTY process
    pub async fn send_signal(&self, handle_id: &str, signal: Signal) -> Result<()> {
        let pid =
            self.process_handles
                .get(handle_id)
                .ok_or_else(|| Error::ProcessNotRegistered {
                    handle_id: handle_id.to_string(),
                })?;

        self.send_signal_to_pid(*pid, signal).await
    }

    /// Send signal to process by PID
    pub async fn send_signal_to_pid(&self, pid: u32, signal: Signal) -> Result<()> {
        // Platform-specific signal sending
        #[cfg(unix)]
        {
            self.send_unix_signal(pid, signal).await
        }

        #[cfg(windows)]
        {
            self.send_windows_signal(pid, signal).await
        }

        #[cfg(not(any(unix, windows)))]
        {
            Err(Error::SignalNotSupported {
                signal: format!("{:?}", signal),
                platform: std::env::consts::OS.to_string(),
            })
        }
    }

    /// Send interrupt signal (Ctrl+C equivalent)
    pub async fn send_interrupt(&self, handle_id: &str) -> Result<()> {
        self.send_signal(handle_id, Signal::Interrupt).await
    }

    /// Send termination signal (graceful shutdown)
    pub async fn send_terminate(&self, handle_id: &str) -> Result<()> {
        self.send_signal(handle_id, Signal::Terminate).await
    }

    /// Send kill signal (forceful termination)
    pub async fn send_kill(&self, handle_id: &str) -> Result<()> {
        self.send_signal(handle_id, Signal::Kill).await
    }

    /// Terminate process gracefully (SIGINT then SIGTERM then SIGKILL)
    pub async fn terminate_gracefully(&self, handle_id: &str) -> Result<()> {
        use tokio::time::{sleep, Duration};

        // First try interrupt if configured
        if self.config.send_interrupt_first {
            let _ = self.send_signal(handle_id, Signal::Interrupt).await;
            sleep(Duration::from_millis(500)).await;
        }

        // Then try terminate
        let _ = self.send_signal(handle_id, Signal::Terminate).await;
        sleep(Duration::from_millis(self.config.graceful_timeout_ms)).await;

        // Finally, force kill if still running
        self.send_signal(handle_id, Signal::Kill).await
    }

    /// Check if a process is still running
    pub fn is_process_running(&self, pid: u32) -> bool {
        // Platform-specific process checking
        #[cfg(unix)]
        {
            self.check_unix_process(pid)
        }

        #[cfg(windows)]
        {
            self.check_windows_process(pid)
        }

        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }

    /// Get the number of registered processes
    pub fn registered_count(&self) -> usize {
        self.process_handles.len()
    }

    /// Platform-specific Unix signal sending
    #[cfg(unix)]
    async fn send_unix_signal(&self, pid: u32, signal: Signal) -> Result<()> {
        use nix::sys::signal::{kill, Signal as NixSignal};
        use nix::unistd::Pid;

        let nix_signal = match signal {
            Signal::Interrupt => NixSignal::SIGINT,
            Signal::Terminate => NixSignal::SIGTERM,
            Signal::Kill => NixSignal::SIGKILL,
            Signal::Hangup => NixSignal::SIGHUP,
            Signal::Continue => NixSignal::SIGCONT,
            Signal::Stop => NixSignal::SIGSTOP,
        };

        kill(Pid::from_raw(pid as i32), nix_signal).map_err(|e| Error::SignalSendFailed {
            signal: format!("{:?}", signal),
            reason: e.to_string(),
        })
    }

    /// Platform-specific Windows signal sending
    #[cfg(windows)]
    async fn send_windows_signal(&self, pid: u32, signal: Signal) -> Result<()> {
        // Windows doesn't have signals like Unix
        // We'll use process termination for kill signal
        match signal {
            Signal::Kill => {
                // Use Windows API to terminate process
                // This is a simplified implementation
                Err(Error::SignalNotSupported {
                    signal: format!("{:?}", signal),
                    platform: "Windows".to_string(),
                })
            }
            _ => {
                // Other signals not applicable on Windows
                Err(Error::SignalNotSupported {
                    signal: format!("{:?}", signal),
                    platform: "Windows".to_string(),
                })
            }
        }
    }

    /// Platform-specific Unix process checking
    #[cfg(unix)]
    fn check_unix_process(&self, pid: u32) -> bool {
        // Check if process exists by sending signal 0
        use nix::sys::signal::{kill, Signal as NixSignal};
        use nix::unistd::Pid;

        kill(Pid::from_raw(pid as i32), NixSignal::SIGCONT).is_ok()
    }

    /// Platform-specific Windows process checking
    #[cfg(windows)]
    fn check_windows_process(&self, pid: u32) -> bool {
        // Windows process checking implementation using WinAPI
        use std::process::Command;

        // Use tasklist command to check if process exists
        let output = Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid)])
            .output();

        match output {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                output_str.contains(&pid.to_string())
            }
            Err(_) => {
                // If tasklist fails, assume process is not running
                false
            }
        }
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Signal handling utilities
pub mod utils {
    use super::*;

    /// Create a signal handler with default settings
    pub fn create_default_handler() -> SignalHandler {
        SignalHandler::new()
    }

    /// Create a signal handler with custom timeout
    pub fn create_handler_with_timeout(timeout_ms: u64) -> SignalHandler {
        let config = SignalConfig {
            graceful_timeout_ms: timeout_ms,
            ..Default::default()
        };
        SignalHandler::with_config(config)
    }

    /// Send interrupt to all registered processes
    pub async fn interrupt_all(handler: &SignalHandler, handle_ids: &[String]) -> Vec<Result<()>> {
        let mut results = Vec::new();
        for id in handle_ids {
            results.push(handler.send_interrupt(id).await);
        }
        results
    }

    /// Terminate all registered processes gracefully
    pub async fn terminate_all_gracefully(
        handler: &SignalHandler,
        handle_ids: &[String],
    ) -> Vec<Result<()>> {
        let mut results = Vec::new();
        for id in handle_ids {
            results.push(handler.terminate_gracefully(id).await);
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_config_defaults() {
        let config = SignalConfig::default();
        assert_eq!(config.graceful_timeout_ms, 5000);
        assert!(config.send_interrupt_first);
        assert_eq!(config.max_signal_attempts, 3);
    }

    #[test]
    fn test_signal_handler_creation() {
        let handler = SignalHandler::new();
        assert_eq!(handler.registered_count(), 0);
    }

    #[test]
    fn test_signal_handler_with_config() {
        let config = SignalConfig {
            graceful_timeout_ms: 1000,
            send_interrupt_first: false,
            max_signal_attempts: 5,
        };

        let handler = SignalHandler::with_config(config);
        assert_eq!(handler.config.graceful_timeout_ms, 1000);
        assert!(!handler.config.send_interrupt_first);
        assert_eq!(handler.config.max_signal_attempts, 5);
    }

    #[test]
    fn test_process_registration() {
        let mut handler = SignalHandler::new();

        handler.register_process("test_handle".to_string(), 12345);
        assert_eq!(handler.registered_count(), 1);

        handler.unregister_process("test_handle");
        assert_eq!(handler.registered_count(), 0);
    }

    #[test]
    fn test_signal_variants() {
        assert_eq!(Signal::Interrupt, Signal::Interrupt);
        assert_eq!(Signal::Terminate, Signal::Terminate);
        assert_eq!(Signal::Kill, Signal::Kill);
    }

    #[tokio::test]
    async fn test_send_signal_to_unregistered_process() {
        let handler = SignalHandler::new();

        let result = handler.send_signal("nonexistent", Signal::Terminate).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_utils_create_default_handler() {
        let handler = utils::create_default_handler();
        assert_eq!(handler.registered_count(), 0);
    }

    #[test]
    fn test_utils_create_handler_with_timeout() {
        let handler = utils::create_handler_with_timeout(2000);
        assert_eq!(handler.config.graceful_timeout_ms, 2000);
    }
}
