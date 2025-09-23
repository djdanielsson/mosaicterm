//! Output Line Model
//!
//! Represents a single line of terminal output with ANSI formatting.
//! This model handles the parsing and storage of ANSI escape sequences
//! along with the actual text content.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use regex::Regex;

/// ANSI escape sequence representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsiCode {
    /// The raw ANSI escape sequence
    pub code: String,
    /// Position in the text where this code appears
    pub position: usize,
}

impl AnsiCode {
    /// Create a new ANSI code
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
            position: 0,
        }
    }

    /// Check if this is a color code
    pub fn is_color_code(&self) -> bool {
        self.code.contains("[3") || self.code.contains("[4") ||
        self.code.contains("[9") || self.code.contains("[10")
    }

    /// Check if this is a formatting code (bold, italic, etc.)
    pub fn is_formatting_code(&self) -> bool {
        self.code.contains("[1") || self.code.contains("[2") ||
        self.code.contains("[4") || self.code.contains("[7")
    }

    /// Check if this is a reset code
    pub fn is_reset_code(&self) -> bool {
        self.code.contains("[0")
    }
}

/// Represents a single line of terminal output with ANSI formatting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputLine {
    /// The actual text content
    pub text: String,

    /// ANSI escape sequences for formatting
    pub ansi_codes: Vec<AnsiCode>,

    /// Position in the output (line number)
    pub line_number: usize,

    /// When this line was received
    pub timestamp: DateTime<Utc>,
}

impl OutputLine {
    /// Create a new output line
    pub fn new(text: String, line_number: usize) -> Self {
        Self {
            text,
            ansi_codes: Vec::new(),
            line_number,
            timestamp: Utc::now(),
        }
    }

    /// Create a new output line with ANSI codes
    pub fn with_ansi_codes(text: String, ansi_codes: Vec<AnsiCode>, line_number: usize) -> Self {
        Self {
            text,
            ansi_codes,
            line_number,
            timestamp: Utc::now(),
        }
    }

    /// Parse ANSI codes from the text and separate them
    pub fn parse_ansi_codes(mut self) -> Self {
        let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[mG]").unwrap();

        let mut codes = Vec::new();
        let mut new_text = String::new();
        let mut last_end = 0;

        for capture in ansi_regex.find_iter(&self.text) {
            let start = capture.start();
            let end = capture.end();

            // Add text before this ANSI code
            new_text.push_str(&self.text[last_end..start]);

            // Create ANSI code
            let mut code = AnsiCode::new(capture.as_str());
            code.position = new_text.len();
            codes.push(code);

            last_end = end;
        }

        // Add remaining text
        new_text.push_str(&self.text[last_end..]);

        self.text = new_text;
        self.ansi_codes = codes;
        self
    }

    /// Get the formatted text with ANSI codes reinserted
    pub fn get_formatted_text(&self) -> String {
        if self.ansi_codes.is_empty() {
            return self.text.clone();
        }

        let mut result = String::new();
        let mut text_pos = 0;

        for code in &self.ansi_codes {
            // Add text before this ANSI code
            if text_pos < code.position && text_pos < self.text.len() {
                let end_pos = (code.position).min(self.text.len());
                result.push_str(&self.text[text_pos..end_pos]);
                text_pos = end_pos;
            }

            // Add the ANSI code
            result.push_str(&code.code);
        }

        // Add remaining text
        if text_pos < self.text.len() {
            result.push_str(&self.text[text_pos..]);
        }

        result
    }

    /// Get the plain text without ANSI codes
    pub fn get_plain_text(&self) -> &str {
        &self.text
    }

    /// Check if this line contains ANSI formatting
    pub fn has_ansi_formatting(&self) -> bool {
        !self.ansi_codes.is_empty()
    }

    /// Get all color codes in this line
    pub fn get_color_codes(&self) -> Vec<&AnsiCode> {
        self.ansi_codes.iter().filter(|code| code.is_color_code()).collect()
    }

    /// Get all formatting codes in this line
    pub fn get_formatting_codes(&self) -> Vec<&AnsiCode> {
        self.ansi_codes.iter().filter(|code| code.is_formatting_code()).collect()
    }

    /// Check if this line has color formatting
    pub fn has_colors(&self) -> bool {
        self.ansi_codes.iter().any(|code| code.is_color_code())
    }

    /// Check if this line has text formatting (bold, italic, etc.)
    pub fn has_formatting(&self) -> bool {
        self.ansi_codes.iter().any(|code| code.is_formatting_code())
    }
}

impl Default for OutputLine {
    fn default() -> Self {
        Self::new(String::new(), 0)
    }
}

impl From<String> for OutputLine {
    fn from(text: String) -> Self {
        Self::new(text, 0)
    }
}

impl From<&str> for OutputLine {
    fn from(text: &str) -> Self {
        Self::new(text.to_string(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_line_creation() {
        let text = "Hello, World!".to_string();
        let line_number = 5;

        let line = OutputLine::new(text.clone(), line_number);

        assert_eq!(line.text, text);
        assert!(line.ansi_codes.is_empty());
        assert_eq!(line.line_number, line_number);
        assert!(line.timestamp <= Utc::now());
    }

    #[test]
    fn test_ansi_code_parsing() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m normal text".to_string();
        let line = OutputLine::new(text_with_ansi, 0).parse_ansi_codes();

        assert!(!line.ansi_codes.is_empty());
        assert_eq!(line.ansi_codes.len(), 2);
        assert!(line.ansi_codes[0].is_color_code());
        assert!(line.ansi_codes[1].is_reset_code());
    }

    #[test]
    fn test_formatted_text_reconstruction() {
        let original = "\x1b[31mRed text\x1b[0m normal";
        let line = OutputLine::new(original.to_string(), 0).parse_ansi_codes();
        let reconstructed = line.get_formatted_text();

        assert_eq!(reconstructed, original);
    }

    #[test]
    fn test_plain_text_extraction() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m normal";
        let line = OutputLine::new(text_with_ansi.to_string(), 0).parse_ansi_codes();

        assert_eq!(line.get_plain_text(), "Red text normal");
    }

    #[test]
    fn test_ansi_code_detection() {
        let color_code = AnsiCode::new("\x1b[31m");
        let format_code = AnsiCode::new("\x1b[1m");
        let reset_code = AnsiCode::new("\x1b[0m");

        assert!(color_code.is_color_code());
        assert!(format_code.is_formatting_code());
        assert!(reset_code.is_reset_code());
    }

    #[test]
    fn test_line_formatting_detection() {
        let mut line = OutputLine::new("test".to_string(), 0);
        line.ansi_codes.push(AnsiCode::new("\x1b[31m")); // Color
        line.ansi_codes.push(AnsiCode::new("\x1b[1m"));  // Bold

        assert!(line.has_ansi_formatting());
        assert!(line.has_colors());
        assert!(line.has_formatting());
    }
}
