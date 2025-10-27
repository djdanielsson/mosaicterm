//! Output Processing and Segmentation
//!
//! Processes terminal output, segments it into logical chunks,
//! and handles ANSI escape sequence parsing.

use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use crate::error::Result;
use crate::models::OutputLine;
use crate::models::output_line::AnsiCode;
use crate::terminal::ansi_parser::{AnsiParser, ParsedText};
use crate::pty::{PtyManager, PtyHandle};

/// Output processor for terminal streams
#[derive(Debug)]
pub struct OutputProcessor {
    /// ANSI parser for escape sequences
    ansi_parser: AnsiParser,
    /// Buffer for incoming raw output
    raw_buffer: Vec<u8>,
    /// Processed output lines
    processed_lines: VecDeque<OutputLine>,
    /// Current line being built
    current_line: String,
    /// Current ANSI codes for the line
    current_ansi_codes: Vec<AnsiCode>,
    /// Line number counter
    line_counter: usize,
    /// Whether we're in the middle of processing ANSI sequences
    in_ansi_sequence: bool,
    /// Maximum buffer size
    max_buffer_size: usize,
}

#[derive(Debug, Clone)]
pub struct OutputChunk {
    /// Raw output data
    pub data: Vec<u8>,
    /// Timestamp when received
    pub timestamp: DateTime<Utc>,
    /// Stream type (stdout/stderr)
    pub stream_type: StreamType,
    /// Whether this is the end of a command
    pub is_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamType {
    /// Standard output
    Stdout,
    /// Standard error
    Stderr,
}

impl OutputProcessor {
    /// Create a new output processor
    pub fn new() -> Self {
        Self {
            ansi_parser: AnsiParser::new(),
            raw_buffer: Vec::new(),
            processed_lines: VecDeque::new(),
            current_line: String::new(),
            current_ansi_codes: Vec::new(),
            line_counter: 0,
            in_ansi_sequence: false,
            max_buffer_size: 100 * 1024 * 1024, // 100MB for unlimited output
        }
    }
}

impl Default for OutputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputProcessor {
    /// Create with custom buffer size
    pub fn with_buffer_size(max_buffer_size: usize) -> Self {
        Self {
            max_buffer_size,
            ..Self::new()
        }
    }

    /// Drain and return only the fully processed lines accumulated so far,
    /// without flushing the partially built current line.
    pub fn take_ready_lines(&mut self) -> Vec<OutputLine> {
        let mut result = Vec::new();
        while let Some(line) = self.processed_lines.pop_front() {
            result.push(line);
        }
        result
    }

    /// Process incoming output chunk
    pub fn process_chunk(&mut self, chunk: OutputChunk) -> Result<Vec<OutputLine>> {
        // Add to raw buffer
        self.raw_buffer.extend(&chunk.data);

        // Limit buffer size
        if self.raw_buffer.len() > self.max_buffer_size {
            let excess = self.raw_buffer.len() - self.max_buffer_size;
            self.raw_buffer.drain(0..excess);
        }

        // Process the data
        self.process_data(&chunk.data, chunk.timestamp, chunk.stream_type)?;

        // Return processed lines if this chunk completes a command
        if chunk.is_complete {
            Ok(self.flush_lines())
        } else {
            Ok(Vec::new())
        }
    }

    /// Process raw data bytes
    fn process_data(&mut self, data: &[u8], timestamp: DateTime<Utc>, stream_type: StreamType) -> Result<()> {
        // Convert bytes to string, handling encoding errors
        let text = String::from_utf8_lossy(data);

        for ch in text.chars() {
            match ch {
                '\n' => self.process_newline(timestamp, stream_type)?,
                '\r' => self.process_carriage_return()?,
                '\x1b' => {
                    self.in_ansi_sequence = true;
                    self.current_line.push(ch);
                }
                ch if self.in_ansi_sequence => {
                    self.current_line.push(ch);
                    if self.is_ansi_sequence_complete(&self.current_line) {
                        self.process_ansi_sequence()?;
                    }
                }
                ch => {
                    self.current_line.push(ch);
                }
            }
        }

        Ok(())
    }

    /// Process newline character
    fn process_newline(&mut self, timestamp: DateTime<Utc>, _stream_type: StreamType) -> Result<()> {
        if !self.current_line.is_empty() {
            // Parse ANSI codes if present
            let parsed = self.ansi_parser.parse(&self.current_line)?;

            let output_line = OutputLine {
                text: parsed.clean_text,
                ansi_codes: parsed.ansi_codes,
                line_number: self.line_counter,
                timestamp,
            };

            self.processed_lines.push_back(output_line);
            self.line_counter += 1;
        }

        // Start new line
        self.current_line.clear();
        self.current_ansi_codes.clear();
        self.in_ansi_sequence = false;

        Ok(())
    }

    /// Process carriage return
    fn process_carriage_return(&mut self) -> Result<()> {
        // For now, treat CR as line clear (could be enhanced)
        self.current_line.clear();
        self.current_ansi_codes.clear();
        self.in_ansi_sequence = false;
        Ok(())
    }

