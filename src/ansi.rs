//! ANSI escape code processing
//!
//! This module handles parsing and processing of ANSI escape sequences
//! for proper display of colored and formatted terminal output.

use crate::Result;
use regex::Regex;

/// ANSI escape sequence parser
pub struct AnsiParser {
    /// Regex for ANSI escape sequences
    escape_regex: Regex,
    /// Current parser state
    state: ParserState,
}

#[derive(Debug, Clone, Default)]
struct ParserState {
    /// Current foreground color
    fg_color: Option<Color>,
    /// Current background color  
    bg_color: Option<Color>,
    /// Current text attributes
    attributes: Vec<TextAttribute>,
}

impl AnsiParser {
    /// Create a new ANSI parser
    pub fn new() -> Self {
        let escape_regex = Regex::new(r"\x1b\[[0-9;]*[mK]").unwrap();
        Self {
            escape_regex,
            state: ParserState::default(),
        }
    }

    /// Parse ANSI sequences from text
    pub fn parse(&mut self, text: &str) -> Result<Vec<AnsiSegment>> {
        let mut segments = Vec::new();
        let mut last_end = 0;

        // Collect all matches first to avoid borrowing issues
        let matches: Vec<_> = self.escape_regex.find_iter(text).collect();

        for mat in matches {
            // Add text before this escape sequence
            if mat.start() > last_end {
                let text_content = &text[last_end..mat.start()];
                if !text_content.is_empty() {
                    segments.push(AnsiSegment {
                        text: text_content.to_string(),
                        foreground_color: self.state.fg_color.clone(),
                        background_color: self.state.bg_color.clone(),
                        attributes: self.state.attributes.clone(),
                    });
                }
            }

            // Process the escape sequence
            let escape_seq = mat.as_str();
            self.process_escape_sequence(escape_seq);

            last_end = mat.end();
        }

        // Add remaining text after last escape sequence
        if last_end < text.len() {
            let remaining_text = &text[last_end..];
            if !remaining_text.is_empty() {
                segments.push(AnsiSegment {
                    text: remaining_text.to_string(),
                    foreground_color: self.state.fg_color.clone(),
                    background_color: self.state.bg_color.clone(),
                    attributes: self.state.attributes.clone(),
                });
            }
        }

        // If no escape sequences found, return the whole text as one segment
        if segments.is_empty() && !text.is_empty() {
            segments.push(AnsiSegment {
                text: text.to_string(),
                foreground_color: None,
                background_color: None,
                attributes: Vec::new(),
            });
        }

        Ok(segments)
    }

    /// Process a single escape sequence
    fn process_escape_sequence(&mut self, seq: &str) {
        // Remove the ESC[ prefix and the final character
        let seq = seq.trim_start_matches('\x1b').trim_start_matches('[');
        let seq = seq.trim_end_matches(char::is_alphabetic);

        if seq.is_empty() {
            return;
        }

        // Split by semicolon to get individual codes
        let codes: Vec<u32> = seq.split(';').filter_map(|s| s.parse().ok()).collect();

        for &code in &codes {
            match code {
                0 => self.reset_formatting(),
                1 => self.add_attribute(TextAttribute::Bold),
                3 => self.add_attribute(TextAttribute::Italic),
                4 => self.add_attribute(TextAttribute::Underline),
                22 => self.remove_attribute(&TextAttribute::Bold),
                23 => self.remove_attribute(&TextAttribute::Italic),
                24 => self.remove_attribute(&TextAttribute::Underline),
                30..=37 => self.set_foreground_color(Self::ansi_color_to_rgb(code - 30)),
                40..=47 => self.set_background_color(Self::ansi_color_to_rgb(code - 40)),
                90..=97 => self.set_foreground_color(Self::ansi_bright_color_to_rgb(code - 90)),
                100..=107 => self.set_background_color(Self::ansi_bright_color_to_rgb(code - 100)),
                _ => {
                    // Ignore unknown codes
                }
            }
        }
    }

    /// Reset all formatting to default
    fn reset_formatting(&mut self) {
        self.state.fg_color = None;
        self.state.bg_color = None;
        self.state.attributes.clear();
    }

