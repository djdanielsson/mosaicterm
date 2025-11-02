//! Property-based tests for ANSI parser
//!
//! These tests use proptest to generate random inputs and verify
//! that the ANSI parser handles them correctly without panicking.

use mosaicterm::terminal::ansi_parser::AnsiParser;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_parser_doesnt_panic_on_random_input(s in "\\PC*") {
        let mut parser = AnsiParser::new();
        let _ = parser.parse(&s);
        // Should not panic, regardless of input
    }

    #[test]
    fn test_parser_handles_random_strings(s in "[a-zA-Z0-9 ]{0,1000}") {
        let mut parser = AnsiParser::new();
        let result = parser.parse(&s);
        prop_assert!(result.is_ok());
        
        let parsed = result.unwrap();
        // Clean text should not be longer than original
        prop_assert!(parsed.clean_text.len() <= s.len());
    }

    #[test]
    fn test_parser_handles_ansi_sequences(
        text in "[a-zA-Z ]{0,100}",
        color_code in 30u8..38u8,
    ) {
        let mut parser = AnsiParser::new();
        let input = format!("\x1b[{}m{}\x1b[0m", color_code, text);
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
        let parsed = result.unwrap();
        prop_assert!(parsed.ansi_codes.len() >= 1);
    }

    #[test]
    fn test_parser_preserves_text_content(s in "[a-zA-Z0-9 ]{1,100}") {
        let mut parser = AnsiParser::new();
        let result = parser.parse(&s);
        
        prop_assert!(result.is_ok());
        let parsed = result.unwrap();
        // For plain text, clean_text should match input
        prop_assert_eq!(parsed.clean_text, s);
    }

    #[test]
    fn test_multiple_ansi_codes(
        text in "[a-zA-Z ]{0,50}",
        codes in prop::collection::vec(30u8..48u8, 1..10),
    ) {
        let mut parser = AnsiParser::new();
        let mut input = String::new();
        
        for code in &codes {
            input.push_str(&format!("\x1b[{}m", code));
        }
        input.push_str(&text);
        input.push_str("\x1b[0m");
        
        let result = parser.parse(&input);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_parser_handles_empty_sequences(count in 0usize..20) {
        let mut parser = AnsiParser::new();
        let input = "\x1b[0m".repeat(count);
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
        let parsed = result.unwrap();
        prop_assert_eq!(parsed.clean_text, "");
    }

    #[test]
    fn test_parser_with_newlines(
        lines in prop::collection::vec("[a-z]{0,50}", 1..20),
    ) {
        let mut parser = AnsiParser::new();
        let input = lines.join("\n");
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
        let parsed = result.unwrap();
        prop_assert!(parsed.clean_text.contains('\n') || lines.len() == 1);
    }

    #[test]
    fn test_256_color_codes(
        text in "[a-zA-Z ]{0,50}",
        color in 0u8..=255u8,
    ) {
        let mut parser = AnsiParser::new();
        let input = format!("\x1b[38;5;{}m{}\x1b[0m", color, text);
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_rgb_colors(
        text in "[a-zA-Z ]{0,50}",
        r in 0u8..=255u8,
        g in 0u8..=255u8,
        b in 0u8..=255u8,
    ) {
        let mut parser = AnsiParser::new();
        let input = format!("\x1b[38;2;{};{};{}m{}\x1b[0m", r, g, b, text);
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_malformed_sequences(
        text in "[a-zA-Z ]{0,50}",
        junk in "[0-9;]{0,20}",
    ) {
        let mut parser = AnsiParser::new();
        let input = format!("\x1b[{}m{}", junk, text);
        let result = parser.parse(&input);
        
        // Should handle gracefully, not panic
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_parser_reuse(inputs in prop::collection::vec("\\PC{0,100}", 1..10)) {
        let mut parser = AnsiParser::new();
        
        for input in inputs {
            let _ = parser.parse(&input);
            // Parser should be reusable
        }
    }

    #[test]
    fn test_combined_styles(
        text in "[a-zA-Z ]{0,50}",
        bold in prop::bool::ANY,
        underline in prop::bool::ANY,
        color in 30u8..38u8,
    ) {
        let mut parser = AnsiParser::new();
        let mut codes = vec![color.to_string()];
        if bold {
            codes.push("1".to_string());
        }
        if underline {
            codes.push("4".to_string());
        }
        
        let input = format!("\x1b[{}m{}\x1b[0m", codes.join(";"), text);
        let result = parser.parse(&input);
        
        prop_assert!(result.is_ok());
    }
}

#[cfg(test)]
mod additional_props {
    use super::*;

    proptest! {
        #[test]
        fn test_parser_output_length(s in "\\PC{0,500}") {
            let mut parser = AnsiParser::new();
            if let Ok(parsed) = parser.parse(&s) {
                // Clean text should never be longer than original + some tolerance
                prop_assert!(parsed.clean_text.len() <= s.len() + 100);
            }
        }

        #[test]
        fn test_unicode_handling(s in "[\\u{0}-\\u{10FFFF}]{0,100}") {
            let mut parser = AnsiParser::new();
            let _ = parser.parse(&s);
            // Should handle any Unicode without panicking
        }

        #[test]
        fn test_control_characters(
            text in "[a-zA-Z ]{0,50}",
            ctrl_chars in prop::collection::vec(0u8..32u8, 0..10),
        ) {
            let mut parser = AnsiParser::new();
            let mut input = text.clone();
            for ch in ctrl_chars {
                input.push(ch as char);
            }
            
            let _ = parser.parse(&input);
            // Should handle control characters
        }
    }
}