    /// Check if ANSI sequence is complete
    fn is_ansi_sequence_complete(&self, sequence: &str) -> bool {
        if !sequence.starts_with('\x1b') {
            return false;
        }

        // Check for various ANSI sequence terminators
        sequence.chars().last().is_some_and(|c| {
            matches!(c, 'm' | 'G' | 'H' | 'J' | 'K' | 'A' | 'B' | 'C' | 'D')
        })
    }

    /// Process complete ANSI sequence
    fn process_ansi_sequence(&mut self) -> Result<()> {
        if let Some(code) = self.ansi_parser.parse(&self.current_line)?.ansi_codes.first() {
            self.current_ansi_codes.push(code.clone());
        }

        // Remove the ANSI sequence from current line
        if let Some(end_pos) = self.find_ansi_sequence_end(&self.current_line) {
            self.current_line = self.current_line[end_pos..].to_string();
        }

        self.in_ansi_sequence = false;
        Ok(())
    }

    /// Find the end position of ANSI sequence
    fn find_ansi_sequence_end(&self, text: &str) -> Option<usize> {
        let bytes = text.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'\x1b' {
                // Found escape sequence start
                i += 1;
                if i < bytes.len() && bytes[i] == b'[' {
                    // CSI sequence
                    i += 1;
                    while i < bytes.len() {
                        let ch = bytes[i];
                        if ch.is_ascii_uppercase() || ch.is_ascii_lowercase() {
                            return Some(i + 1);
                        }
                        i += 1;
                    }
                }
            } else {
                i += 1;
            }
        }

        None
    }

    /// Flush all pending lines
    pub fn flush_lines(&mut self) -> Vec<OutputLine> {
        let mut result = Vec::new();

        // Add any remaining content as a line
        if !self.current_line.is_empty() {
            let parsed = self.ansi_parser.parse(&self.current_line).unwrap_or_else(|_| {
                // Fallback if parsing fails
                ParsedText {
                    original_text: self.current_line.clone(),
                    clean_text: self.current_line.clone(),
                    ansi_codes: Vec::new(),
                    position_map: Vec::new(),
                }
            });

            let output_line = OutputLine {
                text: parsed.clean_text,
                ansi_codes: parsed.ansi_codes,
                line_number: self.line_counter,
                timestamp: Utc::now(),
            };

            result.push(output_line);
            self.line_counter += 1;
            self.current_line.clear();
            self.current_ansi_codes.clear();
        }

        // Move all processed lines to result
        while let Some(line) = self.processed_lines.pop_front() {
            result.push(line);
        }

        result
    }

    /// Check if there are pending lines
    pub fn has_pending_lines(&self) -> bool {
        !self.processed_lines.is_empty() || !self.current_line.is_empty()
    }

    /// Get the number of processed lines
    pub fn processed_line_count(&self) -> usize {
        self.processed_lines.len()
    }

    /// Clear all buffers
    pub fn clear(&mut self) {
        self.raw_buffer.clear();
        self.processed_lines.clear();
        self.current_line.clear();
        self.current_ansi_codes.clear();
        self.line_counter = 0;
        self.in_ansi_sequence = false;
    }

    /// Read output from PTY and process it
    pub async fn read_and_process_output(
        &mut self,
        pty_manager: &mut PtyManager,
        handle: &PtyHandle,
        timeout_ms: u64,
    ) -> Result<Vec<OutputLine>> {
        // Read raw output from PTY
        let raw_output = pty_manager.read_output(handle, timeout_ms).await?;

        if raw_output.is_empty() {
            return Ok(Vec::new());
        }

        // Create output chunk
        let chunk = OutputChunk {
            data: raw_output,
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false, // We'll determine this based on content
        };

        // Process the chunk
        self.process_chunk(chunk)
    }

    /// Continuously read and process output until timeout
    pub async fn read_output_until_timeout(
        &mut self,
        pty_manager: &mut PtyManager,
        handle: &PtyHandle,
        max_timeout_ms: u64,
    ) -> Result<Vec<OutputLine>> {
        let mut all_lines = Vec::new();
        let mut total_time = 0u64;
        let chunk_timeout = 10u64; // Small timeout for each chunk

        // Keep reading until we get no more data or hit max timeout
        while total_time < max_timeout_ms {
            match self.read_and_process_output(pty_manager, handle, chunk_timeout).await {
                Ok(lines) if lines.is_empty() => {
                    // No more data available
                    break;
                }
                Ok(lines) => {
                    all_lines.extend(lines);
                }
                Err(e) => {
                    return Err(e);
                }
            }

            total_time += chunk_timeout;
        }

        Ok(all_lines)
    }

    /// Get buffer statistics
    pub fn buffer_stats(&self) -> BufferStats {
        BufferStats {
            raw_buffer_size: self.raw_buffer.len(),
            processed_lines: self.processed_lines.len(),
            current_line_length: self.current_line.len(),
            ansi_codes_count: self.current_ansi_codes.len(),
        }
    }
}

