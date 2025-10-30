# MosaicTerm Architecture

**Version:** 0.1.0  
**Last Updated:** October 30, 2025

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture Principles](#architecture-principles)
3. [Component Diagram](#component-diagram)
4. [Data Flow](#data-flow)
5. [Threading Model](#threading-model)
6. [PTY Lifecycle](#pty-lifecycle)
7. [UI Update Cycle](#ui-update-cycle)
8. [Module Structure](#module-structure)
9. [Key Subsystems](#key-subsystems)
10. [Design Patterns](#design-patterns)
11. [Performance Considerations](#performance-considerations)

---

## Overview

MosaicTerm is a modern Rust-based terminal emulator built with `egui` (for GUI) and `portable-pty` (for cross-platform PTY support). It provides a block-based command history interface similar to Warp, with a permanently pinned input prompt at the bottom.

### Key Technologies

- **GUI Framework:** `egui` + `eframe` (immediate mode GUI)
- **PTY Management:** `portable-pty` (cross-platform pseudoterminal)
- **Terminal Emulation:** `vte` (ANSI escape sequence parsing)
- **Async Runtime:** `tokio` (for async operations)
- **Logging:** `tracing` (structured logging)

### Design Goals

1. **Simplicity:** Clean, understandable codebase
2. **Performance:** Efficient rendering and output processing
3. **Reliability:** Graceful error handling, no panics
4. **Extensibility:** Modular design for future enhancements

---

## Architecture Principles

### 1. Separation of Concerns

- **Library (`src/lib.rs`):** Core functionality, models, configuration
- **Binary (`src/main.rs`, `src/app.rs`):** GUI application and user interaction
- **Modules:** Self-contained units with clear responsibilities

### 2. Single-Threaded UI with Background I/O

- **Main Thread:** `egui` UI rendering and event handling
- **Background Threads:** PTY I/O (reader/writer threads per PTY)
- **Communication:** Channels for async data transfer

### 3. Immutable Defaults with Mutable State

- Configuration is loaded once and rarely changes
- Application state is mutable but carefully controlled
- PTY processes have explicit lifecycle management

### 4. Fail-Safe Defaults

- Graceful degradation when config loading fails
- Fallback to minimal configuration
- Output size limits prevent memory exhaustion

---

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         MosaicTerm App                          │
│                      (src/main.rs + app.rs)                     │
└────────────┬────────────────────────────────────────────────────┘
             │
             │ Uses
             ├─────────────────────────────────┐
             │                                 │
             ▼                                 ▼
┌────────────────────────┐         ┌──────────────────────────┐
│   Configuration        │         │   Terminal Session       │
│   (src/config/)        │         │   (src/terminal/)        │
│                        │         │                          │
│ - Config               │         │ - Terminal               │
│ - RuntimeConfig        │         │ - TerminalSession        │
│ - ThemeManager         │         │ - CommandInputProcessor  │
│ - PromptFormatter      │         │ - OutputProcessor        │
└────────────────────────┘         │ - AnsiParser             │
                                   └──────┬───────────────────┘
                                          │
                                          │ Uses
                                          ▼
                         ┌────────────────────────────────────┐
                         │   PTY Management                   │
                         │   (src/pty/)                       │
                         │                                    │
                         │ - PtyManager                       │
                         │ - PtyProcess                       │
                         │ - PtyStreams                       │
                         │ - SignalHandler                    │
                         └────────────────────────────────────┘
                                          │
                                          │ Uses
                                          ▼
                         ┌────────────────────────────────────┐
                         │   portable-pty                     │
                         │   (External Crate)                 │
                         │                                    │
                         │ - Native PTY system                │
                         │ - Cross-platform abstraction       │
                         └────────────────────────────────────┘

Supporting Systems:

┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Completion      │  │  Models          │  │  UI Components   │
│  (src/completion)│  │  (src/models/)   │  │  (src/ui/)       │
│                  │  │                  │  │                  │
│ - Provider       │  │ - CommandBlock   │  │ - Blocks         │
│ - Cache          │  │ - OutputLine     │  │ - Input          │
└──────────────────┘  │ - PtyProcess     │  │ - Viewport       │
                      │ - Config         │  │ - Scroll         │
                      └──────────────────┘  └──────────────────┘
```

---

## Data Flow

### Command Execution Flow

```
User Input
    │
    │ 1. Keyboard event
    ▼
egui UI (app.rs)
    │
    │ 2. Process input
    ▼
CommandInputProcessor
    │
    │ 3. Validate & prepare
    ├─── Tab completion
    ├─── History expansion
    └─── Security validation
    │
    │ 4. Send to PTY
    ▼
PtyManager
    │
    │ 5. Write to PTY master
    ▼
PTY Process (shell)
    │
    │ 6. Execute command
    ▼
Output Data
    │
    │ 7. Read from PTY master (background thread)
    ▼
PtyStreams (async channel)
    │
    │ 8. Batch read in update loop
    ▼
Terminal::process_output()
    │
    │ 9. Parse ANSI codes
    ▼
OutputProcessor + AnsiParser
    │
    │ 10. Convert to OutputLines
    ▼
CommandBlock (in command_history)
    │
    │ 11. Render in UI
    ▼
egui UI Display
```

### Configuration Loading Flow

```
main()
    │
    ├─ Parse CLI args
    │
    ├─ Load config file
    │   ├─ ~/.config/mosaicterm/config.toml
    │   ├─ ~/.mosaicterm.toml
    │   └─ ./mosaicterm.toml
    │
    ├─ Create RuntimeConfig
    │   ├─ Load base Config
    │   ├─ Initialize ThemeManager
    │   └─ Create PromptFormatter
    │
    └─ Pass to MosaicTermApp::with_config()
```

### UI Update Cycle

```
eframe::run_native()
    │
    │ 60 FPS (or on-demand)
    │
    ▼
MosaicTermApp::update()
    │
    ├─ 1. Auto-refresh completion cache (every 5 min)
    │
    ├─ 2. Initialize terminal (first frame only)
    │
    ├─ 3. Handle async operations
    │   ├─ Read PTY output (batched)
    │   ├─ Process ANSI codes
    │   ├─ Add to CommandBlock
    │   └─ Check command completion
    │
    ├─ 4. Render UI panels
    │   ├─ Top panel (status bar)
    │   ├─ Central panel (command history)
    │   └─ Bottom panel (input prompt)
    │
    ├─ 5. Handle input events
    │   ├─ Key presses
    │   ├─ Tab completion
    │   └─ Command submission
    │
    └─ 6. Request repaint (conditional)
        ├─ Immediate: if command running or output pending
        └─ Delayed (100ms): for idle polling
```

---

## Threading Model

MosaicTerm uses a **hybrid threading model**:

### Main Thread (UI Thread)

- **Responsibility:** Run `egui` event loop and render UI
- **Framework:** `eframe::App::update()` callback
- **Frequency:** ~60 FPS or on-demand
- **Constraints:** Must complete quickly (<16ms) to maintain smooth UI

### PTY Reader Threads (per PTY)

- **Responsibility:** Read output from PTY master file descriptor
- **Type:** Blocking I/O in dedicated thread
- **Communication:** Send data via `tokio::sync::mpsc::unbounded_channel`
- **Lifecycle:** Lives as long as PTY process

```rust
// Simplified reader thread structure
thread::spawn(move || {
    let mut buf = [0u8; 4096];
    loop {
        match master_reader.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                let _ = tx_async_out.send(buf[..n].to_vec());
            }
            Err(_) => break,
        }
    }
});
```

### PTY Writer Threads (per PTY)

- **Responsibility:** Write user input to PTY master
- **Type:** Blocking I/O in dedicated thread
- **Communication:** Receive data via `std::sync::mpsc::channel`
- **Buffering:** 8KB write buffer

```rust
// Simplified writer thread structure
thread::spawn(move || {
    while let Ok(data) = rx_stdin.recv() {
        let _ = master_writer.write_all(&data);
        let _ = master_writer.flush();
    }
});
```

### Threading Diagram

```
┌─────────────────────────────────────────────────────────┐
│                     Main Thread                         │
│                                                         │
│  ┌─────────────────────────────────────────────────┐  │
│  │  egui UI Loop (60 FPS)                          │  │
│  │                                                 │  │
│  │  - Render UI                                    │  │
│  │  - Handle events                                │  │
│  │  - Read from PTY channels (non-blocking)        │  │
│  │  - Process output                               │  │
│  └─────────────────────────────────────────────────┘  │
│                         │                               │
│                         │ Channels                      │
│                         │                               │
└─────────────────────────┼───────────────────────────────┘
                          │
         ┌────────────────┴────────────────┐
         │                                  │
         ▼                                  ▼
┌──────────────────────┐         ┌──────────────────────┐
│  PTY Reader Thread   │         │  PTY Writer Thread   │
│                      │         │                      │
│  - Blocking read()   │         │  - Blocking write()  │
│  - Send to channel   │         │  - Recv from channel │
└──────────────────────┘         └──────────────────────┘
         │                                  │
         │ OS File Descriptors              │
         │                                  │
         └────────────────┬─────────────────┘
                          │
                          ▼
                 ┌─────────────────┐
                 │   PTY Master    │
                 │  (OS Resource)  │
                 └─────────────────┘
                          │
                          ▼
                 ┌─────────────────┐
                 │   PTY Slave     │
                 │   (Shell/CMD)   │
                 └─────────────────┘
```

### Synchronization

- **PtyManager:** Protected by `Arc<Mutex<PtyManager>>` for safe shared access
- **Retry Logic:** 3 attempts with 100μs sleep to handle lock contention
- **Channels:** Unbounded for PTY output (bounded would risk data loss)

---

## PTY Lifecycle

### 1. Creation

```rust
// In MosaicTermApp::initialize_terminal()
let mut handle = PtyHandle::new();  // Generate UUID
let (process, streams) = spawn_pty_process(
    shell_path,
    &[],
    &env_vars,
    Some(&working_dir)
)?;
handle.set_pid(process.pid);
pty_manager.register(handle, process, streams);
```

**Steps:**
1. Generate unique PTY handle (UUID)
2. Create PTY pair (master/slave) via `portable-pty`
3. Spawn shell process on PTY slave
4. Clone master file descriptors for I/O
5. Start reader and writer background threads
6. Register in `PtyManager`

### 2. Active State

```
PTY Master (managed by MosaicTerm)
    ↕ File Descriptors
PTY Slave (attached to shell process)
    ↕ STDIN/STDOUT/STDERR
Shell Process (bash/zsh/fish)
    ↕ Executes commands
Child Processes (commands)
```

**Operations:**
- **Input:** User types → app.rs → PtyManager → Writer thread → PTY master → Shell
- **Output:** Command output → PTY master → Reader thread → Channel → app.rs → UI

### 3. Command Execution

```rust
// When user presses Enter
let command = "ls -la\n";
pty_manager.send_input(&handle, command.as_bytes()).await?;

// Shell executes command
// Output flows back through PTY
```

### 4. Output Processing

```rust
// In handle_async_operations()
if let Ok(data) = pty_manager.try_read_output_now(handle) {
    terminal.process_output(&data, StreamType::Stdout).await?;
    let ready_lines = terminal.take_ready_output_lines();
    
    // Add to current CommandBlock
    if let Some(last_block) = self.command_history.last_mut() {
        last_block.add_output_lines(ready_lines);
    }
}
```

### 5. Termination

**Graceful:**
```rust
// User exits shell (e.g., types "exit")
// Shell process terminates
// Reader thread detects EOF
// PTY is marked as terminated
```

**Forced:**
```rust
// App shutdown or user closes window
pty_manager.terminate_pty(&handle).await?;
// Sends SIGTERM to shell
// Waits for graceful exit (future: with timeout)
// Falls back to SIGKILL if needed (future)
```

### State Diagram

```
┌───────────────┐
│  Uninitialized│
└───────┬───────┘
        │ create_pty()
        ▼
┌───────────────┐
│  Initializing │
└───────┬───────┘
        │ spawn_command()
        ▼
┌───────────────┐
│    Running    │◄──────┐
└───────┬───────┘       │
        │               │ send_input()
        │ Shell exits   │ read_output()
        ▼               │
┌───────────────┐       │
│  Terminated   │       │
└───────────────┘       │
                        │
┌───────────────┐       │
│     Error     │───────┘
└───────────────┘
   (retry or fail)
```

---

## UI Update Cycle

### Frame Timing

MosaicTerm uses **on-demand rendering** with intelligent repaint requests:

```rust
// In MosaicTermApp::update()
if has_running_command || has_pending_output || completion_popup_visible {
    ctx.request_repaint(); // Immediate (next frame)
} else {
    ctx.request_repaint_after(Duration::from_millis(100)); // Delayed polling
}
```

**Benefits:**
- Low CPU usage when idle (1% CPU)
- Responsive during command execution (60 FPS)
- Balanced power consumption

### Layout Structure

```
┌─────────────────────────────────────────────────────────┐
│  Top Panel (Status Bar)                                 │
│  - Current working directory                            │
│  - Shell type indicator                                 │
│  - Active process status                                │
└─────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  Central Panel (Command History)                        │
│  ┌─────────────────────────────────────────────────┐  │
│  │ CommandBlock 1                                  │  │
│  │ $ echo "hello"                                  │  │
│  │ hello                                           │  │
│  │ [✓ Completed in 50ms]                          │  │
│  └─────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────┐  │
│  │ CommandBlock 2                                  │  │
│  │ $ ls -la                                        │  │
│  │ [... output lines ...]                          │  │
│  │ [⏱ Running for 2.3s...]                        │  │
│  └─────────────────────────────────────────────────┘  │
│                                                         │
│  (Scrollable viewport)                                  │
└─────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────┐
│  Bottom Panel (Input Prompt) - ALWAYS VISIBLE          │
│  ~/project $                                            │
│  █                                                      │
│  (Tab completion popup appears above if triggered)      │
└─────────────────────────────────────────────────────────┘
```

### Rendering Pipeline

```
update() called (each frame)
    │
    ├─ 1. Update state
    │   ├─ Process PTY output
    │   ├─ Update command blocks
    │   └─ Check timeouts
    │
    ├─ 2. Layout pass
    │   ├─ Calculate available space
    │   ├─ Measure text
    │   └─ Determine scroll position
    │
    ├─ 3. Paint pass
    │   ├─ Draw top panel
    │   ├─ Draw command blocks (with clipping)
    │   ├─ Draw bottom input
    │   └─ Draw completion popup
    │
    └─ 4. Input handling
        ├─ Keyboard events
        ├─ Mouse events
        └─ Focus management
```

### Performance Optimizations

1. **Output Batching:** Process all lines at once, single UI update
2. **Viewport Culling:** Only render visible command blocks
3. **Size Limits:** Max 50K lines per command, 10K chars per line
4. **Lazy Parsing:** ANSI codes parsed on-demand during rendering
5. **Conditional Repaints:** Only repaint when necessary

---

## Module Structure

### Core Modules

```
src/
├── main.rs              # Entry point, CLI arg parsing
├── app.rs               # Main application state and UI
├── lib.rs               # Library exports
├── state.rs             # Application state enums
├── error.rs             # Error types and Result aliases
│
├── config/              # Configuration management
│   ├── mod.rs          # RuntimeConfig, Config structs
│   ├── loader.rs       # File loading logic
│   ├── theme.rs        # ThemeManager, color schemes
│   ├── prompt.rs       # PromptFormatter, custom prompts
│   └── shell.rs        # Shell detection and paths
│
├── terminal/            # Terminal emulation
│   ├── mod.rs          # Terminal struct, session management
│   ├── input.rs        # Input processing and validation
│   ├── output.rs       # Output processing and segmentation
│   ├── ansi_parser.rs  # ANSI escape code parser
│   ├── prompt.rs       # Prompt detection and formatting
│   └── state.rs        # Terminal state machine
│
├── pty/                 # PTY management
│   ├── mod.rs          # PTY module exports
│   ├── manager.rs      # PtyManager, lifecycle coordination
│   ├── process.rs      # PTY process spawning
│   ├── streams.rs      # PtyStreams, I/O abstraction
│   └── signals.rs      # Signal handling (SIGTERM, etc.)
│
├── models/              # Data models
│   ├── mod.rs          # Model exports
│   ├── command_block.rs # CommandBlock (command + output)
│   ├── output_line.rs  # OutputLine, ANSI codes
│   ├── pty_process.rs  # PtyProcess state
│   ├── shell_type.rs   # ShellType enum
│   ├── config.rs       # Config structs (models)
│   └── terminal_session.rs # TerminalSession state
│
├── ui/                  # UI components
│   ├── mod.rs          # UI exports
│   ├── blocks.rs       # CommandBlock rendering
│   ├── input.rs        # Input field component
│   ├── viewport.rs     # Scrollable viewport
│   ├── scroll.rs       # Scroll state management
│   ├── text.rs         # Text rendering utilities
│   └── completion_popup.rs # Tab completion UI
│
├── completion.rs        # Command completion
├── commands.rs          # Command parsing (deprecated)
└── execution/           # Direct command execution
    └── mod.rs          # DirectExecutor (for tests)
```

### Module Responsibilities

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `config` | Load, parse, manage configuration | `Config`, `RuntimeConfig`, `ThemeManager` |
| `terminal` | Emulate terminal, process I/O | `Terminal`, `OutputProcessor`, `AnsiParser` |
| `pty` | Manage PTY lifecycle, I/O streams | `PtyManager`, `PtyHandle`, `PtyStreams` |
| `models` | Data structures and state | `CommandBlock`, `OutputLine`, `PtyProcess` |
| `ui` | Render UI components | `BlockRenderer`, `InputField`, `Viewport` |
| `completion` | Tab completion logic | `CompletionProvider`, `CompletionResult` |
| `execution` | Direct command execution (tests) | `DirectExecutor` |

---

## Key Subsystems

### 1. Configuration System

**Purpose:** Load, validate, and provide access to user and runtime configuration.

**Components:**
- `ConfigLoader`: Searches standard paths for config files
- `Config`: Base configuration struct (serializable)
- `RuntimeConfig`: Runtime wrapper with theme and prompt managers
- `ThemeManager`: Manages color schemes and theme switching
- `PromptFormatter`: Generates custom shell prompts

**Features:**
- TOML-based configuration files
- Multiple search paths (`~/.config/mosaicterm/`, `~/.mosaicterm.toml`)
- Fallback to defaults if loading fails
- Hot-reload support (future)

### 2. Terminal Emulation

**Purpose:** Process terminal I/O, parse ANSI codes, manage terminal state.

**Components:**
- `Terminal`: High-level terminal session management
- `TerminalSession`: Tracks shell state, working directory, history
- `OutputProcessor`: Buffers and segments raw output into lines
- `AnsiParser`: Parses ANSI escape sequences (colors, cursor movement)
- `CommandInputProcessor`: Validates, prepares, and enhances user input

**Capabilities:**
- ANSI color support (16 colors, 256 colors, RGB)
- Bold, italic, underline formatting
- Cursor movement and positioning
- Command history with configurable size (default: 1000)
- Tilde expansion (`~` → `/home/user`)
- History expansion (`!!` → last command)
- Multi-line command detection (backslash continuation)

### 3. PTY Management

**Purpose:** Create, manage, and communicate with pseudoterminal processes.

**Components:**
- `PtyManager`: Central coordinator for all PTY operations
- `PtyHandle`: Unique identifier for PTY instances
- `PtyProcess`: Tracks process state (running, completed, failed)
- `PtyStreams`: Abstracts I/O operations over channels
- `SignalHandler`: Handles process signals (SIGTERM, SIGKILL)

**Lifecycle:**
1. **Create:** `PtyManager::create_pty()` → spawn shell in PTY
2. **Communicate:** `send_input()` / `read_output()` via channels
3. **Monitor:** Check `is_alive()`, get process info
4. **Terminate:** `terminate_pty()` → graceful or forced shutdown

### 4. Command History

**Purpose:** Store, display, and manage block-based command history.

**Components:**
- `CommandBlock`: Represents a single command execution
  - Command string
  - Output lines (with ANSI codes)
  - Status (running, success, failed)
  - Timing information
  - Exit code

**Features:**
- Per-command output limits (50K lines, 10K chars/line)
- Truncation notices when limits exceeded
- Execution timing (start → completion)
- Status indicators (✓ success, ✗ failed, ⏱ running)
- Timeout detection (configurable: 30s regular, 5min interactive)

### 5. Completion System

**Purpose:** Provide intelligent tab completion for commands, files, and arguments.

**Components:**
- `CompletionProvider`: Central completion logic
- Command cache (scans `$PATH`, refreshes every 5 minutes)
- File/directory completion (current working directory)
- Common command suggestions

**Completion Types:**
- **Command:** Matches against cached executables
- **Path:** Completes file and directory names
- **Argument:** Context-aware argument suggestions (future)

### 6. UI Rendering

**Purpose:** Render terminal UI using `egui` immediate mode GUI.

**Components:**
- `BlockRenderer`: Renders command blocks with ANSI formatting
- `InputField`: Custom input widget with cursor and selection
- `Viewport`: Manages scrollable command history
- `CompletionPopup`: Displays tab completion suggestions

**Layout:**
- **Top Panel:** Status bar (working directory, shell info)
- **Central Panel:** Scrollable command history
- **Bottom Panel:** Pinned input prompt (always visible)

---

## Design Patterns

### 1. Builder Pattern

Used for complex configuration and process creation:

```rust
impl PtyManager {
    pub async fn create_pty(
        &mut self,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        working_directory: Option<&Path>,
    ) -> Result<PtyHandle> {
        // ...
    }
}
```

### 2. State Machine Pattern

PTY processes and terminal state follow explicit state machines:

```rust
pub enum ExecutionStatus {
    Pending,      // Not yet started
    Running,      // Currently executing
    Success,      // Completed successfully
    Failed,       // Failed with error
    Timedout,     // Exceeded timeout
}
```

### 3. Strategy Pattern

Different shell types have different command processing:

```rust
impl CommandProcessor {
    fn process_shell_specific(&self, command: &str) -> Result<String> {
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh => self.process_bash_zsh_command(command),
            ShellType::Fish => self.process_fish_command(command),
            _ => Ok(command.to_string()),
        }
    }
}
```

### 4. Observer Pattern

Event-driven updates via channels (PTY I/O):

```rust
// Reader thread observes PTY output
let (tx_output, rx_output) = unbounded_channel();
thread::spawn(move || {
    while let Ok(data) = read_pty() {
        tx_output.send(data);
    }
});

// UI thread observes channel
if let Ok(data) = rx_output.try_recv() {
    process_output(data);
}
```

### 5. Command Pattern

User actions encapsulated as operations:

```rust
pub enum InputResult {
    NoOp,                    // No action needed
    CommandReady(String),    // Execute command
    CompletionRequested,     // Show completions
    HistoryNavigation(isize),// Navigate history
}
```

---

## Performance Considerations

### Memory Management

1. **Output Limits:**
   - Max 50,000 lines per command block
   - Max 10,000 characters per line
   - Truncation with clear user feedback

2. **Buffer Sizes:**
   - PTY config buffer: 256KB (down from 1MB)
   - Terminal output max: 10MB (down from 100MB)
   - PTY read buffer: 128KB (down from 1MB)
   - PTY write buffer: 8KB (up from 4KB)

3. **History Management:**
   - Configurable max history size (default: 1000 commands)
   - Old commands automatically pruned
   - Per-session history (not persistent yet)

### CPU Optimization

1. **Conditional Repaints:**
   - Immediate repaint only when output pending or command running
   - 100ms delayed repaint for idle polling
   - Reduces CPU from 100% to 1% when idle

2. **Output Batching:**
   - All output lines processed in batch
   - Single UI update instead of per-line updates
   - Pre-allocated vectors for known sizes

3. **Lazy Operations:**
   - ANSI parsing only when rendering
   - Completion cache refreshed every 5 minutes (not every frame)
   - Viewport culling (only render visible blocks)

### I/O Efficiency

1. **Buffered I/O:**
   - 128KB read buffer reduces syscall overhead
   - 8KB write buffer optimizes small writes
   - Flushing only after complete writes

2. **Non-Blocking Reads:**
   - `try_read()` instead of blocking read in UI thread
   - Retry logic (3 attempts) for lock contention
   - Background threads handle blocking I/O

3. **Channel Capacity:**
   - Unbounded channels for PTY output (prevents data loss)
   - Bounded channels for control messages (backpressure)

---

## Error Handling Strategy

### Principle: No Panics in Production

- All fallible operations return `Result<T, Error>`
- `panic!` only in tests or truly unrecoverable situations
- Graceful degradation when possible

### Error Types

```rust
pub enum Error {
    Io(std::io::Error),           // I/O errors
    Config(String),                // Configuration errors
    Pty(String),                   // PTY-related errors
    Terminal(String),              // Terminal emulation errors
    Parse(String),                 // Parsing errors
    Other(String),                 // Generic errors
}
```

### Error Handling Patterns

1. **Fallback to Defaults:**
   ```rust
   let config = Config::load().unwrap_or_default();
   ```

2. **Log and Continue:**
   ```rust
   if let Err(e) = self.completion_provider.refresh_cache() {
       debug!("Failed to refresh cache: {}", e);
       // Continue without crashing
   }
   ```

3. **User Feedback:**
   ```rust
   if let Err(e) = pty_manager.create_pty(...) {
       self.state.status_message = Some(format!("Failed to start: {}", e));
   }
   ```

---

## Future Architecture Considerations

### Planned Improvements

1. **Async Refactor (TASK-007):**
   - Move from blocking I/O threads to full async/await
   - Use `tokio::io::AsyncRead/AsyncWrite` for PTY
   - Eliminate lock contention in PtyManager

2. **Multi-Terminal Support:**
   - Multiple PTY sessions in tabs
   - Switch between sessions
   - Independent state per terminal

3. **Persistent History:**
   - Save command history to disk
   - History search and replay
   - Cross-session history

4. **Plugin System:**
   - External plugins for custom commands
   - Themeable UI components
   - Extensible completion providers

### Scalability

Current architecture supports:
- **Single terminal:** ✅ Optimized
- **Multiple terminals:** 🚧 Requires refactoring
- **Large output (GB):** ✅ Truncation limits prevent OOM
- **Long-running commands:** ✅ Timeout detection implemented
- **High command throughput:** ✅ Batching improves performance

---

## Conclusion

MosaicTerm's architecture prioritizes:
- **Simplicity:** Easy to understand and modify
- **Reliability:** Graceful error handling, no crashes
- **Performance:** Efficient rendering and I/O
- **Extensibility:** Modular design for future features

The combination of `egui` for UI, `portable-pty` for PTY management, and a hybrid threading model provides a solid foundation for a modern terminal emulator.

---

## Additional Resources

- [README.md](./README.md) - User guide and features
- [TASKS.md](./TASKS.md) - Development task list
- [specs/](./specs/) - Detailed specifications and contracts
- [CODE_REVIEW_FINDINGS.md](./CODE_REVIEW_FINDINGS.md) - Known issues and improvements

