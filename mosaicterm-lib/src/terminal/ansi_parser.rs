//! ANSI Escape Code Parser
//!
//! Parses ANSI escape sequences from terminal output, extracting formatting
//! information and separating it from the actual text content.

use regex::Regex;
use std::collections::HashMap;
use crate::error::{Error, Result};
use crate::models::output_line::AnsiCode;

/// ANSI escape sequence parser
#[derive(Debug)]
pub struct AnsiParser {
    /// Compiled regex for ANSI escape sequences
    ansi_regex: Regex,
    /// Current parsing state
    state: ParserState,
    /// Buffer for incomplete sequences
    buffer: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParserState {
    /// Normal text processing
    Normal,
    /// Inside escape sequence
    InEscape,
    /// Processing control sequence
    InControlSequence,
}

impl AnsiParser {
    /// Create a new ANSI parser
    pub fn new() -> Self {
        Self {
            ansi_regex: Regex::new(r"\x1b\[([0-9;]*)[mG]").unwrap(),
            state: ParserState::Normal,
            buffer: String::new(),
        }
    }

    /// Parse text and extract ANSI codes
    pub fn parse(&mut self, text: &str) -> Result<ParsedText> {
        let mut result = ParsedText::new(text.to_string());
        let mut current_pos = 0;

        for capture in self.ansi_regex.find_iter(text) {
            let start = capture.start();
            let end = capture.end();

            // Add text before the escape sequence
            if start > current_pos {
                let text_before = &text[current_pos..start];
                if !text_before.is_empty() {
                    result.add_text(text_before, current_pos);
                }
            }

            // Parse the escape sequence
            let sequence = &text[start..end];
            if let Some(ansi_code) = self.parse_sequence(sequence) {
                result.add_ansi_code(ansi_code, start);
            }

            current_pos = end;
        }

        // Add remaining text after the last escape sequence
        if current_pos < text.len() {
            let remaining = &text[current_pos..];
            if !remaining.is_empty() {
                result.add_text(remaining, current_pos);
            }
        }

        Ok(result)
    }

    /// Parse a single ANSI escape sequence
    fn parse_sequence(&self, sequence: &str) -> Option<AnsiCode> {
        if !sequence.starts_with("\x1b[") {
            return None;
        }

        let content = &sequence[2..sequence.len() - 1]; // Remove \x1b[ and final character
        let parts: Vec<&str> = content.split(';').collect();

        match sequence.chars().last() {
            Some('m') => self.parse_sgr_sequence(&parts),
            Some('G') => self.parse_cursor_sequence(&parts),
            _ => None,
        }
    }

    /// Parse Select Graphic Rendition (SGR) sequences (colors, styles)
    fn parse_sgr_sequence(&self, parts: &[&str]) -> Option<AnsiCode> {
        let mut attributes = Vec::new();

        for part in parts {
            if let Ok(code) = part.parse::<u8>() {
                match code {
                    0 => attributes.push(AnsiAttribute::Reset),
                    1 => attributes.push(AnsiAttribute::Bold),
                    2 => attributes.push(AnsiAttribute::Dim),
                    3 => attributes.push(AnsiAttribute::Italic),
                    4 => attributes.push(AnsiAttribute::Underline),
                    5 | 6 => attributes.push(AnsiAttribute::Blink),
                    7 => attributes.push(AnsiAttribute::Reverse),
                    8 => attributes.push(AnsiAttribute::Hidden),
                    9 => attributes.push(AnsiAttribute::Strikethrough),
                    21 => attributes.push(AnsiAttribute::DoubleUnderline),
                    22 => attributes.push(AnsiAttribute::NormalIntensity),
                    23 => attributes.push(AnsiAttribute::NotItalic),
                    24 => attributes.push(AnsiAttribute::NotUnderlined),
                    25 => attributes.push(AnsiAttribute::NotBlinking),
                    27 => attributes.push(AnsiAttribute::NotReversed),
                    28 => attributes.push(AnsiAttribute::NotHidden),
                    29 => attributes.push(AnsiAttribute::NotStrikethrough),
                    30..=37 => {
                        let color = AnsiColor::from_ansi_code(code - 30)?;
                        attributes.push(AnsiAttribute::ForegroundColor(color));
                    }
                    40..=47 => {
                        let color = AnsiColor::from_ansi_code(code - 40)?;
                        attributes.push(AnsiAttribute::BackgroundColor(color));
                    }
                    90..=97 => {
                        let color = AnsiColor::from_bright_ansi_code(code - 90)?;
                        attributes.push(AnsiAttribute::ForegroundColor(color));
                    }
                    100..=107 => {
                        let color = AnsiColor::from_bright_ansi_code(code - 100)?;
                        attributes.push(AnsiAttribute::BackgroundColor(color));
                    }
                    _ => {} // Unknown code, ignore
                }
            }
        }

        if attributes.is_empty() {
            None
        } else {
            // Convert attributes to ANSI escape sequence string
            let code_str = Self::attributes_to_ansi_string(&attributes);
            Some(AnsiCode::new(&code_str))
        }
    }