/// Buffer statistics
#[derive(Debug, Clone)]
pub struct BufferStats {
    /// Size of raw buffer in bytes
    pub raw_buffer_size: usize,
    /// Number of processed lines
    pub processed_lines: usize,
    /// Length of current line being built
    pub current_line_length: usize,
    /// Number of ANSI codes in current line
    pub ansi_codes_count: usize,
}

/// Output segmentation utilities
pub mod segmentation {
    use super::*;
    use regex::Regex;

    /// Segment output into logical blocks
    pub fn segment_output(lines: &[OutputLine]) -> Vec<OutputSegment> {
        let mut segments = Vec::new();
        let mut current_segment = Vec::new();

        for line in lines {
            if is_command_prompt(line) {
                // End previous segment if it exists
                if !current_segment.is_empty() {
                    segments.push(OutputSegment {
                        lines: current_segment,
                        segment_type: SegmentType::CommandOutput,
                    });
                    current_segment = Vec::new();
                }
            } else {
                current_segment.push(line.clone());
            }
        }

        // Add final segment
        if !current_segment.is_empty() {
            segments.push(OutputSegment {
                lines: current_segment,
                segment_type: SegmentType::CommandOutput,
            });
        }

        segments
    }

    /// Check if line contains a command prompt
    fn is_command_prompt(line: &OutputLine) -> bool {
        let prompt_patterns = [
            r"^\$",
            r"^#",
            r"^>",
            r"bash-\d+\.\d+\$",
            r"zsh-\d+\.\d+%",
        ];

        for pattern in &prompt_patterns {
            if Regex::new(pattern).unwrap().is_match(&line.text) {
                return true;
            }
        }

        false
    }
}

/// Output segment
#[derive(Debug, Clone)]
pub struct OutputSegment {
    /// Lines in this segment
    pub lines: Vec<OutputLine>,
    /// Type of segment
    pub segment_type: SegmentType,
}

/// Segment type
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentType {
    /// Output from a command
    CommandOutput,
    /// Error output
    ErrorOutput,
    /// System messages
    SystemMessage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_processor_creation() {
        let processor = OutputProcessor::new();
        assert!(!processor.has_pending_lines());
        assert_eq!(processor.processed_line_count(), 0);
    }

    #[test]
    fn test_process_simple_text() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"Hello, World!\n".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello, World!");
    }

    #[test]
    fn test_process_multiple_lines() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"Line 1\nLine 2\nLine 3\n".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "Line 1");
        assert_eq!(lines[1].text, "Line 2");
        assert_eq!(lines[2].text, "Line 3");
    }

    #[test]
    fn test_process_ansi_colors() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"\x1b[31mRed text\x1b[0m\n".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Red text");
        assert!(!lines[0].ansi_codes.is_empty());
    }

    #[test]
    fn test_buffer_stats() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"Test data".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };

        processor.process_chunk(chunk).unwrap();
        let stats = processor.buffer_stats();

        assert_eq!(stats.raw_buffer_size, 9); // "Test data"
        assert_eq!(stats.current_line_length, 9);
    }

    #[test]
    fn test_flush_lines() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"Hello".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };

        processor.process_chunk(chunk).unwrap();
        assert!(processor.has_pending_lines());

        let lines = processor.flush_lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
        assert!(!processor.has_pending_lines());
    }

    #[test]
    fn test_clear_processor() {
        let mut processor = OutputProcessor::new();

        let chunk = OutputChunk {
            data: b"Test".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };

        processor.process_chunk(chunk).unwrap();
        assert!(processor.has_pending_lines());

        processor.clear();
        assert!(!processor.has_pending_lines());
        assert_eq!(processor.processed_line_count(), 0);
    }

    #[test]
    fn test_segmentation() {
        use crate::models::OutputLine;

        let lines = vec![
            OutputLine {
                text: "$ ls".to_string(),
                ansi_codes: Vec::new(),
                line_number: 0,
                timestamp: Utc::now(),
            },
            OutputLine {
                text: "file1.txt".to_string(),
                ansi_codes: Vec::new(),
                line_number: 1,
                timestamp: Utc::now(),
            },
            OutputLine {
                text: "file2.txt".to_string(),
                ansi_codes: Vec::new(),
                line_number: 2,
                timestamp: Utc::now(),
            },
        ];

        let segments = segmentation::segment_output(&lines);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].lines.len(), 2); // ls command output
    }

    #[test]
    fn test_stream_types() {
        assert_eq!(StreamType::Stdout, StreamType::Stdout);
        assert_eq!(StreamType::Stderr, StreamType::Stderr);
        assert_ne!(StreamType::Stdout, StreamType::Stderr);
    }

    #[test]
    fn test_segment_types() {
        assert_eq!(SegmentType::CommandOutput, SegmentType::CommandOutput);
        assert_eq!(SegmentType::ErrorOutput, SegmentType::ErrorOutput);
        assert_eq!(SegmentType::SystemMessage, SegmentType::SystemMessage);
    }
}
