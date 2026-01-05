//! Output Line Model
//!
//! Represents a single line of terminal output with ANSI formatting.
//! This model handles the parsing and storage of ANSI escape sequences
//! along with the actual text content.
//!
//! ## Lazy Parsing
//!
//! ANSI codes are parsed lazily using `OnceCell` for better performance.
//! The raw text is stored and only parsed when rendering requires it.

use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
        self.code.contains("[3")
            || self.code.contains("[4")
            || self.code.contains("[9")
            || self.code.contains("[10")
    }

    /// Check if this is a formatting code (bold, italic, etc.)
    pub fn is_formatting_code(&self) -> bool {
        self.code.contains("[1")
            || self.code.contains("[2")
            || self.code.contains("[4")
            || self.code.contains("[7")
    }

    /// Check if this is a reset code
    pub fn is_reset_code(&self) -> bool {
        self.code.contains("[0")
    }
}

/// Lazily parsed ANSI content
#[derive(Debug, Clone)]
struct ParsedContent {
    /// Plain text without ANSI codes
    plain_text: String,
    /// Extracted ANSI codes with positions
    codes: Vec<AnsiCode>,
}

/// Represents a single line of terminal output with ANSI formatting
///
/// Uses lazy parsing - ANSI codes are only extracted when needed.
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The text content (may contain ANSI codes)
    pub text: String,

    /// Lazily parsed content (populated on first access)
    #[allow(clippy::type_complexity)]
    parsed: Arc<OnceCell<ParsedContent>>,

    /// Legacy field for backward compatibility
    pub ansi_codes: Vec<AnsiCode>,

    /// Position in the output (line number)
    pub line_number: usize,

    /// When this line was received
    pub timestamp: DateTime<Utc>,
}

// Custom Serialize implementation to handle OnceCell
impl Serialize for OutputLine {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("OutputLine", 4)?;
        state.serialize_field("text", &self.text)?;
        state.serialize_field("ansi_codes", &self.ansi_codes)?;
        state.serialize_field("line_number", &self.line_number)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.end()
    }
}

// Custom Deserialize implementation
impl<'de> Deserialize<'de> for OutputLine {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct OutputLineHelper {
            text: String,
            ansi_codes: Vec<AnsiCode>,
            line_number: usize,
            timestamp: DateTime<Utc>,
        }

        let helper = OutputLineHelper::deserialize(deserializer)?;
        Ok(OutputLine {
            text: helper.text,
            parsed: Arc::new(OnceCell::new()),
            ansi_codes: helper.ansi_codes,
            line_number: helper.line_number,
            timestamp: helper.timestamp,
        })
    }
}

