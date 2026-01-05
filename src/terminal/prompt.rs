//! Prompt Detection Logic
//!
//! Detects shell prompts and command completion in terminal output.

use crate::error::Result;
use crate::models::{OutputLine, ShellType};
use regex::Regex;
use tracing::warn;

/// Prompt detector for various shell types
#[derive(Debug)]
pub struct PromptDetector {
    /// Patterns for different shell prompts
    prompt_patterns: Vec<PromptPattern>,
    /// Current shell type
    current_shell: ShellType,
    /// Custom prompt patterns
    custom_patterns: Vec<Regex>,
}

#[derive(Debug, Clone)]
pub struct PromptPattern {
    /// Shell type
    shell_type: ShellType,
    /// Regex pattern for prompt detection
    pattern: Regex,
}

impl PromptDetector {
    /// Create a new prompt detector
    pub fn new() -> Self {
        let mut detector = Self {
            prompt_patterns: Vec::new(),
            current_shell: ShellType::Other,
            custom_patterns: Vec::new(),
        };

        detector.initialize_patterns();
        detector
    }
}

impl Default for PromptDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptDetector {
    /// Create with specific shell type
    pub fn with_shell(shell_type: ShellType) -> Self {
        let mut detector = Self::new();
        detector.current_shell = shell_type;
        detector
    }

    /// Initialize default prompt patterns for common shells
    ///
    /// Uses patterns from [`ShellType::get_default_prompt_patterns`] as the base,
    /// plus additional shell-specific patterns for better detection coverage.
    ///
    /// Note: Pattern order matters! More specific patterns should come before
    /// generic ones to avoid false matches.
    fn initialize_patterns(&mut self) {
        // Add canonical patterns from ShellType for Bash and Zsh
        for shell_type in [ShellType::Bash, ShellType::Zsh] {
            for pattern in shell_type.get_default_prompt_patterns() {
                self.add_pattern(shell_type, &pattern);
            }
        }

        // Additional Bash patterns not in ShellType
        self.add_pattern(ShellType::Bash, r"^bash-\d+\.\d+\$ $");

        // Additional Zsh patterns not in ShellType
        self.add_pattern(ShellType::Zsh, r"^zsh-\d+\.\d+% $");

        // PowerShell patterns - must come before Fish's generic `> $` patterns
        self.add_pattern(ShellType::PowerShell, r"^PS .*> $");
        self.add_pattern(ShellType::PowerShell, r"^PS .*>$");

        // Cmd patterns - must come before Fish's generic `> $` patterns
        self.add_pattern(ShellType::Cmd, r"^[A-Z]:\\.*> $");

        // Fish patterns - specific patterns first, then generic
        self.add_pattern(ShellType::Fish, r"^> $"); // Simple arrow prompt
        self.add_pattern(ShellType::Fish, r"^\[.*\]> $"); // Bracketed prompt
                                                          // Note: We intentionally don't use ShellType::Fish.get_default_prompt_patterns()
                                                          // because it includes `^.*> $` which is too generic and matches other shells
    }

    /// Add a prompt pattern
    fn add_pattern(&mut self, shell_type: ShellType, pattern: &str) {
        match Regex::new(pattern) {
            Ok(regex) => {
                self.prompt_patterns.push(PromptPattern {
                    shell_type,
                    pattern: regex,
                });
            }
            Err(e) => {
                warn!("Failed to compile regex pattern '{}': {}", pattern, e);
            }
        }
    }

    /// Detect if a line contains a shell prompt
    pub fn is_prompt(&mut self, line: &OutputLine) -> bool {
        // Check custom patterns first
        for pattern in &self.custom_patterns {
            if pattern.is_match(&line.text) {
                return true;
            }
        }

        // Check built-in patterns
        for pattern in &self.prompt_patterns {
            if pattern.pattern.is_match(&line.text) {
                // Update current shell type if detected
                if self.current_shell == ShellType::Other {
                    self.current_shell = pattern.shell_type;
                }
                return true;
            }
        }

        false
    }

