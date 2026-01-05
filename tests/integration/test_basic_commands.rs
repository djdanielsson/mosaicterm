//! Integration Tests for Basic Command Execution
//!
//! These tests verify that basic shell commands work correctly
//! in the MosaicTerm environment.

use mosaicterm::execution::DirectExecutor;

#[tokio::test]
async fn test_echo_command() {
    // Test that echo command works
    let executor = DirectExecutor::new();
    let result = executor.execute_command("echo hello world").await;

    assert!(result.is_ok());
    let command_block = result.unwrap();
    assert_eq!(command_block.command, "echo hello world");
    assert!(!command_block.output.is_empty());

    // Check that output contains the expected text
    let output_text: String = command_block
        .output
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(output_text.contains("hello world"));
}

#[tokio::test]
async fn test_pwd_command() {
    // Test that pwd command works
    let executor = DirectExecutor::new();
    let result = executor.execute_command("pwd").await;

    assert!(result.is_ok());
    let command_block = result.unwrap();
    assert_eq!(command_block.command, "pwd");
    assert!(!command_block.output.is_empty());

    // Output should contain a path
    let output_text: String = command_block
        .output
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(output_text.contains("/"));
}

#[tokio::test]
async fn test_ls_command() {
    // Test that ls command works
    let executor = DirectExecutor::new();
    let result = executor.execute_command("ls").await;

    assert!(result.is_ok());
    let command_block = result.unwrap();
    assert_eq!(command_block.command, "ls");
    // ls might have empty output in some directories, so just check it doesn't error
}

#[tokio::test]
async fn test_whoami_command() {
    // Test that whoami command works
    let executor = DirectExecutor::new();
    let result = executor.execute_command("whoami").await;

    assert!(result.is_ok());
    let command_block = result.unwrap();
    assert_eq!(command_block.command, "whoami");
    assert!(!command_block.output.is_empty());

    // Output should contain a username
    let output_text: String = command_block
        .output
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!output_text.trim().is_empty());
}

#[tokio::test]
async fn test_invalid_command() {
    // Test that invalid commands are handled gracefully
    let executor = DirectExecutor::new();
    let result = executor.execute_command("nonexistent_command_xyz").await;

    assert!(result.is_ok()); // Should not panic, but command will fail
    let command_block = result.unwrap();
    assert_eq!(command_block.command, "nonexistent_command_xyz");

    // Should have some error output or indication of failure
    // The exact behavior depends on the shell/OS
}

#[test]
fn test_direct_execution_detection() {
    // Test that appropriate commands are detected for direct execution using static method
    assert!(DirectExecutor::check_direct_execution("ls -la"));
    assert!(DirectExecutor::check_direct_execution("pwd"));
    assert!(DirectExecutor::check_direct_execution("echo hello"));
    assert!(DirectExecutor::check_direct_execution("whoami"));

    // These should not use direct execution
    assert!(!DirectExecutor::check_direct_execution("vim file.txt"));
    assert!(!DirectExecutor::check_direct_execution("ssh user@host"));
    assert!(!DirectExecutor::check_direct_execution("top"));
}

#[test]
fn test_executor_configuration() {
    // Test that executor can be configured
    let mut executor = DirectExecutor::new();

    // Test setting working directory
    executor.set_working_dir(std::path::PathBuf::from("/tmp"));

    // Test setting environment variables
    executor.set_env("TEST_VAR".to_string(), "test_value".to_string());

    // Just test that configuration doesn't panic
}
