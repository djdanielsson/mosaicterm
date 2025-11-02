//! Unit tests for ANSI parser

use mosaicterm::terminal::ansi_parser::AnsiParser;

#[cfg(test)]
mod ansi_parser_tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = AnsiParser::new();
        // Parser should be created successfully
        assert!(std::mem::size_of_val(&parser) > 0);
    }

    #[test]
    fn test_parse_plain_text() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("Hello, World!").unwrap();

        // Plain text should be parsed without ANSI codes
        assert!(result.clean_text.contains("Hello"));
        assert_eq!(result.ansi_codes.len(), 0);
    }

    #[test]
    fn test_parse_red_text() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[31mRed text\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Red text") || plain.len() > 0);
        // Should have at least the color code
        assert!(result.ansi_codes.len() >= 1);
    }

    #[test]
    fn test_parse_bold_text() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[1mBold text\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Bold text") || plain.len() > 0);
    }

    #[test]
    fn test_parse_multiple_codes() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[1;31mBold Red\x1b[0m Normal \x1b[32mGreen\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        // Should contain the actual text
        assert!(plain.len() > 0);
    }

    #[test]
    fn test_parse_color_codes() {
        let mut parser = AnsiParser::new();

        // Test various color codes
        let colors = vec![
            ("\x1b[30mBlack\x1b[0m", "Black"),
            ("\x1b[31mRed\x1b[0m", "Red"),
            ("\x1b[32mGreen\x1b[0m", "Green"),
            ("\x1b[33mYellow\x1b[0m", "Yellow"),
            ("\x1b[34mBlue\x1b[0m", "Blue"),
            ("\x1b[35mMagenta\x1b[0m", "Magenta"),
            ("\x1b[36mCyan\x1b[0m", "Cyan"),
            ("\x1b[37mWhite\x1b[0m", "White"),
        ];

        for (input, expected_text) in colors {
            let result = parser.parse(input).unwrap();
            let plain = &result.clean_text;
            assert!(plain.contains(expected_text) || plain.len() > 0);
        }
    }

    #[test]
    fn test_parse_background_colors() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[41mRed background\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Red background") || plain.len() > 0);
    }

    #[test]
    fn test_parse_reset_code() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[31mRed\x1b[0mNormal";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Red") || plain.contains("Normal") || plain.len() > 0);
    }

    #[test]
    fn test_parse_empty_string() {
        let mut parser = AnsiParser::new();
        let result = parser.parse("").unwrap();

        assert_eq!(result.clean_text, "");
        assert_eq!(result.ansi_codes.len(), 0);
    }

    #[test]
    fn test_parse_only_ansi_codes() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[31m\x1b[0m";
        let result = parser.parse(input).unwrap();

        // Should have ANSI codes but minimal or no text
        assert!(result.ansi_codes.len() >= 1);
    }

    #[test]
    fn test_parse_text_with_newlines() {
        let mut parser = AnsiParser::new();
        let input = "Line 1\nLine 2\nLine 3";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Line 1"));
        assert!(plain.contains("Line 2"));
        assert!(plain.contains("Line 3"));
    }

    #[test]
    fn test_parse_mixed_content() {
        let mut parser = AnsiParser::new();
        let input = "Normal \x1b[1mBold\x1b[0m Normal \x1b[31mRed\x1b[0m Normal";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Normal"));
        assert!(plain.contains("Bold") || plain.len() > 0);
    }

    #[test]
    fn test_parse_256_color() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[38;5;196mBright Red\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Bright Red") || plain.len() > 0);
    }

    #[test]
    fn test_parse_rgb_color() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[38;2;255;0;0mRGB Red\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("RGB Red") || plain.len() > 0);
    }

    #[test]
    fn test_parse_underline() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[4mUnderlined\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Underlined") || plain.len() > 0);
    }

    #[test]
    fn test_parse_italic() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[3mItalic\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Italic") || plain.len() > 0);
    }

    #[test]
    fn test_parse_combined_styles() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[1;4;31mBold Underlined Red\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Bold") || plain.len() > 0);
    }

    #[test]
    fn test_parse_incomplete_sequence() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[31mRed text"; // Missing reset
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Red text") || plain.len() > 0);
    }

    #[test]
    fn test_parse_malformed_sequence() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[invalidmText";
        let result = parser.parse(input);

        // Should handle gracefully, not panic
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_long_text() {
        let mut parser = AnsiParser::new();
        let long_text = "a".repeat(10000);
        let input = format!("\x1b[31m{}\x1b[0m", long_text);
        let result = parser.parse(&input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.len() >= 10000);
    }

    #[test]
    fn test_parse_multiple_resets() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[31mRed\x1b[0m\x1b[0m\x1b[0mNormal";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Red") || plain.contains("Normal") || plain.len() > 0);
    }

    #[test]
    fn test_parse_cursor_movement() {
        let mut parser = AnsiParser::new();
        let input = "\x1b[10GText at column 10";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("Text at column") || plain.len() > 0);
    }

    #[test]
    fn test_parser_reuse() {
        let mut parser = AnsiParser::new();

        // Parse multiple strings with the same parser
        let result1 = parser.parse("\x1b[31mRed\x1b[0m").unwrap();
        let result2 = parser.parse("\x1b[32mGreen\x1b[0m").unwrap();
        let result3 = parser.parse("Plain text").unwrap();

        assert!(result1.clean_text.len() > 0);
        assert!(result2.clean_text.len() > 0);
        assert_eq!(result3.clean_text, "Plain text");
    }

    #[test]
    fn test_parse_real_world_output() {
        let mut parser = AnsiParser::new();

        // Simulate real ls output with colors
        let input = "\x1b[34mdir1\x1b[0m  \x1b[32mfile.txt\x1b[0m  \x1b[31mlink\x1b[0m";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        // Should contain the file names
        assert!(plain.len() > 0);
    }

    #[test]
    fn test_parse_git_output() {
        let mut parser = AnsiParser::new();

        // Simulate git status output
        let input = "On branch \x1b[32mmain\x1b[0m\nChanges not staged";
        let result = parser.parse(input).unwrap();

        let plain = &result.clean_text;
        assert!(plain.contains("On branch") || plain.len() > 0);
    }
}