    /// Parse cursor positioning sequences
    fn parse_cursor_sequence(&self, parts: &[&str]) -> Option<AnsiCode> {
        if let Some(&column) = parts.first() {
            if let Ok(col) = column.parse::<usize>() {
                let code_str = format!("\x1b[{}G", col);
                return Some(AnsiCode::new(&code_str));
            }
        }
        None
    }

    /// Convert ANSI attributes to escape sequence string
    fn attributes_to_ansi_string(attributes: &[AnsiAttribute]) -> String {
        if attributes.is_empty() {
            return String::new();
        }

        let mut codes = Vec::new();

        for attr in attributes {
            let code = match attr {
                AnsiAttribute::Reset => "0",
                AnsiAttribute::Bold => "1",
                AnsiAttribute::Dim => "2",
                AnsiAttribute::Italic => "3",
                AnsiAttribute::Underline => "4",
                AnsiAttribute::Blink => "5",
                AnsiAttribute::Reverse => "7",
                AnsiAttribute::Hidden => "8",
                AnsiAttribute::Strikethrough => "9",
                AnsiAttribute::DoubleUnderline => "21",
                AnsiAttribute::NormalIntensity => "22",
                AnsiAttribute::NotItalic => "23",
                AnsiAttribute::NotUnderlined => "24",
                AnsiAttribute::NotBlinking => "25",
                AnsiAttribute::NotReversed => "27",
                AnsiAttribute::NotHidden => "28",
                AnsiAttribute::NotStrikethrough => "29",
                AnsiAttribute::ForegroundColor(color) => match color {
                    AnsiColor::Black => "30",
                    AnsiColor::Red => "31",
                    AnsiColor::Green => "32",
                    AnsiColor::Yellow => "33",
                    AnsiColor::Blue => "34",
                    AnsiColor::Magenta => "35",
                    AnsiColor::Cyan => "36",
                    AnsiColor::White => "37",
                    AnsiColor::BrightBlack => "90",
                    AnsiColor::BrightRed => "91",
                    AnsiColor::BrightGreen => "92",
                    AnsiColor::BrightYellow => "93",
                    AnsiColor::BrightBlue => "94",
                    AnsiColor::BrightMagenta => "95",
                    AnsiColor::BrightCyan => "96",
                    AnsiColor::BrightWhite => "97",
                },
                AnsiAttribute::BackgroundColor(color) => match color {
                    AnsiColor::Black => "40",
                    AnsiColor::Red => "41",
                    AnsiColor::Green => "42",
                    AnsiColor::Yellow => "43",
                    AnsiColor::Blue => "44",
                    AnsiColor::Magenta => "45",
                    AnsiColor::Cyan => "46",
                    AnsiColor::White => "47",
                    AnsiColor::BrightBlack => "100",
                    AnsiColor::BrightRed => "101",
                    AnsiColor::BrightGreen => "102",
                    AnsiColor::BrightYellow => "103",
                    AnsiColor::BrightBlue => "104",
                    AnsiColor::BrightMagenta => "105",
                    AnsiColor::BrightCyan => "106",
                    AnsiColor::BrightWhite => "107",
                },
            };
            codes.push(code);
        }

        format!("\x1b[{}m", codes.join(";"))
    }

    /// Reset parser state
    pub fn reset(&mut self) {
        self.state = ParserState::Normal;
        self.buffer.clear();
    }
}

/// Parsed text result with ANSI codes separated
#[derive(Debug, Clone)]
pub struct ParsedText {
    /// Original text
    pub original_text: String,
    /// Text segments without ANSI codes
    pub clean_text: String,
    /// ANSI codes with their positions
    pub ansi_codes: Vec<AnsiCode>,
    /// Mapping of positions in clean text to original positions
    pub position_map: Vec<(usize, usize)>, // (clean_pos, original_pos)
}

impl ParsedText {
    /// Create new parsed text result
    pub fn new(original_text: String) -> Self {
        Self {
            original_text,
            clean_text: String::new(),
            ansi_codes: Vec::new(),
            position_map: Vec::new(),
        }
    }

