//! Security Audit Logging
//!
//! This module provides security audit logging for important events.
//!
//! ## Security Policy
//!
//! - **NEVER** log passwords, passphrases, or credentials
//! - Only log security-relevant events with non-sensitive metadata
//! - Use INFO level for normal events, WARN for suspicious activity
//! - Audit logs are separate from debug logs
//!
//! ## Events Logged
//!
//! - SSH connection attempts (host only, no credentials)
//! - SSH session start/end
//! - Authentication prompt display (type only, no input)
//! - File permission changes
//! - Configuration loads/errors

use tracing::{info, warn};

/// Security audit event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEvent {
    /// SSH connection initiated
    SshConnectionAttempt,
    /// SSH session established
    SshSessionStart,
    /// SSH session ended
    SshSessionEnd,
    /// SSH authentication prompt shown
    SshAuthPrompt,
    /// History file accessed
    HistoryFileAccess,
    /// Configuration file loaded
    ConfigLoaded,
    /// Configuration file error
    ConfigError,
    /// Suspicious activity detected
    SuspiciousActivity,
}

impl SecurityEvent {
    /// Get a human-readable description of the event
    pub fn description(&self) -> &'static str {
        match self {
            SecurityEvent::SshConnectionAttempt => "SSH connection initiated",
            SecurityEvent::SshSessionStart => "SSH session established",
            SecurityEvent::SshSessionEnd => "SSH session terminated",
            SecurityEvent::SshAuthPrompt => "SSH authentication prompt displayed",
            SecurityEvent::HistoryFileAccess => "Command history file accessed",
            SecurityEvent::ConfigLoaded => "Configuration loaded successfully",
            SecurityEvent::ConfigError => "Configuration loading error",
            SecurityEvent::SuspiciousActivity => "Suspicious activity detected",
        }
    }

    /// Check if this event is suspicious and should trigger warnings
    pub fn is_suspicious(&self) -> bool {
        matches!(self, SecurityEvent::SuspiciousActivity)
    }
}

/// Log a security audit event
///
/// ## Security Note
///
/// Never pass sensitive data (passwords, keys, etc.) as metadata.
/// Only include non-sensitive information like hostnames, event types, counts, etc.
///
/// # Examples
///
/// ```
/// use mosaicterm::security_audit::{log_security_event, SecurityEvent};
///
/// // Good: Logs event with non-sensitive metadata
/// log_security_event(SecurityEvent::SshConnectionAttempt, Some("host=example.com"));
///
/// // BAD: Never do this!
/// // log_security_event(SecurityEvent::SshAuthPrompt, Some("password=secret123"));
/// ```
pub fn log_security_event(event: SecurityEvent, metadata: Option<&str>) {
    let event_desc = event.description();

    let log_message = if let Some(meta) = metadata {
        format!("🔒 SECURITY AUDIT: {} | {}", event_desc, meta)
    } else {
        format!("🔒 SECURITY AUDIT: {}", event_desc)
    };

    if event.is_suspicious() {
        warn!("{}", log_message);
    } else {
        info!("{}", log_message);
    }
}

/// Log SSH connection attempt (host only, no credentials)
pub fn log_ssh_connection(host: &str) {
    log_security_event(
        SecurityEvent::SshConnectionAttempt,
        Some(&format!("host={}", sanitize_hostname(host))),
    );
}

/// Log SSH session start
pub fn log_ssh_session_start(host: &str) {
    log_security_event(
        SecurityEvent::SshSessionStart,
        Some(&format!("host={}", sanitize_hostname(host))),
    );
}

/// Log SSH session end
pub fn log_ssh_session_end(duration_secs: u64) {
    log_security_event(
        SecurityEvent::SshSessionEnd,
        Some(&format!("duration={}s", duration_secs)),
    );
}

/// Log authentication prompt display (type only, never the input)
pub fn log_auth_prompt(prompt_type: &str) {
    log_security_event(
        SecurityEvent::SshAuthPrompt,
        Some(&format!("type={}", prompt_type)),
    );
}

/// Log history file access
pub fn log_history_access(operation: &str) {
    log_security_event(
        SecurityEvent::HistoryFileAccess,
        Some(&format!("operation={}", operation)),
    );
}

/// Log configuration events
pub fn log_config_event(is_error: bool, details: Option<&str>) {
    let event = if is_error {
        SecurityEvent::ConfigError
    } else {
        SecurityEvent::ConfigLoaded
    };
    log_security_event(event, details);
}

/// Sanitize hostname to prevent log injection
fn sanitize_hostname(host: &str) -> String {
    // Remove any characters that could be used for log injection
    host.chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '@' || *c == ':')
        .take(100) // Limit length
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_event_description() {
        assert_eq!(
            SecurityEvent::SshConnectionAttempt.description(),
            "SSH connection initiated"
        );
        assert_eq!(
            SecurityEvent::SshSessionStart.description(),
            "SSH session established"
        );
        assert_eq!(
            SecurityEvent::SuspiciousActivity.description(),
            "Suspicious activity detected"
        );
    }

    #[test]
    fn test_suspicious_detection() {
        assert!(SecurityEvent::SuspiciousActivity.is_suspicious());
        assert!(!SecurityEvent::SshConnectionAttempt.is_suspicious());
        assert!(!SecurityEvent::ConfigLoaded.is_suspicious());
    }

    #[test]
    fn test_sanitize_hostname() {
        // Valid hostnames
        assert_eq!(sanitize_hostname("example.com"), "example.com");
        assert_eq!(sanitize_hostname("user@host.com"), "user@host.com");
        assert_eq!(sanitize_hostname("192.168.1.1"), "192.168.1.1");
        assert_eq!(sanitize_hostname("host.com:22"), "host.com:22");

        // Should remove dangerous characters
        assert_eq!(sanitize_hostname("host;rm -rf"), "hostrm-rf");
        assert_eq!(sanitize_hostname("host\nmalicious"), "hostmalicious");
        assert_eq!(sanitize_hostname("host$(cmd)"), "hostcmd");

        // Should truncate long strings
        let long_host = "a".repeat(200);
        assert_eq!(sanitize_hostname(&long_host).len(), 100);
    }

    #[test]
    fn test_log_functions_dont_panic() {
        // These should not panic even with edge cases
        log_ssh_connection("example.com");
        log_ssh_connection(""); // Empty
        log_ssh_connection("very-long-hostname-that-exceeds-normal-lengths.example.com");

        log_ssh_session_start("host");
        log_ssh_session_end(3600);
        log_auth_prompt("password");
        log_history_access("read");
        log_config_event(false, Some("test"));
        log_config_event(true, None);
    }

    #[test]
    fn test_no_sensitive_data_in_logs() {
        // This test documents the security policy:
        // NEVER log sensitive data

        // Good examples (safe to log):
        log_security_event(
            SecurityEvent::SshConnectionAttempt,
            Some("host=example.com"),
        );
        log_security_event(SecurityEvent::SshAuthPrompt, Some("type=password"));
        log_security_event(SecurityEvent::SshSessionEnd, Some("duration=120s"));

        // BAD examples (commented out - never do this):
        // log_security_event(SecurityEvent::SshAuthPrompt, Some("password=secret123"));
        // log_security_event(SecurityEvent::SshAuthPrompt, Some("passphrase=mykey"));
        // log_security_event(SecurityEvent::SshSessionStart, Some("credentials=user:pass"));

        // This test passes by not panicking
    }
}
