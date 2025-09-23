# Data Model: MosaicTerm Core Entities

**Date**: September 17, 2025
**Purpose**: Define core data structures and relationships for MosaicTerm

## Entity Definitions

### 1. Command Block Entity
Represents a single executed command and its complete output.

**Fields**:
- `id: String` - Unique identifier for the block
- `command: String` - The command text that was executed
- `output: Vec<OutputLine>` - Lines of output with ANSI formatting
- `timestamp: DateTime<Utc>` - When the command was executed
- `status: ExecutionStatus` - Success, failure, or running state
- `working_directory: PathBuf` - Directory where command was executed
- `execution_time: Option<Duration>` - How long the command took

**Relationships**:
- Belongs to: Terminal Session (many-to-one)
- Contains: Output Lines (one-to-many)

**Validation Rules**:
- Command cannot be empty
- Timestamp must be valid
- Status must match execution result

**State Transitions**:
- `Pending` → `Running` (when PTY starts command)
- `Running` → `Completed` (when prompt detected)
- `Running` → `Failed` (when process exits with error)

---

### 2. Terminal Session Entity
Represents a running terminal session with its PTY process.

**Fields**:
- `id: String` - Session identifier
- `pty_process: PtyProcess` - Handle to the PTY process
- `shell_type: ShellType` - zsh, bash, etc.
- `working_directory: PathBuf` - Current working directory
- `environment: HashMap<String, String>` - Environment variables
- `start_time: DateTime<Utc>` - When session started
- `is_active: bool` - Whether session is still running

**Relationships**:
- Contains: Command Blocks (one-to-many)
- Manages: PTY Process (one-to-one)

**Validation Rules**:
- PTY process must be valid when active
- Working directory must exist
- Environment variables must be valid

**State Transitions**:
- `Initializing` → `Active` (when PTY ready)
- `Active` → `Terminated` (when PTY exits)

---

### 3. PTY Process Entity
Manages the pseudoterminal process lifecycle.

**Fields**:
- `handle: ProcessHandle` - OS process identifier
- `stdin_writer: AsyncWrite` - Input stream to process
- `stdout_reader: AsyncRead` - Output stream from process
- `stderr_reader: AsyncRead` - Error stream from process
- `pid: u32` - Process ID
- `is_alive: bool` - Whether process is still running

**Relationships**:
- Managed by: Terminal Session (one-to-one)
- Provides streams for: Command execution

**Validation Rules**:
- All streams must be valid when process is alive
- PID must be valid OS process identifier

**State Transitions**:
- `Created` → `Running` (when process starts)
- `Running` → `Terminated` (when process exits)

---

### 4. Output Line Entity
Represents a single line of terminal output with ANSI formatting.

**Fields**:
- `text: String` - The actual text content
- `ansi_codes: Vec<AnsiCode>` - ANSI escape sequences for formatting
- `line_number: usize` - Position in output
- `timestamp: DateTime<Utc>` - When line was received

**Relationships**:
- Belongs to: Command Block (many-to-one)

**Validation Rules**:
- Text can be empty (for lines with only formatting)
- ANSI codes must be valid escape sequences

---

### 5. Configuration Entity
Application configuration settings.

**Fields**:
- `theme: Theme` - UI color scheme
- `font_family: String` - Font family for terminal
- `font_size: u32` - Font size in points
- `scrollback_lines: usize` - Maximum lines to keep in history
- `shell_path: PathBuf` - Path to shell executable
- `shell_args: Vec<String>` - Arguments for shell startup

**Relationships**:
- Used by: Application (one-to-one)

**Validation Rules**:
- Font size must be between 8-72 points
- Shell path must exist and be executable
- Scrollback lines must be reasonable (>0, <100000)

## Data Flow

### Command Execution Flow
1. User types command in input field
2. Input sent to PTY via stdin_writer
3. PTY executes command in shell
4. Output received via stdout_reader/stderr_reader
5. Lines parsed and stored as OutputLine entities
6. When prompt detected, CommandBlock marked complete
7. Block added to history in UI

### Session Management Flow
1. Application starts → TerminalSession created
2. PTY process spawned with shell
3. Session becomes active when shell ready
4. Commands executed through session
5. Session terminated when app closes or shell exits

## Storage Strategy

### In-Memory Storage
- Command blocks: VecDeque with size limit
- Current session state: Single instance
- Configuration: Loaded at startup, cached

### Persistence (Future)
- Command history: SQLite database
- Configuration: TOML file in user directory
- Session state: Temporary, not persisted

## Validation Rules Summary

### Business Rules
- Each command produces exactly one block
- Blocks are immutable once completed
- Only one session active at a time
- Configuration changes require restart

### Data Integrity
- All timestamps must be valid and sequential
- Process handles must correspond to real OS processes
- ANSI codes must be well-formed escape sequences
- File paths must exist and be accessible

## Future Extensions

### Planned Additions
- **Themes**: Multiple color schemes
- **Profiles**: Different shell configurations
- **Bookmarks**: Saved commands and directories
- **Search**: Full-text search in command history

### Scalability Considerations
- Command block limit to prevent memory issues
- Lazy loading for large output blocks
- Background processing for long-running commands
- Compression for historical data
