//! Integration Tests for Large Output Handling
//!
//! These tests verify that the terminal can handle commands with large amounts of output.

use mosaicterm::execution::DirectExecutor;

#[tokio::test]
async fn test_moderate_output() {
    // Test command with moderate output (100 lines)
    let executor = DirectExecutor::new();
    let result = executor
        .execute_command("for i in $(seq 1 100); do echo \"Line $i\"; done")
        .await;

    assert!(result.is_ok(), "Should handle 100 lines of output");
    let command_block = result.unwrap();

    assert!(!command_block.output.is_empty(), "Should have output");
    assert!(
        command_block.output.len() <= 100,
        "Should have roughly 100 lines (or less if truncated)"
    );
}

#[tokio::test]
async fn test_large_output() {
    // Test command with large output (1000 lines)
    let executor = DirectExecutor::new();
    let result = executor
        .execute_command("for i in $(seq 1 1000); do echo \"Line $i\"; done")
        .await;

    assert!(result.is_ok(), "Should handle 1000 lines of output");
    let command_block = result.unwrap();

    assert!(!command_block.output.is_empty(), "Should have output");
    // May be truncated based on limits
}

#[tokio::test]
async fn test_very_long_single_line() {
    // Test command with one very long line
    let executor = DirectExecutor::new();
    let long_string = "x".repeat(5000);
    let result = executor
        .execute_command(&format!("echo '{}'", long_string))
        .await;

    assert!(result.is_ok(), "Should handle long single line");
    let command_block = result.unwrap();

    if !command_block.output.is_empty() {
        let first_line = &command_block.output[0].text;
        // Line might be truncated
        assert!(
            first_line.len() <= 10100,
            "Very long lines should be truncated or handled"
        );
    }
}

#[tokio::test]
async fn test_binary_output_handling() {
    // Test that binary output doesn't crash the system
    // Using 'cat' on the binary executable
    let executor = DirectExecutor::new();
    let result = executor.execute_command("head -c 100 /bin/ls").await;

    // Should not panic, even with binary data
    assert!(result.is_ok(), "Should handle binary output gracefully");
}

#[tokio::test]
async fn test_rapid_output() {
    // Test command that outputs rapidly
    let executor = DirectExecutor::new();
    let result = executor
        .execute_command("for i in $(seq 1 50); do echo \"Fast $i\"; done")
        .await;

    assert!(result.is_ok(), "Should handle rapid output");
    let command_block = result.unwrap();
    assert!(!command_block.output.is_empty());
}

#[tokio::test]
async fn test_mixed_content_output() {
    // Test output with mixed content (numbers, symbols, unicode)
    let executor = DirectExecutor::new();
    let result = executor
        .execute_command("echo '123 !@# 中文 🚀 Русский'")
        .await;

    assert!(result.is_ok(), "Should handle mixed unicode content");
    let command_block = result.unwrap();

    if !command_block.output.is_empty() {
        let output_text: String = command_block
            .output
            .iter()
            .map(|line| line.text.clone())
            .collect();
        // Just verify we got some output, encoding might vary
        assert!(!output_text.is_empty());
    }
}
