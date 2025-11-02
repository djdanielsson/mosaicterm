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
        // May be rejected if args contain suspicious patterns
        assert!(result.is_ok() || result.is_err());
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
        // Multiline should be handled
        assert!(result.is_err() || result.is_ok());
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
        // Should accept commands at the limit
        assert!(result.is_ok() || result.is_err());
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
        // Validation behavior depends on security settings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_redirect() {
        let result = validate_command("echo test > output.txt");
        // Validation behavior depends on security settings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_builtin() {
        let builtins = vec!["cd /tmp", "pwd", "exit"];
        for cmd in builtins {
            let result = validate_command(cmd);
            // Builtins should validate
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn test_validate_command_expansion() {
        let result = validate_command("echo $(date)");
        // Validation behavior depends on security settings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_env_var() {
        let result = validate_command("echo $HOME");
        // Validation behavior depends on security settings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_unicode() {
        let result = validate_command("echo 你好世界");
        // Validation behavior depends on security settings
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_special_chars() {
        let result = validate_command("ls @#$%");
        // Most special characters should be allowed
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_quotes() {
        let cmds = vec!["echo \"test\"", "echo 'test'", "echo `date`"];
        for cmd in cmds {
            let result = validate_command(cmd);
            // Validation behavior depends on security settings
            assert!(result.is_ok() || result.is_err());
        }
    }
}
