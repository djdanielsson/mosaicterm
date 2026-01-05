//! Test Fixtures
//!
//! Common test data and fixtures for testing

use mosaicterm::models::{CommandBlock, Config, ExecutionStatus, OutputLine};
use std::path::PathBuf;

/// Create a test configuration with sensible defaults
pub fn create_test_config() -> Config {
    Config::default()
}

/// Create a test command block
pub fn create_test_command_block(command: &str) -> CommandBlock {
    CommandBlock::new(command.to_string(), PathBuf::from("/tmp"))
}

/// Create a test output line
pub fn create_test_output_line(text: &str, line_number: usize) -> OutputLine {
    OutputLine::with_line_number(text, line_number)
}

/// Create a command block with output
pub fn create_command_block_with_output(command: &str, output_lines: Vec<&str>) -> CommandBlock {
    let mut block = create_test_command_block(command);

    for (i, line_text) in output_lines.iter().enumerate() {
        block.add_output_line(create_test_output_line(line_text, i + 1));
    }

    block
}

/// Create a completed command block
pub fn create_completed_command_block(command: &str, output_lines: Vec<&str>) -> CommandBlock {
    let mut block = create_command_block_with_output(command, output_lines);
    block.mark_completed(std::time::Duration::from_secs(1));
    block
}

/// Create a failed command block
pub fn create_failed_command_block(command: &str, error_msg: &str) -> CommandBlock {
    let mut block = create_test_command_block(command);
    block.add_output_line(create_test_output_line(error_msg, 1));
    block.mark_failed(std::time::Duration::from_secs(1), 1);
    block
}

/// Create sample ANSI output for testing
pub fn create_ansi_output() -> Vec<String> {
    vec![
        "\x1b[31mRed text\x1b[0m".to_string(),
        "\x1b[32mGreen text\x1b[0m".to_string(),
        "\x1b[1mBold text\x1b[0m".to_string(),
        "\x1b[4mUnderlined text\x1b[0m".to_string(),
    ]
}

/// Create sample plain output for testing
pub fn create_plain_output() -> Vec<String> {
    vec![
        "Line 1".to_string(),
        "Line 2".to_string(),
        "Line 3".to_string(),
    ]
}

/// Create large output for performance testing
pub fn create_large_output(line_count: usize) -> Vec<String> {
    (0..line_count)
        .map(|i| format!("Line {} with some test data", i))
        .collect()
}

/// Create binary output for testing
pub fn create_binary_output() -> Vec<u8> {
    vec![0x00, 0x01, 0x02, 0xFF, 0xFE, 0xFD]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_config() {
        let config = create_test_config();
        assert!(config.ui.font_size > 0);
    }

    #[test]
    fn test_create_test_command_block() {
        let block = create_test_command_block("ls -la");
        assert_eq!(block.command, "ls -la");
        assert_eq!(block.status, ExecutionStatus::Pending);
    }

    #[test]
    fn test_create_test_output_line() {
        let line = create_test_output_line("test", 1);
        assert_eq!(line.text, "test");
        assert_eq!(line.line_number, 1);
    }

    #[test]
    fn test_create_command_block_with_output() {
        let block = create_command_block_with_output("echo test", vec!["line1", "line2", "line3"]);
        assert_eq!(block.output.len(), 3);
        assert_eq!(block.output[0].text, "line1");
    }

    #[test]
    fn test_create_completed_command_block() {
        let block = create_completed_command_block("pwd", vec!["/tmp"]);
        assert_eq!(block.status, ExecutionStatus::Completed);
        assert_eq!(block.output.len(), 1);
    }

    #[test]
    fn test_create_failed_command_block() {
        let block = create_failed_command_block("false", "Command failed");
        assert_eq!(block.status, ExecutionStatus::Failed);
        assert!(block.output[0].text.contains("Command failed"));
    }

    #[test]
    fn test_create_ansi_output() {
        let output = create_ansi_output();
        assert_eq!(output.len(), 4);
        assert!(output[0].contains("\x1b[31m"));
    }

    #[test]
    fn test_create_large_output() {
        let output = create_large_output(1000);
        assert_eq!(output.len(), 1000);
        assert!(output[500].contains("Line 500"));
    }

    #[test]
    fn test_create_binary_output() {
        let output = create_binary_output();
        assert_eq!(output.len(), 6);
        assert_eq!(output[0], 0x00);
        assert_eq!(output[5], 0xFD);
    }
}
