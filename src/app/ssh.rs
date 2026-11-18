//! SSH Session Handling
//!
//! This module handles SSH session detection, remote prompt parsing,
//! and session lifecycle management.

use tracing::info;

use super::MosaicTermApp;
use mosaicterm::security_audit;

impl MosaicTermApp {
    /// Check if a command is an SSH command
    pub(super) fn is_ssh_command(&self, command: &str) -> bool {
        let cmd_name = self.get_command_name(command);
        cmd_name == "ssh"
    }

    /// Extract the host from an SSH command (e.g., "ssh user@host" -> "user@host")
    pub(super) fn extract_ssh_host(&self, command: &str) -> String {
        let parts: Vec<&str> = command.split_whitespace().collect();
        // Find the first argument that looks like a host (contains @ or doesn't start with -)
        for part in parts.iter().skip(1) {
            if !part.starts_with('-') {
                return part.to_string();
            }
        }
        "remote".to_string()
    }

    /// End the SSH session and restore local mode
    pub(super) fn end_ssh_session(&mut self) {
        if self.pane.ssh_session_active {
            info!("Ending SSH session - performing full cleanup");

            // Log session end (non-sensitive)
            security_audit::log_security_event(
                security_audit::SecurityEvent::SshSessionEnd,
                Some("reason=user_exit"),
            );

            self.pane.ssh_session_active = false;
            self.pane.ssh_session_command = None;
            self.pane.ssh_remote_prompt = None;
            self.pane.ssh_prompt_buffer.clear();

            // Restore local prompt
            self.update_prompt();
            self.set_status_message(Some("Disconnected from remote host".to_string()));
        }
    }

    /// Detect if SSH session has ended based on output (static version for borrow-safe use)
    pub(super) fn detect_ssh_session_end_static(output: &str) -> bool {
        let output_lower = output.to_lowercase();

        // Common SSH connection closed messages - be specific to avoid false positives
        // Messages like "Connection to host closed." from SSH itself
        if (output_lower.contains("connection to") && output_lower.contains("closed"))
            || output_lower.contains("connection closed by")
            || output_lower.contains("connection reset by peer")
            || output_lower.contains("connection timed out")
            || output_lower.contains("broken pipe")
            // "Connection to X closed." is the standard SSH exit message
            || (output_lower.contains("closed.") && output_lower.contains("connection"))
        {
            return true;
        }

        // Check if any line is exactly "logout" (the remote shell logout message)
        for line in output_lower.lines() {
            if line.trim() == "logout" {
                return true;
            }
        }

        false
    }

    /// Check if prompt looks like local prompt (not SSH remote prompt)
    /// Used to detect when SSH session has ended and we're back to local shell
    /// Static version for borrow-safe use
    pub(super) fn is_local_prompt_static(remote_prompt: Option<&String>, prompt: &str) -> bool {
        if let Some(remote_prompt) = remote_prompt {
            // If we have a remote prompt captured, check if this prompt is different
            let remote_cleaned = remote_prompt.trim();
            let prompt_cleaned = prompt.trim();

            // Extract user@host portion from both prompts
            let extract_user_host = |p: &str| -> Option<String> {
                if let Some(at_pos) = p.find('@') {
                    // Find where user@host ends (usually at : or space or $)
                    let start = p[..at_pos]
                        .rfind(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let end = p[at_pos..]
                        .find([':', ' ', '$', '%', '>'])
                        .map(|i| at_pos + i)
                        .unwrap_or(p.len());
                    Some(p[start..end].to_string())
                } else {
                    None
                }
            };

            let remote_user_host = extract_user_host(remote_cleaned);
            let current_user_host = extract_user_host(prompt_cleaned);

            // If both have user@host and they're different, we're probably back to local
            if let (Some(remote), Some(current)) = (remote_user_host, current_user_host) {
                if remote != current {
                    info!(
                        "Prompt changed from '{}' to '{}' - session likely ended",
                        remote, current
                    );
                    return true;
                }
            }
        }
        false
    }

    /// Try to detect a shell prompt from SSH output (static version for borrow-safe use)
    /// Returns the prompt string if found
    pub(super) fn detect_remote_prompt_static(output: &str) -> Option<String> {
        // Strip ANSI codes first
        let cleaned = Self::strip_ansi_codes_static(output);

        // Look for common prompt patterns at the end of lines
        // Prompts typically end with $ (bash), % (zsh), > (other), or #  (root)
        for line in cleaned.lines().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this looks like a prompt
            // Common patterns: user@host:path$ or [user@host path]$ or host:path$
            let is_prompt = (trimmed.ends_with("$ ")
                || trimmed.ends_with('$')
                || trimmed.ends_with("% ")
                || trimmed.ends_with('%')
                || trimmed.ends_with("> ")
                || trimmed.ends_with('>')
                || trimmed.ends_with("# ")
                || trimmed.ends_with('#'))
                && (trimmed.contains('@') || trimmed.contains(':') || trimmed.len() < 50);

            if is_prompt {
                // Extract the prompt (add space if not present)
                let prompt = if trimmed.ends_with(' ') {
                    trimmed.to_string()
                } else {
                    format!("{} ", trimmed)
                };
                return Some(prompt);
            }
        }

        None
    }

