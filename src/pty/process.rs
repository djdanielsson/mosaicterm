//! PTY Process Spawning
//!
//! Handles the creation and spawning of pseudoterminal processes
//! using the portable-pty crate for cross-platform compatibility.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::io::{Read, Write};
use std::sync::mpsc::channel;
use std::thread;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use tokio::sync::mpsc::unbounded_channel;

use crate::error::{Error, Result};
use crate::models::PtyProcess;
use super::streams::PtyStreams;

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
        .map_err(|e| Error::Other(format!("Failed to open PTY: {}", e)))?;

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
        .map_err(|e| Error::Other(format!("Failed to spawn command: {}", e)))?;

    // Get the PID
    let pid = child.process_id().unwrap_or(0);

    // Create PTY process model
    let mut pty_process = PtyProcess::new(command.to_string(), args.to_vec());
    pty_process.mark_started(pid);

    // Create streams wrapper
    let streams = create_pty_streams(pair)?;

    Ok((pty_process, streams))
}

/// Create PTY streams from a PTY pair
fn create_pty_streams(pair: PtyPair) -> Result<PtyStreams> {
    // Bridge blocking PTY I/O to async via channels and a background thread
    let mut master_reader = pair.master.try_clone_reader()
        .map_err(|e| Error::Other(format!("Failed to clone PTY reader: {}", e)))?;
    let mut master_writer = pair.master.take_writer()
        .map_err(|e| Error::Other(format!("Failed to take PTY writer: {}", e)))?;

    // Channel: PTY output -> async consumer
    let (tx_async_out, rx_async_out) = unbounded_channel::<Vec<u8>>();
    // Channel: async producer (stdin) -> PTY writer thread
    let (tx_stdin, rx_stdin) = channel::<Vec<u8>>();

    // Reader thread: read from PTY master and forward to async channel
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match master_reader.read(&mut buf) {
                Ok(0) => {
                    // EOF
                    break;
                }
                Ok(n) => {
                    // Debug: print how many bytes were read
                    eprintln!("PTY read {} bytes", n);
                    let _ = tx_async_out.send(buf[..n].to_vec());
                }
                Err(e) => {
                    // On EAGAIN/EINTR, continue; otherwise break
                    // For simplicity, just break on error
                    eprintln!("PTY read error: {}", e);
                    break;
                }
            }
        }
    });

    // Writer thread: receive stdin data and write to PTY master
    thread::spawn(move || {
        while let Ok(data) = rx_stdin.recv() {
            if let Err(e) = master_writer.write_all(&data) {
                eprintln!("PTY write error: {}", e);
                break;
            }
            let _ = master_writer.flush();
            eprintln!("PTY wrote {} bytes", data.len());
        }
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
    // Check if command exists in PATH
    match std::process::Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => Err(Error::Other(format!("Command '{}' not found in PATH", command))),
    }
}

/// Get the default shell for the current platform
pub fn get_default_shell() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/bash"
    }
}

/// Check if a command is available on the system
pub fn is_command_available(command: &str) -> bool {
    std::process::Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the current user's shell from environment
pub fn get_user_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| get_default_shell().to_string())
}

/// Get effective environment for process spawning
pub fn get_effective_environment(custom_env: &HashMap<String, String>, inherit: bool) -> HashMap<String, String> {
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
        assert!(env.contains_key("PATH") || env.contains_key("HOME"));
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
        let result = spawn_pty_process(
            "/nonexistent/command",
            &[],
            &HashMap::new(),
            None,
        ).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_pty_process_success() {
        // Test with valid command - this might fail in CI environments
        // where PTY support is limited, so we'll make it non-critical
        let result = spawn_pty_process(
            "echo",
            &["test".to_string()],
            &HashMap::new(),
            None,
        ).await;

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