    /// Add text segment
    pub fn add_text(&mut self, text: &str, original_pos: usize) {
        let clean_pos = self.clean_text.len();
        self.clean_text.push_str(text);
        self.position_map.push((clean_pos, original_pos));
    }

    /// Add ANSI code
    pub fn add_ansi_code(&mut self, mut ansi_code: AnsiCode, position: usize) {
        ansi_code.position = self.clean_text.len();
        self.ansi_codes.push(ansi_code);
    }

    /// Get ANSI codes that affect a specific position
    pub fn get_codes_at(&self, position: usize) -> Vec<&AnsiCode> {
        self.ansi_codes.iter()
            .filter(|code| code.position <= position)
            .collect()
    }

    /// Check if text contains any ANSI codes
    pub fn has_ansi_codes(&self) -> bool {
        !self.ansi_codes.is_empty()
    }
}

/// ANSI color representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnsiColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl AnsiColor {
    /// Create color from standard ANSI code (0-7)
    pub fn from_ansi_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(AnsiColor::Black),
            1 => Some(AnsiColor::Red),
            2 => Some(AnsiColor::Green),
            3 => Some(AnsiColor::Yellow),
            4 => Some(AnsiColor::Blue),
            5 => Some(AnsiColor::Magenta),
            6 => Some(AnsiColor::Cyan),
            7 => Some(AnsiColor::White),
            _ => None,
        }
    }

    /// Create color from bright ANSI code (0-7)
    pub fn from_bright_ansi_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(AnsiColor::BrightBlack),
            1 => Some(AnsiColor::BrightRed),
            2 => Some(AnsiColor::BrightGreen),
            3 => Some(AnsiColor::BrightYellow),
            4 => Some(AnsiColor::BrightBlue),
            5 => Some(AnsiColor::BrightMagenta),
            6 => Some(AnsiColor::BrightCyan),
            7 => Some(AnsiColor::BrightWhite),
            _ => None,
        }
    }
}

/// ANSI text attributes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnsiAttribute {
    Reset,
    Bold,
    Dim,
    Italic,
    Underline,
    Blink,
    Reverse,
    Hidden,
    Strikethrough,
    DoubleUnderline,
    NormalIntensity,
    NotItalic,
    NotUnderlined,
    NotBlinking,
    NotReversed,
    NotHidden,
    NotStrikethrough,
    ForegroundColor(AnsiColor),
    BackgroundColor(AnsiColor),
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_parser_creation() {
        let parser = AnsiParser::new();
        assert!(parser.ansi_regex.is_match("\x1b[31m"));
    }

    #[test]
    fn test_parse_simple_color() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[31mHello\x1b[0m").unwrap();

        assert_eq!(result.clean_text, "Hello");
        assert_eq!(result.ansi_codes.len(), 2); // Color + reset
    }

    #[test]
    fn test_parse_bold_text() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[1mBold\x1b[22m").unwrap();

        assert_eq!(result.clean_text, "Bold");
        assert!(result.has_ansi_codes());
    }

    #[test]
    fn test_parse_multiple_colors() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[31mRed\x1b[32mGreen\x1b[0m").unwrap();

        assert_eq!(result.clean_text, "RedGreen");
        assert_eq!(result.ansi_codes.len(), 3); // Two colors + reset
    }

    #[test]
    fn test_parse_plain_text() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("Plain text").unwrap();

        assert_eq!(result.clean_text, "Plain text");
        assert!(!result.has_ansi_codes());
    }

    #[test]
    fn test_parse_cursor_position() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[5G").unwrap();

        assert_eq!(result.clean_text, "");
        assert_eq!(result.ansi_codes.len(), 1);
    }

    #[test]
    fn test_ansi_colors() {
        assert_eq!(AnsiColor::from_ansi_code(0), Some(AnsiColor::Black));
        assert_eq!(AnsiColor::from_ansi_code(1), Some(AnsiColor::Red));
        assert_eq!(AnsiColor::from_ansi_code(8), None);

        assert_eq!(AnsiColor::from_bright_ansi_code(0), Some(AnsiColor::BrightBlack));
        assert_eq!(AnsiColor::from_bright_ansi_code(7), Some(AnsiColor::BrightWhite));
    }

    #[test]
    fn test_parsed_text_position_mapping() {
        let mut parsed = ParsedText::new("Test text".to_string());
        parsed.add_text("Test", 0);
        parsed.add_text(" text", 4);

        assert_eq!(parsed.clean_text, "Test text");
        assert_eq!(parsed.position_map.len(), 2);
    }
}