    /// Simple ANSI code stripping (static version for borrow-safe use)
    pub(super) fn strip_ansi_codes_static(text: &str) -> String {
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Skip ANSI sequence
                if chars.peek() == Some(&'[') {
                    chars.next();
                    // Skip until letter
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
            } else {
                result.push(ch);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ssh_session_end_connection_closed() {
        assert!(MosaicTermApp::detect_ssh_session_end_static(
            "Connection to example.com closed."
        ));
        assert!(MosaicTermApp::detect_ssh_session_end_static(
            "Connection to 192.168.1.1 closed."
        ));
    }

    #[test]
    fn test_detect_ssh_session_end_logout() {
        assert!(MosaicTermApp::detect_ssh_session_end_static("logout\n"));
        assert!(MosaicTermApp::detect_ssh_session_end_static(
            "some output\nlogout\n"
        ));
    }

    #[test]
    fn test_detect_ssh_session_end_connection_reset() {
        assert!(MosaicTermApp::detect_ssh_session_end_static(
            "Connection reset by peer"
        ));
        assert!(MosaicTermApp::detect_ssh_session_end_static(
            "Read from remote host: Connection reset by peer"
        ));
    }

    #[test]
    fn test_detect_ssh_session_end_false_positives() {
        // These should NOT trigger end detection
        assert!(!MosaicTermApp::detect_ssh_session_end_static(
            "Connecting to server..."
        ));
        assert!(!MosaicTermApp::detect_ssh_session_end_static(
            "user@remote:~$ ls"
        ));
        assert!(!MosaicTermApp::detect_ssh_session_end_static(
            "file.log\ndir/"
        ));
        // "logging out" is different from "logout"
        assert!(!MosaicTermApp::detect_ssh_session_end_static(
            "logging out..."
        ));
    }

    #[test]
    fn test_detect_remote_prompt_bash() {
        let output = "Welcome to Ubuntu\nuser@server:~$ ";
        let prompt = MosaicTermApp::detect_remote_prompt_static(output);
        assert!(prompt.is_some());
        assert!(prompt.unwrap().contains("user@server"));
    }

    #[test]
    fn test_detect_remote_prompt_zsh() {
        let output = "Last login: Mon Jan 1\nuser@host % ";
        let prompt = MosaicTermApp::detect_remote_prompt_static(output);
        assert!(prompt.is_some());
    }

    #[test]
    fn test_detect_remote_prompt_root() {
        let output = "root@server:~# ";
        let prompt = MosaicTermApp::detect_remote_prompt_static(output);
        assert!(prompt.is_some());
    }

    #[test]
    fn test_detect_remote_prompt_with_output() {
        let output = "file1.txt\nfile2.txt\nuser@host:~$ ";
        let prompt = MosaicTermApp::detect_remote_prompt_static(output);
        assert!(prompt.is_some());
    }

    #[test]
    fn test_detect_remote_prompt_no_match() {
        let output = "Just some regular output\nwithout any prompts";
        let prompt = MosaicTermApp::detect_remote_prompt_static(output);
        assert!(prompt.is_none());
    }

    #[test]
    fn test_is_local_prompt_different_hosts() {
        let remote = Some("user@remote:~$ ".to_string());
        assert!(MosaicTermApp::is_local_prompt_static(
            remote.as_ref(),
            "user@local:~$ "
        ));
    }

    #[test]
    fn test_is_local_prompt_same_host() {
        let remote = Some("user@remote:~$ ".to_string());
        assert!(!MosaicTermApp::is_local_prompt_static(
            remote.as_ref(),
            "user@remote:/tmp$ "
        ));
    }

    #[test]
    fn test_is_local_prompt_no_remote() {
        assert!(!MosaicTermApp::is_local_prompt_static(
            None,
            "user@local:~$ "
        ));
    }

    #[test]
    fn test_strip_ansi_codes_static() {
        let input = "\x1b[32mgreen\x1b[0m text";
        let output = MosaicTermApp::strip_ansi_codes_static(input);
        assert_eq!(output, "green text");
    }

    #[test]
    fn test_strip_ansi_codes_static_cursor_movement() {
        let input = "\x1b[2Jcleared\x1b[H";
        let output = MosaicTermApp::strip_ansi_codes_static(input);
        assert_eq!(output, "cleared");
    }

    #[test]
    fn test_strip_ansi_codes_static_no_codes() {
        let input = "plain text";
        let output = MosaicTermApp::strip_ansi_codes_static(input);
        assert_eq!(output, "plain text");
    }
}
