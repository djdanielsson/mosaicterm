//! Prompt Formatting
//!
//! Handles custom prompt formatting with variable substitution.
//! Supports common shell prompt variables like $USER, $HOSTNAME, $PWD, etc.

use std::env;
use std::path::Path;

/// Prompt formatter that handles variable substitution
#[derive(Debug, Clone)]
pub struct PromptFormatter {
    /// The format template
    format: String,
}

impl PromptFormatter {
    /// Create a new prompt formatter with the given format string
    pub fn new(format: String) -> Self {
        Self { format }
    }

    /// Render the prompt by substituting variables
    pub fn render(&self, working_dir: &Path) -> String {
        let mut result = self.format.clone();

        // Get system information
        let user = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "user".to_string());

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "localhost".to_string());

        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/".to_string());

        let shell = env::var("SHELL")
            .unwrap_or_else(|_| "sh".to_string());

        // Format PWD (with tilde expansion for home directory)
        let pwd = if let Ok(stripped) = working_dir.strip_prefix(&home) {
            format!("~/{}", stripped.display())
                .trim_end_matches('/')
                .to_string()
        } else {
            working_dir.display().to_string()
        };

        // Handle special case where PWD is exactly home
        let pwd = if working_dir == Path::new(&home) {
            "~".to_string()
        } else {
            pwd
        };

        // Substitute variables
        result = result.replace("$USER", &user);
        result = result.replace("$HOSTNAME", &hostname);
        result = result.replace("$PWD", &pwd);
        result = result.replace("$HOME", &home);
        result = result.replace("$SHELL", &shell);

        // Support escaped variables ($$VAR -> $VAR)
        result = result.replace("$$USER", "$USER");
        result = result.replace("$$HOSTNAME", "$HOSTNAME");
        result = result.replace("$$PWD", "$PWD");
        result = result.replace("$$HOME", "$HOME");
        result = result.replace("$$SHELL", "$SHELL");

        result
    }

    /// Update the format template
    pub fn set_format(&mut self, format: String) {
        self.format = format;
    }

    /// Get the current format template
    pub fn format(&self) -> &str {
        &self.format
    }
}

impl Default for PromptFormatter {
    fn default() -> Self {
        Self::new("$USER@$HOSTNAME:$PWD$ ".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_formatting() {
        let formatter = PromptFormatter::new("$USER> ".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains(">"));
    }

    #[test]
    fn test_pwd_substitution() {
        let formatter = PromptFormatter::new("[$PWD]$ ".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains("/tmp") || result.contains("~"));
    }

    #[test]
    fn test_hostname_substitution() {
        let formatter = PromptFormatter::new("$HOSTNAME:".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(!result.contains("$HOSTNAME"));
    }

    #[test]
    fn test_multiple_variables() {
        let formatter = PromptFormatter::new("$USER@$HOSTNAME:$PWD$ ".to_string());
        let result = formatter.render(Path::new("/usr"));
        assert!(result.contains("@"));
        assert!(result.contains(":"));
        assert!(result.ends_with("$ "));
    }

    #[test]
    fn test_home_directory_tilde() {
        if let Ok(home) = env::var("HOME") {
            let formatter = PromptFormatter::new("$PWD> ".to_string());
            let result = formatter.render(Path::new(&home));
            assert!(result.starts_with("~>") || result.starts_with("~/>"));
        }
    }

    #[test]
    fn test_escaped_variables() {
        let formatter = PromptFormatter::new("$$USER is $USER".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains("$USER is "));
    }

    #[test]
    fn test_set_format() {
        let mut formatter = PromptFormatter::new("old".to_string());
        formatter.set_format("new".to_string());
        assert_eq!(formatter.format(), "new");
    }

    #[test]
    fn test_default_format() {
        let formatter = PromptFormatter::default();
        assert_eq!(formatter.format(), "$USER@$HOSTNAME:$PWD$ ");
    }
}

