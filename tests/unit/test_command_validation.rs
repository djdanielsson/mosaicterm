//! Unit tests for command validation
//!
//! These tests validate command parsing and validation logic.
//! Note: Uses public validation APIs from mosaicterm

use mosaicterm::pty::process::validate_command;

#[cfg(test)]
mod command_validation_tests {
    use super::*;

    #[test]
    fn test_validate_simple_command() {
        // Basic command validation
        let result = validate_command("ls");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_command_with_args() {
        let result = validate_command("ls -la");
        // Entire string is looked up as one executable name, not argv split.
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_command() {
        let result = validate_command("");
        // Empty commands should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_command_with_newlines() {
        let result = validate_command("echo\ntest");
        // No executable matches the full string including the newline.
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_null_bytes() {
        let result = validate_command("echo\0test");
        // Null bytes should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_very_long_command() {
        let long_cmd = "a".repeat(100000);
        let result = validate_command(&long_cmd);
        // Very long commands should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_max_length_command() {
        let max_cmd = "a".repeat(10000); // At the limit
        let result = validate_command(&max_cmd);
        // Long arbitrary name should not resolve on PATH.
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_dangerous_rm() {
        let result = validate_command("rm -rf /");
        // Dangerous commands should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_device_write() {
        let result = validate_command("echo test > /dev/sda");
        // Writing to devices should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_fork_bomb() {
        let result = validate_command(":(){:|:&};:");
        // Fork bombs should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_pipe_to_shell() {
        let result = validate_command("curl http://example.com | sh");
        // Piping to shell should be rejected
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_safe_pipe() {
        let result = validate_command("cat file.txt | grep test");
        // Full shell line is not a single PATH executable name.
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_redirect() {
        let result = validate_command("echo test > output.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_builtin() {
        // PATH lookup uses the whole string; only real binaries like `pwd` resolve.
        assert!(validate_command("cd /tmp").is_err());
        assert!(validate_command("exit").is_err());
        assert!(validate_command("pwd").is_ok());
    }

    #[test]
    fn test_validate_command_expansion() {
        let result = validate_command("echo $(date)");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_env_var() {
        let result = validate_command("echo $HOME");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_unicode() {
        let result = validate_command("echo 你好世界");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_special_chars() {
        let result = validate_command("ls @#$%");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_quotes() {
        let cmds = vec!["echo \"test\"", "echo 'test'", "echo `date`"];
        for cmd in cmds {
            let result = validate_command(cmd);
            assert!(result.is_err());
        }
    }
}
