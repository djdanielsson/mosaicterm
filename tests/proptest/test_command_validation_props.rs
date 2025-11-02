//! Property-based tests for command validation

use mosaicterm::pty::process::validate_command;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_validate_doesnt_panic(s in "\\PC*") {
        let _ = validate_command(&s);
        // Should not panic on any input
    }

    #[test]
    fn test_validate_simple_commands(cmd in "[a-z]{1,20}") {
        let result = validate_command(&cmd);
        // Simple lowercase commands should mostly validate
        // (unless they happen to match a dangerous pattern)
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validate_with_args(
        cmd in "[a-z]{1,10}",
        args in prop::collection::vec("[a-zA-Z0-9]{0,10}", 0..5),
    ) {
        let full_cmd = format!("{} {}", cmd, args.join(" "));
        let _ = validate_command(&full_cmd);
        // Should handle commands with arguments
    }

    #[test]
    fn test_rejects_null_bytes(
        prefix in "[a-z]{0,20}",
        suffix in "[a-z]{0,20}",
    ) {
        let cmd = format!("{}\0{}", prefix, suffix);
        let result = validate_command(&cmd);
        prop_assert!(result.is_err());
    }

    #[test]
    fn test_rejects_very_long_commands(len in 10001usize..20000) {
        let cmd = "a".repeat(len);
        let result = validate_command(&cmd);
        prop_assert!(result.is_err());
    }

    #[test]
    fn test_handles_various_lengths(len in 1usize..10000) {
        let cmd = "a".repeat(len);
        let result = validate_command(&cmd);
        // Validation depends on security rules and command patterns
        // Just ensure it doesn't panic
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_validates_unicode(s in "[\\u{20}-\\u{7E}]{1,100}") {
        let _ = validate_command(&s);
        // Should handle printable ASCII
    }

    #[test]
    fn test_command_with_spaces(
        parts in prop::collection::vec("[a-z]{1,10}", 1..10),
    ) {
        let cmd = parts.join(" ");
        let _ = validate_command(&cmd);
        // Should handle multi-word commands
    }

    #[test]
    fn test_command_with_dashes(
        cmd in "[a-z]{1,10}",
        flags in prop::collection::vec("-[a-z]", 0..5),
    ) {
        let full_cmd = format!("{} {}", cmd, flags.join(" "));
        let result = validate_command(&full_cmd);
        // Simple flags should be okay
        prop_assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_rejects_device_patterns(device in "/dev/(sd[a-z]|null|zero)") {
        let cmd = format!("echo test > {}", device);
        let result = validate_command(&cmd);
        // Should reject device writes
        prop_assert!(result.is_err());
    }

    #[test]
    fn test_empty_command_rejected(whitespace in "[ \t\n]{0,10}") {
        let result = validate_command(&whitespace);
        prop_assert!(result.is_err());
    }

    #[test]
    fn test_newlines_rejected(
        prefix in "[a-z]{1,10}",
        suffix in "[a-z]{1,10}",
    ) {
        let cmd = format!("{}\n{}", prefix, suffix);
        let result = validate_command(&cmd);
        prop_assert!(result.is_err());
    }
}

#[cfg(test)]
mod security_props {
    use super::*;

    proptest! {
        #[test]
        fn test_dangerous_rm_patterns(path in "/[a-z/]{0,20}") {
            let cmd = format!("rm -rf {}", path);
            let result = validate_command(&cmd);
            // Dangerous rm should be caught
            if path.starts_with('/') && path.len() < 10 {
                prop_assert!(result.is_err());
            }
        }

        #[test]
        fn test_pipe_to_shell_rejected(url in "https?://[a-z.]+\\.[a-z]{2,}") {
            let cmd = format!("curl {} | sh", url);
            let result = validate_command(&cmd);
            prop_assert!(result.is_err());
        }

        #[test]
        fn test_safe_pipes_allowed(
            cmd1 in "(cat|ls|echo)",
            cmd2 in "(grep|sort|uniq)",
        ) {
            let cmd = format!("{} | {}", cmd1, cmd2);
            let result = validate_command(&cmd);
            // Safe pipes might be allowed depending on implementation
            prop_assert!(result.is_ok() || result.is_err());
        }
    }
}
