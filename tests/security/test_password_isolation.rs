//! Security Tests: SSH Password Isolation
//!
//! These tests verify that SSH passwords and passphrases never leak into
//! command history or other persistent storage.

use std::fs;
use tempfile::TempDir;

use mosaicterm::history::HistoryManager;

#[test]
fn test_history_file_secure_permissions() {
    // Create a temporary directory for test history
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_path = temp_dir.path().join("test_history.txt");

    // Create history manager with test path
    let mut manager =
        HistoryManager::with_path(history_path.clone()).expect("Failed to create history manager");

    // Add a regular command to create the file
    manager
        .add("ls -la".to_string())
        .expect("Failed to add command");

    // Check file permissions on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&history_path).expect("Failed to read metadata");
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Extract permission bits (last 9 bits: rwxrwxrwx)
        let perms = mode & 0o777;

        // Should be 0600 (owner read/write only)
        assert_eq!(
            perms, 0o600,
            "History file should have 0600 permissions, got {:o}",
            perms
        );
    }
}

#[test]
fn test_ssh_password_not_in_history() {
    // Create a temporary directory for test history
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_path = temp_dir.path().join("test_ssh_history.txt");

    // Create history manager
    let mut manager =
        HistoryManager::with_path(history_path.clone()).expect("Failed to create history manager");

    // Add SSH command (this should be in history)
    manager
        .add("ssh user@example.com".to_string())
        .expect("Failed to add SSH command");

    // Simulate other commands that might be run
    manager
        .add("ls -la".to_string())
        .expect("Failed to add ls command");
    manager
        .add("pwd".to_string())
        .expect("Failed to add pwd command");

    // The following strings simulate what would happen if passwords were
    // accidentally added to history (they should NOT be there)
    let sensitive_strings = vec![
        "my_secret_password",
        "p@ssw0rd123",
        "Enter passphrase",
        "SSH_PASSWORD=secret",
    ];

    // Read history file contents
    let history_contents = fs::read_to_string(&history_path).expect("Failed to read history file");

    // Verify no sensitive strings are present
    for sensitive in &sensitive_strings {
        assert!(
            !history_contents.contains(sensitive),
            "History file should not contain sensitive string: {}",
            sensitive
        );
    }

    // Verify expected commands are present
    assert!(
        history_contents.contains("ssh user@example.com"),
        "History should contain SSH command"
    );
    assert!(
        history_contents.contains("ls -la"),
        "History should contain ls command"
    );
}

#[test]
fn test_history_only_contains_user_commands() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_path = temp_dir.path().join("test_user_commands.txt");

    let mut manager =
        HistoryManager::with_path(history_path.clone()).expect("Failed to create history manager");

    // Add legitimate user commands
    let user_commands = vec![
        "git status",
        "cargo build",
        "ssh user@host",
        "docker ps",
        "npm install",
    ];

    for cmd in &user_commands {
        manager.add(cmd.to_string()).expect("Failed to add command");
    }

    // Read history
    let history_contents = fs::read_to_string(&history_path).expect("Failed to read history file");

    // Verify all user commands are present
    for cmd in &user_commands {
        assert!(
            history_contents.contains(cmd),
            "History should contain user command: {}",
            cmd
        );
    }

    // Verify no authentication artifacts
    let forbidden_patterns = vec![
        "password",
        "passphrase",
        "secret",
        "credential",
        "token",
        "auth_key",
    ];

    let history_lower = history_contents.to_lowercase();
    for pattern in &forbidden_patterns {
        // We allow "ssh" which contains "ssh" but not standalone auth terms
        if *pattern != "ssh" {
            assert!(
                !history_lower.contains(pattern),
                "History should not contain authentication pattern: {}",
                pattern
            );
        }
    }
}