    /// Add a text attribute
    fn add_attribute(&mut self, attr: TextAttribute) {
        if !self.state.attributes.contains(&attr) {
            self.state.attributes.push(attr);
        }
    }

    /// Remove a text attribute
    fn remove_attribute(&mut self, attr: &TextAttribute) {
        self.state.attributes.retain(|a| a != attr);
    }

    /// Set foreground color
    fn set_foreground_color(&mut self, color: Color) {
        self.state.fg_color = Some(color);
    }

    /// Set background color
    fn set_background_color(&mut self, color: Color) {
        self.state.bg_color = Some(color);
    }

    /// Convert ANSI color code to RGB
    fn ansi_color_to_rgb(code: u32) -> Color {
        match code {
            0 => Color { r: 0, g: 0, b: 0 },   // Black
            1 => Color { r: 128, g: 0, b: 0 }, // Red
            2 => Color { r: 0, g: 128, b: 0 }, // Green
            3 => Color {
                r: 128,
                g: 128,
                b: 0,
            }, // Yellow
            4 => Color { r: 0, g: 0, b: 128 }, // Blue
            5 => Color {
                r: 128,
                g: 0,
                b: 128,
            }, // Magenta
            6 => Color {
                r: 0,
                g: 128,
                b: 128,
            }, // Cyan
            7 => Color {
                r: 192,
                g: 192,
                b: 192,
            }, // White
            _ => Color {
                r: 255,
                g: 255,
                b: 255,
            }, // Default
        }
    }

    /// Convert ANSI bright color code to RGB
    fn ansi_bright_color_to_rgb(code: u32) -> Color {
        match code {
            0 => Color {
                r: 128,
                g: 128,
                b: 128,
            }, // Bright Black (Gray)
            1 => Color { r: 255, g: 0, b: 0 }, // Bright Red
            2 => Color { r: 0, g: 255, b: 0 }, // Bright Green
            3 => Color {
                r: 255,
                g: 255,
                b: 0,
            }, // Bright Yellow
            4 => Color { r: 0, g: 0, b: 255 }, // Bright Blue
            5 => Color {
                r: 255,
                g: 0,
                b: 255,
            }, // Bright Magenta
            6 => Color {
                r: 0,
                g: 255,
                b: 255,
            }, // Bright Cyan
            7 => Color {
                r: 255,
                g: 255,
                b: 255,
            }, // Bright White
            _ => Color {
                r: 255,
                g: 255,
                b: 255,
            }, // Default
        }
    }
}

impl Default for AnsiParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed ANSI text segment
#[derive(Debug, Clone)]
pub struct AnsiSegment {
    pub text: String,
    pub foreground_color: Option<Color>,
    pub background_color: Option<Color>,
    pub attributes: Vec<TextAttribute>,
}

/// ANSI color representation
#[derive(Debug, Clone, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Text formatting attributes
#[derive(Debug, Clone, PartialEq)]
pub enum TextAttribute {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Dim,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_parser_creation() {
        let parser = AnsiParser::new();
        // Just test that it doesn't panic
        assert!(parser.escape_regex.is_match("\x1b[31m"));
    }

    #[test]
    fn test_simple_color_parsing() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[31mred text\x1b[0m").unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "red text");
        assert!(result[0].foreground_color.is_some());
    }

    #[test]
    fn test_plain_text() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("plain text").unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "plain text");
        assert!(result[0].foreground_color.is_none());
    }

    #[test]
    fn test_bold_text() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("\x1b[1mbold text\x1b[0m").unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "bold text");
        assert!(result[0].attributes.contains(&TextAttribute::Bold));
    }

    #[test]
    fn test_color_constants() {
        let red = AnsiParser::ansi_color_to_rgb(1);
        assert_eq!(red, Color { r: 128, g: 0, b: 0 });

        let bright_red = AnsiParser::ansi_bright_color_to_rgb(1);
        assert_eq!(bright_red, Color { r: 255, g: 0, b: 0 });
    }
}
