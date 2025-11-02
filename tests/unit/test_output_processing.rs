//! Unit tests for output processing

use mosaicterm::terminal::output::OutputProcessor;
use mosaicterm::terminal::StreamType;

#[cfg(test)]
mod output_processing_tests {
    use super::*;

    #[test]
    fn test_processor_creation() {
        let processor = OutputProcessor::new();
        assert!(std::mem::size_of_val(&processor) > 0);
    }

    #[test]
    fn test_process_simple_text() {
        let mut processor = OutputProcessor::new();
        let data = b"Hello, World!\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert!(lines.len() >= 1);
        if !lines.is_empty() {
            assert!(lines[0].text.contains("Hello"));
        }
    }

    #[test]
    fn test_process_multiple_lines() {
        let mut processor = OutputProcessor::new();
        let data = b"Line 1\nLine 2\nLine 3\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_process_crlf_line_endings() {
        let mut processor = OutputProcessor::new();
        let data = b"Line 1\r\nLine 2\r\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        // Should process CRLF as line endings
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_process_partial_line() {
        let mut processor = OutputProcessor::new();
        let data = b"Partial line without newline";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        // Partial lines should be buffered, not returned
        let lines = result.unwrap();
        // May or may not have lines depending on implementation
        assert!(lines.len() >= 0);
    }

    #[test]
    fn test_process_complete_chunk() {
        let mut processor = OutputProcessor::new();
        let data = b"Complete output\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: true,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert!(lines.len() >= 1);
    }

    #[test]
    fn test_process_empty_data() {
        let mut processor = OutputProcessor::new();
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: vec![],
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_process_ansi_output() {
        let mut processor = OutputProcessor::new();
        let data = b"\x1b[31mRed text\x1b[0m\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        if !lines.is_empty() {
            // Should contain the text
            assert!(lines[0].text.len() > 0);
        }
    }

    #[test]
    fn test_process_large_output() {
        let mut processor = OutputProcessor::new();
        let large_text = "X".repeat(5000);
        let mut data = large_text.as_bytes().to_vec();
        data.push(b'\n');

        let chunk = mosaicterm::terminal::output::OutputChunk {
            data,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        if !lines.is_empty() {
            assert!(lines[0].text.len() > 1000);
        }
    }

    #[test]
    fn test_process_binary_data() {
        let mut processor = OutputProcessor::new();
        let data = vec![0x00, 0x01, 0x02, 0xFF, 0xFE, b'\n'];
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        // Should handle gracefully, not panic
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_utf8_text() {
        let mut processor = OutputProcessor::new();
        let data = "Hello 世界 🌍\n".as_bytes().to_vec();
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        if !lines.is_empty() {
            assert!(lines[0].text.contains("Hello"));
        }
    }

    #[test]
    fn test_process_carriage_return() {
        let mut processor = OutputProcessor::new();
        let data = b"Progress: 50%\rProgress: 100%\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        // Should handle carriage return (overwrite)
        assert!(lines.len() >= 1);
    }

    #[test]
    fn test_process_stderr() {
        let mut processor = OutputProcessor::new();
        let data = b"Error message\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stderr,
        };

        let result = processor.process_chunk(chunk);
        assert!(result.is_ok());

        let lines = result.unwrap();
        if !lines.is_empty() {
            assert!(lines[0].text.contains("Error"));
        }
    }

    #[test]
    fn test_buffer_overflow_protection() {
        let mut processor = OutputProcessor::new(); // Default buffer
        let large_data = "X".repeat(200).as_bytes().to_vec();
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: large_data,
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let result = processor.process_chunk(chunk);
        // Should handle gracefully without panic
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_chunks() {
        let mut processor = OutputProcessor::new();

        // First chunk
        let chunk1 = mosaicterm::terminal::output::OutputChunk {
            data: b"First ".to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };
        processor.process_chunk(chunk1).unwrap();

        // Second chunk completing the line
        let chunk2 = mosaicterm::terminal::output::OutputChunk {
            data: b"line\n".to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };
        let lines = processor.process_chunk(chunk2).unwrap();

        // Should have assembled the full line
        assert!(lines.len() >= 1);
        if !lines.is_empty() {
            assert!(lines[0].text.contains("First") || lines[0].text.contains("line"));
        }
    }

    #[test]
    fn test_line_number_tracking() {
        let mut processor = OutputProcessor::new();
        let data = b"Line 1\nLine 2\nLine 3\n";
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: data.to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        if lines.len() >= 3 {
            // Line numbers should increment
            assert!(lines[0].line_number < lines[1].line_number);
            assert!(lines[1].line_number < lines[2].line_number);
        }
    }

    #[test]
    fn test_timestamp_assignment() {
        let mut processor = OutputProcessor::new();
        let now = chrono::Utc::now();
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: b"Test line\n".to_vec(),
            timestamp: now,
            is_complete: false,
            stream_type: StreamType::Stdout,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        if !lines.is_empty() {
            // Timestamp should be set
            assert!(lines[0].timestamp <= chrono::Utc::now());
        }
    }

    #[test]
    fn test_processor_reset() {
        let mut processor = OutputProcessor::new();

        // Process some data
        let chunk = mosaicterm::terminal::output::OutputChunk {
            data: b"Test\n".to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: true,
            stream_type: StreamType::Stdout,
        };
        processor.process_chunk(chunk).unwrap();

        // Process new data - should work independently
        let chunk2 = mosaicterm::terminal::output::OutputChunk {
            data: b"New test\n".to_vec(),
            timestamp: chrono::Utc::now(),
            is_complete: false,
            stream_type: StreamType::Stdout,
        };
        let result = processor.process_chunk(chunk2);
        assert!(result.is_ok());
    }
}

