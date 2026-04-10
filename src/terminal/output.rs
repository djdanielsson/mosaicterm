//! Output Processing and Segmentation
//!
//! Processes terminal output, segments it into logical chunks,
//! and handles ANSI escape sequence parsing.

use crate::error::Result;
use crate::models::output_line::AnsiCode;
use crate::models::OutputLine;
use crate::terminal::ansi_parser::{AnsiParser, ParsedText};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;

/// Tracks what kind of escape sequence we are accumulating.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EscapeState {
    /// Not inside any escape sequence.
    None,
    /// Saw ESC, waiting for the next byte to determine sequence type.
    Escape,
    /// Inside a CSI sequence: ESC [ ... (terminated by an ASCII letter).
    Csi,
    /// Inside an OSC sequence: ESC ] ... (terminated by BEL or ST).
    Osc,
    /// Saw ESC inside an OSC sequence (possible ST = ESC \).
    OscEscapeSeen,
}

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
    /// State machine for escape sequence parsing
    escape_state: EscapeState,
    /// Buffer for the escape sequence currently being accumulated (so it can
    /// be discarded or handed to the ANSI parser once complete).
    escape_buf: String,
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
            escape_state: EscapeState::None,
            escape_buf: String::new(),
            max_buffer_size: 10 * 1024 * 1024, // 10MB
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
        let mut p = Self::new();
        p.max_buffer_size = max_buffer_size;
        p
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
            // Return ready lines (completed lines with newlines) but keep partial line buffered
            Ok(self.take_ready_lines())
        }
    }

    /// Process raw data bytes using a state machine that handles CSI, OSC,
    /// and other escape sequence families.
    fn process_data(
        &mut self,
        data: &[u8],
        timestamp: DateTime<Utc>,
        stream_type: StreamType,
    ) -> Result<()> {
        let text = String::from_utf8_lossy(data);

        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            let ch = chars[i];

            match self.escape_state {
                // ----- Normal text -----
                EscapeState::None => match ch {
                    '\x1b' => {
                        self.escape_state = EscapeState::Escape;
                        self.escape_buf.clear();
                        self.escape_buf.push(ch);
                    }
                    '\n' => self.emit_line(timestamp, stream_type)?,
                    '\r' => {
                        let next_is_newline = i + 1 < len && chars[i + 1] == '\n';
                        if !next_is_newline {
                            self.current_line.clear();
                            self.current_ansi_codes.clear();
                        }
                    }
                    '\x07' => {} // BEL outside of escape — ignore
                    _ => self.current_line.push(ch),
                },

                // ----- Saw ESC, decide which family -----
                EscapeState::Escape => {
                    self.escape_buf.push(ch);
                    match ch {
                        '[' => self.escape_state = EscapeState::Csi,
                        ']' => self.escape_state = EscapeState::Osc,
                        // Two-character sequences: ESC ( ESC ) ESC = ESC > ESC M etc.
                        '(' | ')' | '*' | '+' | '=' | '>' | 'M' | '7' | '8' | 'c' | 'D' | 'E'
                        | 'H' | 'N' | 'O' => {
                            // Discard the sequence
                            self.escape_state = EscapeState::None;
                            self.escape_buf.clear();
                        }
                        _ => {
                            // Unknown single-char escape — discard
                            self.escape_state = EscapeState::None;
                            self.escape_buf.clear();
                        }
                    }
                }

                // ----- CSI sequence: ESC [ params letter -----
                EscapeState::Csi => {
                    self.escape_buf.push(ch);
                    if ch.is_ascii_alphabetic() || ch == '@' || ch == '`' {
                        let seq = std::mem::take(&mut self.escape_buf);
                        let text_pos = self.current_line.len();
                        if let Ok(parsed) = self.ansi_parser.parse(&seq) {
                            for code in &parsed.ansi_codes {
                                let mut c = code.clone();
                                c.position = text_pos;
                                self.current_ansi_codes.push(c);
                            }
                        }
                        self.escape_state = EscapeState::None;
                    }
                }

                // ----- OSC sequence: ESC ] ... BEL  or  ESC ] ... ESC \ -----
                EscapeState::Osc => match ch {
                    '\x07' => {
                        // BEL terminates OSC — discard entire sequence
                        self.escape_buf.clear();
                        self.escape_state = EscapeState::None;
                    }
                    '\x1b' => {
                        // Might be the start of ST (ESC \)
                        self.escape_state = EscapeState::OscEscapeSeen;
                    }
                    '\n' => {
                        // Newline inside an OSC is unusual; abort the sequence
                        // and emit the line (the OSC content is discarded).
                        self.escape_buf.clear();
                        self.escape_state = EscapeState::None;
                        self.emit_line(timestamp, stream_type)?;
                    }
                    _ => {
                        self.escape_buf.push(ch);
                        // Safety valve: if the OSC sequence is absurdly long,
                        // discard it to avoid unbounded memory growth.
                        if self.escape_buf.len() > 4096 {
                            self.escape_buf.clear();
                            self.escape_state = EscapeState::None;
                        }
                    }
                },

                // ----- Inside OSC, saw ESC — check for backslash (ST) -----
                EscapeState::OscEscapeSeen => {
                    if ch == '\\' {
                        // ST received — discard the OSC sequence
                        self.escape_buf.clear();
                        self.escape_state = EscapeState::None;
                    } else {
                        // Was not ST; the ESC starts a new sequence
                        self.escape_buf.clear();
                        self.escape_state = EscapeState::Escape;
                        self.escape_buf.push('\x1b');
                        // Re-process this character in the Escape state
                        // (don't increment i)
                        continue;
                    }
                }
            }

            i += 1;
        }

        Ok(())
    }

    /// Finish the current line and push it into `processed_lines`.
    ///
    /// Any residual CSI sequences that were not fully consumed by the state
    /// machine (e.g. embedded in the text) are re-parsed here; the codes
    /// already tracked in `current_ansi_codes` are merged in.
    fn emit_line(&mut self, timestamp: DateTime<Utc>, _stream_type: StreamType) -> Result<()> {
        if !self.current_line.is_empty() || !self.current_ansi_codes.is_empty() {
            let parsed = self.ansi_parser.parse(&self.current_line)?;

            let mut codes = std::mem::take(&mut self.current_ansi_codes);
            codes.extend(parsed.ansi_codes);

            let text = if parsed.clean_text.is_empty() && self.current_line.is_empty() {
                String::new()
            } else {
                parsed.clean_text
            };

            if !text.is_empty() || !codes.is_empty() {
                let mut output_line = OutputLine::with_ansi_codes(text, codes, self.line_counter);
                output_line.timestamp = timestamp;
                self.processed_lines.push_back(output_line);
                self.line_counter += 1;
            }
        }

        self.current_line.clear();
        self.current_ansi_codes.clear();

        Ok(())
    }

    /// Flush all pending lines
    pub fn flush_lines(&mut self) -> Vec<OutputLine> {
        let mut result = Vec::new();

        // Discard any in-progress escape sequence
        self.escape_state = EscapeState::None;
        self.escape_buf.clear();

        // Add any remaining content as a line
        if !self.current_line.is_empty() {
            let parsed = self
                .ansi_parser
                .parse(&self.current_line)
                .unwrap_or_else(|_| ParsedText {
                    original_text: self.current_line.clone(),
                    clean_text: self.current_line.clone(),
                    ansi_codes: Vec::new(),
                    position_map: Vec::new(),
                });

            let output_line = OutputLine::with_ansi_codes(
                parsed.clean_text,
                parsed.ansi_codes,
                self.line_counter,
            );

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

    /// Return the partial line currently being accumulated (no newline yet).
    /// Useful for prompt detection when the shell writes a prompt without a
    /// trailing newline.  Returns the cleaned text with ANSI/OSC stripped.
    pub fn peek_partial_line(&self) -> Option<&str> {
        if self.current_line.is_empty() && self.current_ansi_codes.is_empty() {
            None
        } else {
            Some(&self.current_line)
        }
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
        self.escape_state = EscapeState::None;
        self.escape_buf.clear();
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
        use once_cell::sync::Lazy;
        static PROMPT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
            [r"^\$", r"^#", r"^>", r"bash-\d+\.\d+\$", r"zsh-\d+\.\d+%"]
                .iter()
                .map(|p| Regex::new(p).unwrap())
                .collect()
        });

        PROMPT_PATTERNS.iter().any(|re| re.is_match(&line.text))
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
            OutputLine::with_line_number("$ ls", 0),
            OutputLine::with_line_number("file1.txt", 1),
            OutputLine::with_line_number("file2.txt", 2),
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

    #[test]
    fn test_ansi_code_positions_in_clean_text() {
        let mut processor = OutputProcessor::new();

        // "\x1b[31m" at position 0 in clean text, then "Red" text,
        // then "\x1b[0m" at position 3, then " normal" text
        let chunk = OutputChunk {
            data: b"\x1b[31mRed\x1b[0m normal\n".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Red normal");
        assert!(
            lines[0].ansi_codes.len() >= 2,
            "expected at least 2 ANSI codes"
        );

        // First code (\x1b[31m) should be at position 0
        assert_eq!(lines[0].ansi_codes[0].position, 0);
        // Second code (\x1b[0m) should be at position 3 ("Red" has 3 chars)
        assert_eq!(lines[0].ansi_codes[1].position, 3);
    }

    #[test]
    fn test_osc_without_bel_uses_backslash_terminator() {
        let mut processor = OutputProcessor::new();

        // OSC terminated by ESC \ (ST) — common in zsh
        let data = b"\x1b]2;eza -l\x1b\\\x1b]1;eza\x1b\\actual output\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "actual output");
    }

    #[test]
    fn test_osc_split_across_chunks() {
        let mut processor = OutputProcessor::new();

        // First chunk ends mid-OSC sequence
        let chunk1 = OutputChunk {
            data: b"\x1b]2;eza".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };
        let lines1 = processor.process_chunk(chunk1).unwrap();
        assert!(lines1.is_empty(), "no output yet — still in OSC");

        // Second chunk finishes OSC and provides real content
        let chunk2 = OutputChunk {
            data: b" -l\x07real output\n".to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };
        let lines2 = processor.process_chunk(chunk2).unwrap();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0].text, "real output");
    }

    #[test]
    fn test_osc7_with_backslash_newline() {
        // zsh's OSC-7 ends with ESC \ followed by newline
        let mut processor = OutputProcessor::new();
        let data = b"\x1b]7;file://host/Users/user\x1b\\\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };
        let lines = processor.process_chunk(chunk).unwrap();
        // Should produce no visible output
        assert!(lines.is_empty() || lines.iter().all(|l| l.text.is_empty()));
    }

    #[test]
    fn test_exact_zsh_ohmyzsh_output_pattern() {
        // This is the EXACT byte sequence zsh + Oh My Zsh produces for `eza -l`:
        // 1. prompt echo with OSC title/icon/cwd sequences
        // 2. command output
        // 3. post-command prompt with OSC title/icon/cwd sequences
        let mut processor = OutputProcessor::new();

        // zsh echoes the command surrounded by OSC sequences
        let prompt_echo = b"\r\n\x1b]2;eza -l\x07\x1b]1;eza\x07";
        let chunk1 = OutputChunk {
            data: prompt_echo.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };
        let lines1 = processor.process_chunk(chunk1).unwrap();
        // Should be empty — all OSC sequences, no printable text after the \r\n
        for line in &lines1 {
            assert!(
                line.text.is_empty() || !line.text.contains("]2;"),
                "OSC leaked: '{}'",
                line.text
            );
        }

        // Actual command output
        let output = b".rw-r--r--  2.3k ddaniels  6 Apr 15:33 Current-IT-Root-CAs.pem\ndrwx------     - ddaniels 27 Mar 13:38 Desktop\n";
        let chunk2 = OutputChunk {
            data: output.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: false,
        };
        let lines2 = processor.process_chunk(chunk2).unwrap();
        assert_eq!(lines2.len(), 2);
        assert!(lines2[0].text.contains("Current-IT-Root-CAs.pem"));
        assert!(lines2[1].text.contains("Desktop"));

        // Post-command: zsh sends OSC-2 (title), OSC-1 (icon), OSC-7 (cwd)
        let post_prompt = b"\x1b]2;ddaniels@ddaniels-mac:~\x07\x1b]1;~\x07\x1b]7;file://ddaniels-mac/Users/ddaniels\x1b\\\r\n";
        let chunk3 = OutputChunk {
            data: post_prompt.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };
        let lines3 = processor.process_chunk(chunk3).unwrap();
        for line in &lines3 {
            assert!(
                !line.text.contains("]2;")
                    && !line.text.contains("]1;")
                    && !line.text.contains("]7;"),
                "OSC leaked in post-prompt: '{}'",
                line.text
            );
        }
    }

    #[test]
    fn test_osc_title_sequences_stripped() {
        let mut processor = OutputProcessor::new();

        // Simulate what zsh/Oh My Zsh sends: OSC-2 (title), OSC-1 (icon), OSC-7 (cwd)
        // followed by actual command output.
        let data = b"\x1b]2;user@host:~/proj\x07\x1b]1;proj\x07\x1b]7;file://host/Users/user/proj\x1b\\git status\nOn branch main\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "git status");
        assert_eq!(lines[1].text, "On branch main");
    }

    #[test]
    fn test_osc_sequence_with_bel_terminator() {
        let mut processor = OutputProcessor::new();

        let data = b"\x1b]0;Window Title\x07Hello\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
    }

    #[test]
    fn test_osc_sequence_with_st_terminator() {
        let mut processor = OutputProcessor::new();

        // ST = ESC backslash
        let data = b"\x1b]2;title\x1b\\output text\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "output text");
    }

    #[test]
    fn test_mixed_csi_and_osc_sequences() {
        let mut processor = OutputProcessor::new();

        let data = b"\x1b]2;title\x07\x1b[31mRed text\x1b[0m\n";
        let chunk = OutputChunk {
            data: data.to_vec(),
            timestamp: Utc::now(),
            stream_type: StreamType::Stdout,
            is_complete: true,
        };

        let lines = processor.process_chunk(chunk).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Red text");
        assert!(!lines[0].ansi_codes.is_empty());
    }
}
