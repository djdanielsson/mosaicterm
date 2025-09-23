# Contract: Command Execution and Output Streaming

**Purpose**: Define the interface for sending commands to PTY and receiving structured output.

## Interface Definition

### Command Input
**Function**: `send_command(handle: &PtyHandle, command: &str) -> Result<(), CommandError>`

**Preconditions**:
- PtyHandle must be valid and process alive
- Command string must not be empty
- PTY must be in prompt-ready state

**Postconditions**:
- Command sent to PTY stdin with newline
- Returns immediately (non-blocking)
- Command execution begins in PTY

**Error Conditions**:
- PTY not ready → `CommandError::PtyNotReady`
- Write failure → `CommandError::WriteFailed`
- Process terminated → `CommandError::ProcessTerminated`

---

### Output Streaming
**Function**: `read_output(handle: &PtyHandle) -> Result<OutputChunk, ReadError>`

**Returns**:
```
OutputChunk {
    data: Vec<u8>,                // Raw output bytes
    timestamp: DateTime<Utc>,     // When received
    stream_type: StreamType,      // Stdout or Stderr
    is_complete: bool             // End of current command
}
```

**Preconditions**:
- PtyHandle must be valid
- Process may be running or terminated

**Postconditions**:
- Returns available output data
- Non-blocking, returns immediately if no data
- Data removed from internal buffer

**Error Conditions**:
- Process terminated → `ReadError::ProcessTerminated`
- Buffer overflow → `ReadError::BufferOverflow`

---

### Command Completion Detection
**Function**: `is_command_complete(output: &str, prompt_pattern: &Regex) -> bool`

**Preconditions**:
- Output string from recent read operations
- Prompt pattern configured for current shell

**Postconditions**:
- Returns true if shell prompt detected in output
- Returns false if command still executing
- Pure function, no side effects

---

### Output Processing
**Function**: `process_output_chunk(chunk: OutputChunk) -> ProcessedOutput`

**Returns**:
```
ProcessedOutput {
    lines: Vec<OutputLine>,        // Parsed lines with ANSI
    ansi_codes: Vec<AnsiCode>,    // Extracted formatting
    command_complete: bool,       // Whether command finished
    exit_status: Option<i32>      // Exit code if available
}
```

**Preconditions**:
- Chunk data must be valid UTF-8 or handle encoding errors

**Postconditions**:
- ANSI escape sequences parsed and separated
- Text split into logical lines
- Command completion status determined

---

## Quality Assurance

### Contract Tests Required
- [ ] Test command sending to running PTY
- [ ] Test command sending to terminated PTY
- [ ] Test output reading with data available
- [ ] Test output reading with no data (non-blocking)
- [ ] Test command completion detection with various prompts
- [ ] Test output processing with ANSI codes
- [ ] Test output processing with multi-byte characters
- [ ] Test error handling for malformed input

### Performance Requirements
- Command send <10ms latency
- Output read <5ms when data available
- Output processing <50ms for 1KB of data
- Memory usage scales linearly with output size

### Reliability Requirements
- No data loss during high-throughput scenarios
- Proper handling of binary data in output
- Graceful degradation for encoding errors
- Thread-safe concurrent read/write operations