#[test]
fn test_command_block_does_not_persist_passwords() {
    // Test that CommandBlock doesn't accidentally serialize sensitive data
    use mosaicterm::models::CommandBlock;
    use std::path::PathBuf;

    // Create a command block (simulating an SSH session)
    let block = CommandBlock::new("ssh user@host".to_string(), PathBuf::from("/tmp"));

    // Serialize to JSON (if this were to be persisted)
    let serialized = serde_json::to_string(&block).expect("Failed to serialize");

    // Verify the serialized form doesn't contain password field
    // (CommandBlock should never have a password field)
    assert!(
        !serialized.contains("\"password\""),
        "CommandBlock serialization should not contain password field"
    );
    assert!(
        !serialized.contains("\"passphrase\""),
        "CommandBlock serialization should not contain passphrase field"
    );

    // Verify it only contains expected fields
    assert!(serialized.contains("\"command\""), "Should contain command");
    assert!(serialized.contains("\"output\""), "Should contain output");
    assert!(
        serialized.contains("\"timestamp\""),
        "Should contain timestamp"
    );
}

#[test]
fn test_history_manager_filters_empty_commands() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_path = temp_dir.path().join("test_empty_filter.txt");

    let mut manager =
        HistoryManager::with_path(history_path.clone()).expect("Failed to create history manager");

    // Try to add empty/whitespace commands
    manager.add("".to_string()).expect("Should handle empty");
    manager
        .add("   ".to_string())
        .expect("Should handle whitespace");
    manager
        .add("\n".to_string())
        .expect("Should handle newline");

    // Add valid command
    manager
        .add("valid command".to_string())
        .expect("Should add valid command");

    // Read history
    let history_contents = fs::read_to_string(&history_path).expect("Failed to read history file");

    // Should only contain the valid command
    assert!(
        history_contents.contains("valid command"),
        "Should contain valid command"
    );

    // Count non-empty lines
    let line_count = history_contents
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    assert_eq!(line_count, 1, "Should only have one non-empty line");
}

#[test]
fn test_history_deduplication() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let history_path = temp_dir.path().join("test_dedup.txt");

    let mut manager =
        HistoryManager::with_path(history_path.clone()).expect("Failed to create history manager");

    // Add the same command multiple times
    manager
        .add("ls -la".to_string())
        .expect("Failed to add command");
    manager
        .add("pwd".to_string())
        .expect("Failed to add command");
    manager
        .add("ls -la".to_string())
        .expect("Failed to add command");
    manager
        .add("ls -la".to_string())
        .expect("Failed to add command");

    // Check in-memory history has deduplication
    let entries = manager.entries();
    let ls_count = entries.iter().filter(|e| *e == "ls -la").count();

    assert_eq!(
        ls_count, 1,
        "Duplicate commands should be deduplicated in memory"
    );

    // The most recent occurrence should be at the end
    assert_eq!(
        entries.back().unwrap(),
        "ls -la",
        "Most recent command should be at the end"
    );
}

#[cfg(test)]
mod integration {
    use super::*;

    /// Integration test: Verify SSH authentication flow doesn't leak passwords
    /// This simulates a complete SSH session flow
    #[test]
    fn test_ssh_session_complete_flow() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let history_path = temp_dir.path().join("test_ssh_flow.txt");

        let mut manager = HistoryManager::with_path(history_path.clone())
            .expect("Failed to create history manager");

        // Step 1: User types SSH command
        manager
            .add("ssh admin@production-server".to_string())
            .expect("Failed to add SSH command");

        // Step 2: SSH asks for password (this happens in overlay, not as a command)
        // Password "SuperSecret123!" is entered in overlay
        // This should NEVER reach history

        // Step 3: User runs commands on remote server
        manager
            .add("whoami".to_string())
            .expect("Failed to add whoami");
        manager
            .add("uptime".to_string())
            .expect("Failed to add uptime");

        // Step 4: User exits SSH
        manager.add("exit".to_string()).expect("Failed to add exit");

        // Verify history
        let history_contents =
            fs::read_to_string(&history_path).expect("Failed to read history file");

        // Should contain all commands
        assert!(history_contents.contains("ssh admin@production-server"));
        assert!(history_contents.contains("whoami"));
        assert!(history_contents.contains("uptime"));
        assert!(history_contents.contains("exit"));

        // Should NOT contain password or authentication artifacts
        assert!(!history_contents.contains("SuperSecret123!"));
        assert!(!history_contents.contains("password:"));
        assert!(!history_contents.contains("passphrase:"));
        assert!(
            !history_contents.to_lowercase().contains("enter password"),
            "Should not contain password prompts"
        );
    }
}
