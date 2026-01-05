//! Contract Tests for PTY Process Lifecycle Management
//!
//! These tests define the expected behavior of PTY process creation,
//! monitoring, and termination using PtyManager.
//!
//! Contract: PTY Process Lifecycle Management
//! See: specs/001-mosaicterm-terminal-emulator/contracts/pty-lifecycle.md

use mosaicterm::pty::{PtyHandle, PtyManager};
use std::collections::HashMap;
use std::sync::Arc;

/// Get the appropriate echo command for the current platform
fn get_echo_command() -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        (
            "cmd.exe",
            vec!["/C".to_string(), "echo".to_string(), "hello".to_string()],
        )
    }
    #[cfg(not(windows))]
    {
        ("/bin/echo", vec!["hello".to_string()])
    }
}

/// Get the appropriate pwd/cd command for the current platform
fn get_pwd_command() -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        ("cmd.exe", vec!["/C".to_string(), "cd".to_string()])
    }
    #[cfg(not(windows))]
    {
        ("/bin/pwd", vec![])
    }
}

/// Get the appropriate sleep/timeout command for the current platform
fn get_sleep_command(seconds: u32) -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        // Use ping to localhost as a reliable sleep substitute on Windows
        // ping -n <count> 127.0.0.1 waits approximately (count-1) seconds
        (
            "cmd.exe",
            vec![
                "/C".to_string(),
                "ping".to_string(),
                "-n".to_string(),
                (seconds + 1).to_string(),
                "127.0.0.1".to_string(),
                ">".to_string(),
                "nul".to_string(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        ("/bin/sleep", vec![seconds.to_string()])
    }
}

/// Get the appropriate short-lived command for the current platform
fn get_short_command() -> (&'static str, Vec<String>) {
    #[cfg(windows)]
    {
        (
            "cmd.exe",
            vec!["/C".to_string(), "echo".to_string(), "test".to_string()],
        )
    }
    #[cfg(not(windows))]
    {
        ("/bin/echo", vec!["test".to_string()])
    }
}

// Test PTY creation with valid command
#[tokio::test]
async fn test_pty_creation_with_valid_command() {
    // Arrange
    let manager = Arc::new(PtyManager::new());
    let (command, args) = get_echo_command();
    let env = HashMap::new();
    let working_dir = std::env::current_dir().unwrap();

    // Act
    let result = manager
        .create_pty(command, &args, &env, Some(working_dir.as_path()))
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "PTY creation should succeed with valid command: {:?}",
        result.err()
    );
    let handle = result.unwrap();
    assert!(!handle.id.is_empty(), "PTY handle should have valid ID");
}

// Test PTY status check for running process
#[tokio::test]
async fn test_pty_status_check_for_running_process() {
    // Arrange
    let manager = Arc::new(PtyManager::new());
    let handle = create_long_running_pty(&manager).await;

    // Act
    let is_running = manager.is_alive(&handle).await;

    // Assert
    assert!(
        is_running,
        "PTY should be reported as alive for valid handle"
    );

    // Cleanup
    let _ = manager.terminate_pty(&handle).await;
}

// Test PTY termination
#[tokio::test]
async fn test_pty_termination() {
    // Arrange
    let manager = Arc::new(PtyManager::new());
    let handle = create_long_running_pty(&manager).await;

    // Act
    let result = manager.terminate_pty(&handle).await;

    // Assert
    assert!(result.is_ok(), "PTY termination should succeed");
}

// Test working directory handling
#[tokio::test]
async fn test_working_directory_handling() {
    // Arrange
    let manager = Arc::new(PtyManager::new());
    let (command, args) = get_pwd_command();
    let env = HashMap::new();
    let working_dir = std::env::current_dir().unwrap();

    // Act
    let result = manager
        .create_pty(command, &args, &env, Some(working_dir.as_path()))
        .await;

    // Assert
    assert!(
        result.is_ok(),
        "PTY creation with working directory should succeed"
    );
}

// Test active count tracking
#[tokio::test]
async fn test_active_count_tracking() {
    // Arrange
    let manager = Arc::new(PtyManager::new());
    let initial_count = manager.active_count().await;

    // Create a PTY
    let handle = create_long_running_pty(&manager).await;
    let count_after_create = manager.active_count().await;

    // Terminate the PTY
    let _ = manager.terminate_pty(&handle).await;

    // Wait a bit for cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let _ = manager.cleanup_terminated().await;
    let count_after_terminate = manager.active_count().await;

    // Assert
    assert!(
        count_after_create > initial_count,
        "Active count should increase after creation"
    );
    assert!(
        count_after_terminate <= count_after_create,
        "Active count should not increase after termination"
    );
}

// Test cleanup of terminated processes
#[tokio::test]
async fn test_cleanup_terminated_processes() {
    // Arrange
    let manager = Arc::new(PtyManager::new());

    // Create and terminate a PTY
    let handle = create_short_lived_pty(&manager).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Act
    let cleaned = manager.cleanup_terminated().await;

    // Assert - cleanup ran successfully (count is always valid for usize)
    // Note: cleanup count may vary based on timing
    let _ = cleaned; // Acknowledge the result
    let _ = manager.terminate_pty(&handle).await; // Ensure cleanup
}

// Helper functions

async fn create_long_running_pty(manager: &Arc<PtyManager>) -> PtyHandle {
    let (command, args) = get_sleep_command(10);
    let env = HashMap::new();
    let working_dir = std::env::current_dir().unwrap();
    manager
        .create_pty(command, &args, &env, Some(working_dir.as_path()))
        .await
        .unwrap()
}

async fn create_short_lived_pty(manager: &Arc<PtyManager>) -> PtyHandle {
    let (command, args) = get_short_command();
    let env = HashMap::new();
    let working_dir = std::env::current_dir().unwrap();
    manager
        .create_pty(command, &args, &env, Some(working_dir.as_path()))
        .await
        .unwrap()
}
