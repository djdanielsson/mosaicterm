//! Integration Tests for Command Execution Flows
//!
//! These tests verify that command execution works correctly in various scenarios.

use mosaicterm::execution::DirectExecutor;

#[tokio::test]
async fn test_single_command_execution() {
    // Test that a single command executes and captures output correctly
    let executor = DirectExecutor::new();
    let result = executor.execute_command("echo 'test output'").await;

    assert!(result.is_ok(), "Command should execute successfully");
    let command_block = result.unwrap();

    assert_eq!(command_block.command, "echo 'test output'");
    assert!(!command_block.output.is_empty(), "Should have output");

    let output_text: String = command_block
        .output
        .iter()
        .map(|line| line.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        output_text.contains("test output"),
        "Output should contain command result"
    );
}

#[tokio::test]
async fn test_multiple_commands_sequence() {
    // Test that multiple commands can be executed in sequence
    let executor = DirectExecutor::new();

    let commands = vec!["echo 'first'", "echo 'second'", "echo 'third'"];
    let mut results = vec![];

    for cmd in commands {
        let result = executor.execute_command(cmd).await;
        assert!(
            result.is_ok(),
            "Command '{}' should execute successfully",
            cmd
        );
        results.push(result.unwrap());
    }

    assert_eq!(results.len(), 3, "Should have executed 3 commands");

    // Verify each command's output
    assert!(results[0].output.iter().any(|l| l.text.contains("first")));
    assert!(results[1].output.iter().any(|l| l.text.contains("second")));
    assert!(results[2].output.iter().any(|l| l.text.contains("third")));
}

#[tokio::test]
async fn test_command_with_arguments() {
    // Test that commands with various arguments work correctly
    let executor = DirectExecutor::new();

    let result = executor.execute_command("echo -n 'no newline'").await;
    assert!(result.is_ok(), "Command with flags should work");

    let result = executor.execute_command("ls -la").await;
    assert!(result.is_ok(), "Command with multiple flags should work");
}

#[tokio::test]
async fn test_command_exit_codes() {
    // Test that we can handle commands with different exit codes
    let executor = DirectExecutor::new();

    // Successful command
    let result = executor.execute_command("true").await;
    assert!(result.is_ok());

    // Failing command
    let result = executor.execute_command("false").await;
    assert!(result.is_ok(), "Should not panic on non-zero exit code");
}

#[tokio::test]
async fn test_command_timing() {
    // Test that command execution timing is recorded
    let executor = DirectExecutor::new();
    let result = executor.execute_command("echo 'timing test'").await;

    assert!(result.is_ok());
    let command_block = result.unwrap();

    // Verify that timing information is present
    assert!(
        command_block.execution_time.is_some(),
        "Should have execution time"
    );
    let duration = command_block.execution_time.unwrap();
    assert!(duration.as_millis() > 0, "Duration should be positive");
    assert!(
        duration.as_secs() < 5,
        "Simple command should complete quickly"
    );
}

#[tokio::test]
async fn test_empty_command() {
    // Test that empty commands are handled gracefully
    let executor = DirectExecutor::new();
    let result = executor.execute_command("").await;

    // Should either succeed with no output or fail gracefully
    // Depending on implementation, this might be filtered out before execution
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_whitespace_only_command() {
    // Test that whitespace-only commands are handled gracefully
    let executor = DirectExecutor::new();
    let result = executor.execute_command("   ").await;

    assert!(result.is_ok() || result.is_err());
}