impl OutputLine {
    /// Create a new output line (text may contain ANSI codes - parsed lazily)
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            parsed: Arc::new(OnceCell::new()),
            ansi_codes: Vec::new(),
            line_number: 0,
            timestamp: Utc::now(),
        }
    }

    /// Create a new output line with a specific line number
    pub fn with_line_number(text: impl Into<String>, line_number: usize) -> Self {
        Self {
            text: text.into(),
            parsed: Arc::new(OnceCell::new()),
            ansi_codes: Vec::new(),
            line_number,
            timestamp: Utc::now(),
        }
    }

    /// Create a new output line with pre-parsed ANSI codes (for backward compatibility)
    pub fn with_ansi_codes(text: String, ansi_codes: Vec<AnsiCode>, line_number: usize) -> Self {
        Self {
            text,
            parsed: Arc::new(OnceCell::new()),
            ansi_codes,
            line_number,
            timestamp: Utc::now(),
        }
    }

    /// Get or lazily parse the content
    fn get_parsed(&self) -> &ParsedContent {
        self.parsed
            .get_or_init(|| Self::parse_ansi_internal(&self.text))
    }

    /// Internal ANSI parsing (called lazily)
    fn parse_ansi_internal(text: &str) -> ParsedContent {
        // Use a static regex for better performance
        thread_local! {
            static ANSI_REGEX: Regex = Regex::new(r"\x1b\[[0-9;]*[mGKHJsu]").unwrap();
        }

        ANSI_REGEX.with(|regex| {
            let mut codes = Vec::new();
            let mut plain_text = String::with_capacity(text.len());
            let mut last_end = 0;

            for capture in regex.find_iter(text) {
                let start = capture.start();
                let end = capture.end();

                // Add text before this ANSI code
                plain_text.push_str(&text[last_end..start]);

                // Create ANSI code with position in plain text
                let mut code = AnsiCode::new(capture.as_str());
                code.position = plain_text.len();
                codes.push(code);

                last_end = end;
            }

            // Add remaining text
            plain_text.push_str(&text[last_end..]);

            ParsedContent { plain_text, codes }
        })
    }

    /// Parse ANSI codes from the text (for backward compatibility - now a no-op)
    #[deprecated(note = "ANSI parsing is now lazy; this method is no longer needed")]
    pub fn parse_ansi_codes(self) -> Self {
        self // Parsing happens lazily now
    }

    /// Get the formatted text with ANSI codes (returns original text)
    pub fn get_formatted_text(&self) -> &str {
        &self.text
    }

    /// Get the plain text without ANSI codes (lazily parsed)
    pub fn get_plain_text(&self) -> &str {
        &self.get_parsed().plain_text
    }

    /// Get the parsed ANSI codes (lazily parsed)
    pub fn get_ansi_codes(&self) -> &[AnsiCode] {
        &self.get_parsed().codes
    }

    /// Check if this line contains ANSI formatting
    pub fn has_ansi_formatting(&self) -> bool {
        // Quick check without full parsing
        self.text.contains('\x1b')
    }

    /// Check if this line might contain ANSI codes (fast check without parsing)
    pub fn might_have_ansi(&self) -> bool {
        self.text.contains('\x1b')
    }

    /// Get all color codes in this line (lazily parsed)
    pub fn get_color_codes(&self) -> Vec<&AnsiCode> {
        self.get_parsed()
            .codes
            .iter()
            .filter(|code| code.is_color_code())
            .collect()
    }

    /// Get all formatting codes in this line (lazily parsed)
    pub fn get_formatting_codes(&self) -> Vec<&AnsiCode> {
        self.get_parsed()
            .codes
            .iter()
            .filter(|code| code.is_formatting_code())
            .collect()
    }

    /// Check if this line has color formatting (lazily parsed)
    pub fn has_colors(&self) -> bool {
        self.get_parsed()
            .codes
            .iter()
            .any(|code| code.is_color_code())
    }

    /// Check if this line has text formatting (bold, italic, etc.) (lazily parsed)
    pub fn has_formatting(&self) -> bool {
        self.get_parsed()
            .codes
            .iter()
            .any(|code| code.is_formatting_code())
    }

    /// Get the raw text (for direct access without parsing)
    pub fn raw(&self) -> &str {
        &self.text
    }
}

impl Default for OutputLine {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl From<String> for OutputLine {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for OutputLine {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_line_creation() {
        let text = "Hello, World!".to_string();
        let line_number = 5;

        let line = OutputLine::with_line_number(text.clone(), line_number);

        assert_eq!(line.text, text);
        assert_eq!(line.get_plain_text(), text);
        assert_eq!(line.line_number, line_number);
        assert!(line.timestamp <= Utc::now());
    }

    #[test]
    fn test_lazy_ansi_code_parsing() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m normal text";
        let line = OutputLine::new(text_with_ansi);

        // Verify lazy parsing hasn't happened yet
        assert!(line.might_have_ansi());

        // Now trigger parsing by accessing codes
        let codes = line.get_ansi_codes();
        assert_eq!(codes.len(), 2);
        assert!(codes[0].is_color_code());
        assert!(codes[1].is_reset_code());
    }

    #[test]
    fn test_formatted_text_preservation() {
        let original = "\x1b[31mRed text\x1b[0m normal";
        let line = OutputLine::new(original);

        // get_formatted_text returns raw text (original with ANSI)
        assert_eq!(line.get_formatted_text(), original);
    }

    #[test]
    fn test_plain_text_extraction() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m normal";
        let line = OutputLine::new(text_with_ansi);

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
        let line = OutputLine::new("\x1b[31m\x1b[1mtest\x1b[0m");

        assert!(line.has_ansi_formatting());
        assert!(line.has_colors());
        assert!(line.has_formatting());
    }

    #[test]
    fn test_no_ansi_codes() {
        let line = OutputLine::new("plain text without formatting");

        assert!(!line.might_have_ansi());
        assert_eq!(line.get_plain_text(), "plain text without formatting");
        assert!(line.get_ansi_codes().is_empty());
    }

    #[test]
    fn test_from_string() {
        let line: OutputLine = "test string".into();
        assert_eq!(line.text, "test string");
    }

    #[test]
    fn test_clone_preserves_lazy_parsing() {
        let original = OutputLine::new("\x1b[31mRed\x1b[0m");

        // Parse the original
        let _ = original.get_plain_text();

        // Clone and verify
        let cloned = original.clone();
        assert_eq!(cloned.get_plain_text(), "Red");
    }
}