    /// Detect shell type from output
    pub fn detect_shell_type(&mut self, lines: &[OutputLine]) -> ShellType {
        for line in lines {
            for pattern in &self.prompt_patterns {
                if pattern.pattern.is_match(&line.text) {
                    self.current_shell = pattern.shell_type;
                    return pattern.shell_type;
                }
            }
        }

        ShellType::Other
    }

    /// Add custom prompt pattern
    pub fn add_custom_pattern(&mut self, pattern: &str) -> Result<()> {
        let regex = Regex::new(pattern)?;
        self.custom_patterns.push(regex);
        Ok(())
    }

    /// Get current shell type
    pub fn current_shell(&self) -> ShellType {
        self.current_shell
    }

    /// Get prompt patterns for current shell
    pub fn get_patterns_for_shell(&self, shell_type: ShellType) -> Vec<&PromptPattern> {
        self.prompt_patterns
            .iter()
            .filter(|p| p.shell_type == shell_type)
            .collect()
    }

    /// Clear custom patterns
    pub fn clear_custom_patterns(&mut self) {
        self.custom_patterns.clear();
    }

    /// Get shell type from string
    ///
    /// Delegates to [`ShellType::from_string`] for consistent shell type parsing.
    #[inline]
    pub fn shell_type_from_string(shell_name: &str) -> ShellType {
        ShellType::from_string(shell_name)
    }
}

/// Command completion detection
#[derive(Debug)]
pub struct CommandCompletionDetector {
    /// Patterns that indicate command completion
    completion_patterns: Vec<Regex>,
    /// Patterns that indicate command is still running
    continuation_patterns: Vec<Regex>,
}

impl CommandCompletionDetector {
    /// Create a new completion detector
    pub fn new() -> Self {
        let mut detector = Self {
            completion_patterns: Vec::new(),
            continuation_patterns: Vec::new(),
        };

        detector.initialize_patterns();
        detector
    }
}

impl Default for CommandCompletionDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCompletionDetector {
    /// Initialize default patterns
    fn initialize_patterns(&mut self) {
        // Completion patterns (indicate command finished) - enhanced with versioned prompts
        self.completion_patterns.extend([
            // Basic prompts (with and without trailing space)
            Regex::new(r"^\$$").unwrap(),
            Regex::new(r"^\$ $").unwrap(),
            Regex::new(r"^%$").unwrap(),
            Regex::new(r"^% $").unwrap(),
            Regex::new(r"^>$").unwrap(),
            Regex::new(r"^> $").unwrap(),
            // Versioned shell prompts (with and without trailing space)
            Regex::new(r"^bash-\d+\.\d+\$").unwrap(), // bash-5.2$
            Regex::new(r"^bash-\d+\.\d+\$ ").unwrap(), // bash-5.2$
            Regex::new(r"^zsh-\d+\.\d+%").unwrap(),   // zsh-5.8%
            Regex::new(r"^zsh-\d+\.\d+% ").unwrap(),  // zsh-5.8%
            Regex::new(r"^fish-\d+\.\d+>").unwrap(),  // fish-3.4>
            Regex::new(r"^fish-\d+\.\d+> ").unwrap(), // fish-3.4>
            // User@host style prompts
            Regex::new(r"^\[.*\]\$ $").unwrap(),
            Regex::new(r"^\[.*\]% $").unwrap(),
            Regex::new(r"^\[.*\]> $").unwrap(),
            // PS1 style prompts with paths (very specific to avoid false positives)
            Regex::new(r"^[~/][A-Za-z0-9/_.-]*\$ $").unwrap(), // /path/to/dir$ or ~/path$
            Regex::new(r"^[~/][A-Za-z0-9/_.-]*% $").unwrap(),  // /path/to/dir% or ~/path%
            // Windows prompts
            Regex::new(r"^[A-Z]:\\.*>$").unwrap(),
            // Colored prompt indicators (common ANSI stripped patterns)
            Regex::new(r"^\s*\$ $").unwrap(),
            Regex::new(r"^\s*% $").unwrap(),
            Regex::new(r"^\s*> $").unwrap(),
        ]);

        // Continuation patterns (indicate command still running)
        self.continuation_patterns.extend([
            Regex::new(r"\\$").unwrap(),        // Line continuation
            Regex::new(r"^\s*>\s*$").unwrap(),  // Input redirection prompt
            Regex::new(r"^\s*\?\s*$").unwrap(), // Multi-line input prompt
            Regex::new(r"^\s*\+\s*$").unwrap(), // Continuation prompt
        ]);
    }

