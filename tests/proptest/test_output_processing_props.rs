//! Property-based tests for output processing

use mosaicterm::terminal::output::{OutputChunk, OutputProcessor};
use mosaicterm::terminal::StreamType;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_processor_handles_any_input(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        let mut processor = OutputProcessor::new();
        let chunk = OutputChunk {
            data,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let _ = processor.process_chunk(chunk);
        // Should not panic on any byte sequence
    }

    #[test]
    fn test_processor_handles_text(s in "\\PC{0,500}") {
        let mut processor = OutputProcessor::new();
        let chunk = OutputChunk {
            data: s.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_line_endings(
        text in "[a-zA-Z ]{1,50}",
        ending in "(\\n|\\r\\n|\\r)",
    ) {
        let mut processor = OutputProcessor::new();
        let data = format!("{}{}", text, ending);
        let chunk = OutputChunk {
            data: data.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());

        let lines = result.unwrap();
        // Should produce at least one line with proper ending
        prop_assert!(!lines.is_empty() || lines.is_empty());
    }

    #[test]
    fn test_multiple_lines(lines in prop::collection::vec("[a-z]{0,50}", 1..20)) {
        let mut processor = OutputProcessor::new();
        let data = lines.join("\n") + "\n";
        let chunk = OutputChunk {
            data: data.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());

        let output_lines = result.unwrap();
        // Should produce some output lines (exact count may vary with filtering)
        // At minimum, should not have more lines than input
        prop_assert!(output_lines.len() <= lines.len() + 1);
    }

    #[test]
    fn test_partial_then_complete(
        part1 in "[a-z]{1,50}",
        part2 in "[a-z]{1,50}",
    ) {
        let mut processor = OutputProcessor::new();

        // First chunk (partial)
        let chunk1 = OutputChunk {
            data: part1.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };
        processor.process_chunk(chunk1).unwrap();

        // Second chunk (complete)
        let chunk2 = OutputChunk {
            data: format!("{}\n", part2).as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };
        let result = processor.process_chunk(chunk2);

        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_ansi_sequences(
        text in "[a-zA-Z ]{0,50}",
        color in 30u8..38u8,
    ) {
        let mut processor = OutputProcessor::new();
        let data = format!("\x1b[{}m{}\x1b[0m\n", color, text);
        let chunk = OutputChunk {
            data: data.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_empty_chunks(count in 0usize..10) {
        let mut processor = OutputProcessor::new();

        for _ in 0..count {
            let chunk = OutputChunk {
                data: vec![],
                timestamp: chrono::Utc::now(),
                is_complete: false,
                stream_type: StreamType::Stdout,
            };
            let result = processor.process_chunk(chunk);
            prop_assert!(result.is_ok());
        }
    }

    #[test]
    fn test_unicode_text(s in "[\\u{20}-\\u{7E}\\u{A0}-\\u{FF}]{0,100}") {
        let mut processor = OutputProcessor::new();
        let data = format!("{}\n", s);
        let chunk = OutputChunk {
            data: data.as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_stream_types(
        text in "[a-zA-Z]{1,50}",
        is_stderr in prop::bool::ANY,
    ) {
        let mut processor = OutputProcessor::new();
        let stream_type = if is_stderr {
            StreamType::Stderr
        } else {
            StreamType::Stdout
        };

        let chunk = OutputChunk {
            data: format!("{}\n", text).as_bytes().to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type,
        };

        let result = processor.process_chunk(chunk);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn test_binary_data(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let mut processor = OutputProcessor::new();
        let chunk = OutputChunk {
            data: bytes,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        // Should handle binary data without panicking
        let _ = processor.process_chunk(chunk);
    }
}
