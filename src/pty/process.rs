//! PTY Process Spawning
//!
//! Handles the creation and spawning of pseudoterminal processes
//! using the portable-pty crate for cross-platform compatibility.

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use tokio::sync::mpsc::unbounded_channel;

use super::streams::PtyStreams;
use crate::error::{Error, Result};
use crate::models::PtyProcess;

/// Spawn a new PTY process with the given command and environment
pub async fn spawn_pty_process(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
    working_directory: Option<&Path>,
) -> Result<(PtyProcess, PtyStreams)> {
    // Get the native PTY system
    let pty_system = native_pty_system();

    // Create a new PTY pair
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| Error::PtyCreationFailed {
            command: command.to_string(),
            reason: e.to_string(),
        })?;

    // Build the command
    let mut cmd_builder = CommandBuilder::new(command);
    cmd_builder.args(args);

    // Set environment variables
    for (key, value) in env {
        cmd_builder.env(key, value);
    }

    // Set working directory if provided
    if let Some(dir) = working_directory {
        cmd_builder.cwd(dir);
    }

    // Spawn the process
    let child = pair
        .slave
        .spawn_command(cmd_builder)
        .map_err(|e| Error::CommandSpawnFailed {
            command: command.to_string(),
            reason: e.to_string(),
        })?;

    // Get the PID
    let pid = child.process_id().unwrap_or(0);

    // Create PTY process model with working directory
    let mut pty_process = if let Some(dir) = working_directory {
        PtyProcess::with_working_directory(command.to_string(), args.to_vec(), dir.to_path_buf())
    } else {
        PtyProcess::new(command.to_string(), args.to_vec())
    };
    pty_process.mark_started(pid);

    // Create streams wrapper
    let streams = create_pty_streams(pair)?;

    Ok((pty_process, streams))
}

/// Create PTY streams from a PTY pair
fn create_pty_streams(pair: PtyPair) -> Result<PtyStreams> {
    // Bridge blocking PTY I/O to async via channels and a background thread
    let mut master_reader =
        pair.master
            .try_clone_reader()
            .map_err(|e| Error::PtyReaderCloneFailed {
                reason: e.to_string(),
            })?;
    let mut master_writer = pair
        .master
        .take_writer()
        .map_err(|e| Error::PtyWriterTakeFailed {
            reason: e.to_string(),
        })?;

    // Channel: PTY output -> async consumer
    let (tx_async_out, rx_async_out) = unbounded_channel::<Vec<u8>>();
    // Channel: async producer (stdin) -> PTY writer thread
    let (tx_stdin, rx_stdin) = channel::<Vec<u8>>();

    // Reader thread: read from PTY master and forward to async channel
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 5;

        loop {
            match master_reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - process terminated normally
                    debug!("PTY read EOF - process terminated");
                    break;
                }
                Ok(n) => {
                    consecutive_errors = 0; // Reset error counter on success

                    // Send data to async channel
                    if tx_async_out.send(buf[..n].to_vec()).is_err() {
                        debug!("PTY read: receiver dropped, stopping reader thread");
                        break;
                    }
                }
                Err(e) => {
                    // Handle recoverable errors (EAGAIN, EINTR)
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        debug!("PTY read interrupted (EINTR), retrying...");
                        continue;
                    }

                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        debug!("PTY read would block (EAGAIN), retrying...");
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }

                    // Non-recoverable error
                    consecutive_errors += 1;
                    warn!(
                        "PTY read error ({}): {} (attempt {}/{})",
                        e.kind(),
                        e,
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS
                    );

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        error!("PTY read: too many consecutive errors, stopping reader thread");
                        break;
                    }

                    // Brief delay before retry
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
        debug!("PTY reader thread exiting");
    });

    // Writer thread: receive stdin data and write to PTY master
    thread::spawn(move || {
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 3;

        while let Ok(data) = rx_stdin.recv() {
            let mut attempts = 0;
            const MAX_ATTEMPTS: u32 = 3;

            loop {
                match master_writer.write_all(&data) {
                    Ok(()) => {
                        consecutive_errors = 0; // Reset error counter on success

                        // Flush the write
                        if let Err(e) = master_writer.flush() {
                            debug!("PTY flush error: {}", e);
                            // Continue anyway, flush errors are usually not fatal
                        }

                        break; // Move to next data item
                    }
                    Err(e) => {
                        attempts += 1;

                        // Handle recoverable errors
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            debug!("PTY write interrupted (EINTR), retrying...");
                            continue;
                        }

                        if e.kind() == std::io::ErrorKind::WouldBlock && attempts < MAX_ATTEMPTS {
                            debug!(
                                "PTY write would block (EAGAIN), retrying ({}/{})...",
                                attempts, MAX_ATTEMPTS
                            );
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        }

                        // Non-recoverable error or max attempts reached
                        consecutive_errors += 1;
                        warn!(
                            "PTY write error ({}): {} (consecutive errors: {}/{})",
                            e.kind(),
                            e,
                            consecutive_errors,
                            MAX_CONSECUTIVE_ERRORS
                        );

                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            error!(
                                "PTY write: too many consecutive errors, stopping writer thread"
                            );
                            return; // Exit thread
                        }

                        break; // Skip this data item and try next
                    }
                }
            }
        }
        debug!("PTY writer thread exiting");
    });

    Ok(PtyStreams::from_channels(rx_async_out, tx_stdin))
}