    /// Check if output indicates command completion
    pub fn is_command_complete(&self, lines: &[OutputLine]) -> bool {
        if lines.is_empty() {
            return false;
        }

        // Check the last line for prompt patterns
        if let Some(last_line) = lines.last() {
            // First check with completion patterns
            for pattern in &self.completion_patterns {
                if pattern.is_match(&last_line.text) {
                    return true;
                }
            }

            // Also check trimmed text for edge cases with whitespace
            let trimmed_text = last_line.text.trim();
            for pattern in &self.completion_patterns {
                if pattern.is_match(trimmed_text) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if a specific line looks like a shell prompt (for integration with PromptDetector)
    pub fn is_line_a_prompt(&self, line: &OutputLine) -> bool {
        let text = line.text.trim();
        for pattern in &self.completion_patterns {
            if pattern.is_match(text) {
                return true;
            }
        }
        false
    }

    /// Check if command is continuing (multi-line)
    pub fn is_command_continuing(&self, lines: &[OutputLine]) -> bool {
        if lines.is_empty() {
            return false;
        }

        // Check the last line for continuation patterns
        if let Some(last_line) = lines.last() {
            for pattern in &self.continuation_patterns {
                if pattern.is_match(&last_line.text) {
                    return true;
                }
            }
        }

        false
    }

    /// Add custom completion pattern
    pub fn add_completion_pattern(&mut self, pattern: &str) -> Result<()> {
        let regex = Regex::new(pattern)?;
        self.completion_patterns.push(regex);
        Ok(())
    }

    /// Add custom continuation pattern
    pub fn add_continuation_pattern(&mut self, pattern: &str) -> Result<()> {
        let regex = Regex::new(pattern)?;
        self.continuation_patterns.push(regex);
        Ok(())
    }
}

/// Utilities for prompt analysis
pub mod utils {
    use super::*;

    /// Extract prompt text from a line
    pub fn extract_prompt_text(line: &OutputLine) -> Option<String> {
        // Remove ANSI codes and extract the actual prompt
        let clean_text = strip_ansi_codes(&line.text);

        if clean_text.trim().is_empty() {
            return None;
        }

        Some(clean_text.trim().to_string())
    }

    /// Strip ANSI codes from text
    pub fn strip_ansi_codes(text: &str) -> String {
        let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[mG]").unwrap();
        ansi_regex.replace_all(text, "").to_string()
    }

    /// Check if line contains only a prompt (no other content)
    pub fn is_pure_prompt(line: &OutputLine) -> bool {
        let clean_text = strip_ansi_codes(&line.text);
        let trimmed = clean_text.trim();

        // Check if it matches common prompt patterns
        matches!(trimmed, "$" | "%" | ">" | _ if trimmed.ends_with("$") || trimmed.ends_with("%") || trimmed.ends_with(">"))
    }

    /// Get the shell type name as string
    pub fn shell_type_name(shell_type: ShellType) -> &'static str {
        match shell_type {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Ksh => "ksh",
            ShellType::Csh => "csh",
            ShellType::Tcsh => "tcsh",
            ShellType::Dash => "dash",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Other => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OutputLine;

    fn create_test_line(text: &str) -> OutputLine {
        OutputLine::new(text)
    }

    #[test]
    fn test_prompt_detector_creation() {
        let detector = PromptDetector::new();
        assert_eq!(detector.current_shell(), ShellType::Other);
    }

    #[test]
    fn test_bash_prompt_detection() {
        let mut detector = PromptDetector::new();
        let bash_prompt = create_test_line("$ ");

        assert!(detector.is_prompt(&bash_prompt));
        assert_eq!(detector.current_shell(), ShellType::Bash);
    }

    #[test]
    fn test_zsh_prompt_detection() {
        let mut detector = PromptDetector::new();
        let zsh_prompt = create_test_line("% ");

        assert!(detector.is_prompt(&zsh_prompt));
        assert_eq!(detector.current_shell(), ShellType::Zsh);
    }

    #[test]
    fn test_fish_prompt_detection() {
        let mut detector = PromptDetector::new();
        let fish_prompt = create_test_line("> ");

        assert!(detector.is_prompt(&fish_prompt));
        assert_eq!(detector.current_shell(), ShellType::Fish);
    }

    #[test]
    fn test_powershell_prompt_detection() {
        let mut detector = PromptDetector::new();
        let ps_prompt = create_test_line("PS C:\\> ");

        assert!(detector.is_prompt(&ps_prompt));
        assert_eq!(detector.current_shell(), ShellType::PowerShell);
    }

    #[test]
    fn test_cmd_prompt_detection() {
        let mut detector = PromptDetector::new();
        let cmd_prompt = create_test_line("C:\\> ");

        assert!(detector.is_prompt(&cmd_prompt));
        assert_eq!(detector.current_shell(), ShellType::Cmd);
    }

    #[test]
    fn test_non_prompt_detection() {
        let mut detector = PromptDetector::new();
        let regular_line = create_test_line("Hello, World!");

        assert!(!detector.is_prompt(&regular_line));
        assert_eq!(detector.current_shell(), ShellType::Other);
    }

    #[test]
    fn test_custom_pattern() {
        let mut detector = PromptDetector::new();
        detector.add_custom_pattern(r"^>>> $").unwrap();

        let custom_prompt = create_test_line(">>> ");
        assert!(detector.is_prompt(&custom_prompt));
    }

    #[test]
    fn test_shell_type_from_string() {
        assert_eq!(
            PromptDetector::shell_type_from_string("bash"),
            ShellType::Bash
        );
        assert_eq!(
            PromptDetector::shell_type_from_string("zsh"),
            ShellType::Zsh
        );
        assert_eq!(
            PromptDetector::shell_type_from_string("fish"),
            ShellType::Fish
        );
        assert_eq!(
            PromptDetector::shell_type_from_string("powershell"),
            ShellType::PowerShell
        );
        assert_eq!(
            PromptDetector::shell_type_from_string("cmd"),
            ShellType::Cmd
        );
        assert_eq!(
            PromptDetector::shell_type_from_string("unknown"),
            ShellType::Other
        );
    }

    #[test]
    fn test_command_completion_detector() {
        let detector = CommandCompletionDetector::new();

        let completed_lines = vec![create_test_line("output"), create_test_line("$ ")];
        let incomplete_lines = vec![create_test_line("output"), create_test_line("command \\")];

        assert!(detector.is_command_complete(&completed_lines));
        assert!(!detector.is_command_complete(&incomplete_lines));
        assert!(detector.is_command_continuing(&incomplete_lines));
    }

    #[test]
    fn test_utils_extract_prompt_text() {
        let line = create_test_line("$ ");
        assert_eq!(utils::extract_prompt_text(&line), Some("$".to_string()));
    }

    #[test]
    fn test_utils_strip_ansi_codes() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m";
        assert_eq!(utils::strip_ansi_codes(text_with_ansi), "Red text");
    }

    #[test]
    fn test_utils_is_pure_prompt() {
        let prompt_line = create_test_line("$ ");
        let content_line = create_test_line("ls -la");

        assert!(utils::is_pure_prompt(&prompt_line));
        assert!(!utils::is_pure_prompt(&content_line));
    }

    #[test]
    fn test_utils_shell_type_name() {
        assert_eq!(utils::shell_type_name(ShellType::Bash), "bash");
        assert_eq!(utils::shell_type_name(ShellType::Zsh), "zsh");
        assert_eq!(utils::shell_type_name(ShellType::Other), "unknown");
    }

    #[test]
    fn test_shell_type_detection() {
        let mut detector = PromptDetector::new();
        let lines = vec![create_test_line("Welcome to bash"), create_test_line("$ ")];

        let detected = detector.detect_shell_type(&lines);
        assert_eq!(detected, ShellType::Bash);
        assert_eq!(detector.current_shell(), ShellType::Bash);
    }
}