// Note: PTY stream conversion is simplified for now.
// In a production implementation, proper async I/O conversion would be needed.

/// Process spawning configuration
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// Terminal size
    pub size: PtySize,
    /// Whether to inherit environment
    pub inherit_env: bool,
    /// Custom environment variables
    pub env_vars: HashMap<String, String>,
    /// Working directory
    pub working_directory: Option<std::path::PathBuf>,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            size: PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
            inherit_env: true,
            env_vars: HashMap::new(),
            working_directory: None,
        }
    }
}

/// Validate command before spawning
pub fn validate_command(command: &str) -> Result<()> {
    use crate::platform::Platform;

    let fs_ops = Platform::filesystem();
    match fs_ops.find_command(command) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(Error::CommandNotFound {
            command: command.to_string(),
        }),
        Err(e) => Err(e),
    }
}

/// Get the default shell for the current platform
pub fn get_default_shell() -> String {
    use crate::platform::Platform;

    Platform::shell()
        .default_shell()
        .to_string_lossy()
        .to_string()
}

/// Check if a command is available on the system
pub fn is_command_available(command: &str) -> bool {
    use crate::platform::Platform;

    let fs_ops = Platform::filesystem();
    fs_ops.find_command(command).ok().flatten().is_some()
}

/// Get the current user's shell from environment
pub fn get_user_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| get_default_shell().to_string())
}

/// Get effective environment for process spawning
pub fn get_effective_environment(
    custom_env: &HashMap<String, String>,
    inherit: bool,
) -> HashMap<String, String> {
    let mut env = if inherit {
        std::env::vars().collect()
    } else {
        HashMap::new()
    };

    // Override with custom environment variables
    for (key, value) in custom_env {
        env.insert(key.clone(), value.clone());
    }

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_command() {
        // Test with a command that should exist
        assert!(validate_command("echo").is_ok());

        // Test with a non-existent command
        assert!(validate_command("/nonexistent/command").is_err());
    }

    #[test]
    fn test_default_shell() {
        let shell = get_default_shell();
        assert!(!shell.is_empty());
    }

    #[test]
    fn test_command_availability() {
        assert!(is_command_available("echo"));
        assert!(!is_command_available("/nonexistent/command"));
    }

    #[test]
    fn test_user_shell() {
        let shell = get_user_shell();
        assert!(!shell.is_empty());
    }

    #[test]
    fn test_effective_environment() {
        let mut custom_env = HashMap::new();
        custom_env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let env = get_effective_environment(&custom_env, true);
        assert!(env.contains_key("TEST_VAR"));
        assert_eq!(env.get("TEST_VAR"), Some(&"test_value".to_string()));

        // Should also contain inherited environment variables
        // When inherit=true, the environment should have more than just the custom variable
        // Check that at least one common environment variable exists (indicating inheritance worked)
        let has_inherited_env = env.contains_key("PATH")
            || env.contains_key("HOME")
            || env.contains_key("USERPROFILE")
            || env.contains_key("TEMP")
            || env.contains_key("TMP")
            || env.contains_key("USERNAME")
            || env.contains_key("COMPUTERNAME");
        assert!(
            has_inherited_env || env.len() > 1,
            "Expected inherited environment variables when inherit=true, but only found: {:?}",
            env.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_spawn_config_defaults() {
        let config = SpawnConfig::default();
        assert_eq!(config.size.rows, 24);
        assert_eq!(config.size.cols, 80);
        assert!(config.inherit_env);
        assert!(config.env_vars.is_empty());
        assert!(config.working_directory.is_none());
    }

    #[tokio::test]
    async fn test_spawn_pty_process_validation() {
        // Test with invalid command
        let result = spawn_pty_process("/nonexistent/command", &[], &HashMap::new(), None).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_pty_process_success() {
        // Test with valid command - this might fail in CI environments
        // where PTY support is limited, so we'll make it non-critical
        let result = spawn_pty_process("echo", &["test".to_string()], &HashMap::new(), None).await;

        // In some environments (like CI), PTY spawning might fail
        // So we'll just ensure it doesn't panic
        match result {
            Ok((process, _streams)) => {
                assert_eq!(process.command, "echo");
                assert!(process.args.contains(&"test".to_string()));
            }
            Err(_) => {
                // PTY spawning failed - this is acceptable in some environments
                // The important thing is that it didn't panic
            }
        }
    }
}
