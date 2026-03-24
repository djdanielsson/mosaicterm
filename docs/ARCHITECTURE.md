# MosaicTerm Architecture

**Version:** 0.4.0
**Last Updated:** March 24, 2026

---

## Overview

MosaicTerm is a Rust-based GUI terminal emulator that groups commands and their outputs into discrete, scrollable "blocks" with a permanently pinned input prompt. It runs your shell (zsh, bash, fish) inside a PTY and renders the UI with egui/eframe.

### Key Technologies

| Crate | Purpose |
|-------|---------|
| `eframe` 0.24 | GUI framework (re-exports `egui`, immediate mode) |
| `portable-pty` 0.9 | Cross-platform pseudoterminal |
| `vte` 0.15 | ANSI escape sequence parsing |
| `tokio` 1.49 | Async runtime |
| `serde` + `toml` | Configuration (TOML format) |
| `git2` 0.20 | Git status detection |
| `cocoa` 0.24 / `objc` 0.2 | macOS native menu bar (macOS only) |
| `tracing` | Structured logging |

### Design Goals

1. **Simplicity** -- Clean, understandable codebase
2. **Performance** -- <16ms frames, <100ms command latency, <200MB memory
3. **Reliability** -- Graceful error handling, no panics in production
4. **Extensibility** -- Modular design for future enhancements

---

## Module Structure

```
src/
├── main.rs              # Entry point, CLI args, icon loading
├── lib.rs               # Library exports, module declarations
├── error.rs             # Error types and Result aliases
├── state_manager.rs     # Global state (sessions, history, contexts)
│
├── app/                 # Main application
│   ├── mod.rs           # Core app struct, update loop, rendering,
│   │                    # native menu bar, font loading, notifications
│   ├── input.rs         # Keyboard shortcuts, pane management
│   ├── prompt.rs        # Prompt building (Vec<PromptSegment>)
│   ├── context.rs       # Git status + environment context
│   ├── commands.rs      # Command classification (cd, ssh, tui)
│   └── pane_tree.rs     # Split pane tree data structure
│
├── config/              # Configuration management
│   ├── mod.rs           # RuntimeConfig, Config struct, merging
│   ├── prompt.rs        # PromptFormatter, 6 render styles
│   ├── theme.rs         # ThemeManager, 3 built-in themes, 5 color presets
│   ├── shell.rs         # Shell detection and configuration
│   ├── loader.rs        # Config file discovery and loading
│   └── watcher.rs       # Config file watching for hot-reload
│
├── session/             # Session persistence
│   ├── mod.rs           # Session module
│   └── tmux_backend.rs  # Tmux CLI integration
│
├── pty/                 # PTY management
│   ├── mod.rs           # PtyHandle, module exports
│   ├── manager.rs       # PtyManager, lifecycle coordination
│   ├── process.rs       # PTY process spawning
│   └── streams.rs       # PtyStreams, async I/O abstraction
│
├── terminal/            # Terminal emulation
│   └── mod.rs           # Terminal struct, session, working dir
│
├── models/              # Data models
│   ├── mod.rs           # Model exports
│   ├── config.rs        # Serde config structs (Theme, PromptConfig, etc.)
│   └── command_block.rs # CommandBlock, OutputLine, ExecutionStatus
│
├── ui/                  # UI components
│   ├── mod.rs           # LayoutManager, breakpoints
│   ├── colors.rs        # UiColors (egui color provider from theme)
│   ├── text.rs          # AnsiTextRenderer, ColorScheme
│   ├── input.rs         # InputPrompt widget
│   ├── command_block.rs # CommandBlocks rendering
│   ├── completion_popup.rs # Tab completion popup
│   ├── scrollable_history.rs # Scrollable history
│   ├── metrics.rs       # Performance metrics panel
│   ├── ssh_prompt_overlay.rs # SSH password/passphrase prompts
│   └── tui_overlay.rs   # Fullscreen TUI overlay (vim, top, etc.)
│
├── completion.rs        # CompletionProvider (fzf integration)
├── context.rs           # ContextDetector (20+ environments)
├── history.rs           # Persistent command history
├── security_audit.rs    # Security event logging
├── platform/            # Platform-specific utilities
├── ansi/                # ANSI escape code types
├── commands/            # Command parsing utilities
└── execution/           # DirectExecutor (for tests)
```

---

## Data Flow

### Command Execution

```
User types in pinned input → Enter
    │
    ▼
MosaicTermApp::update() handles Enter key
    │
    ▼
PtyManager::send_input() → writer thread → PTY master → shell
    │
    ▼
Shell executes command, produces output
    │
    ▼
PTY master → reader thread → tokio channel → main thread polls
    │
    ▼
Terminal::process_output() → ANSI parsing → OutputLines
    │
    ▼
CommandBlock updated with output lines
    │
    ▼
egui renders block in scrollable history
    │
    ▼
Prompt updated with new working directory and context
```

### Configuration Loading

```
main() → parse CLI args
    │
    ├─ Search config paths:
    │   $MOSAICTERM_CONFIG → ~/.config/mosaicterm/config.toml → ./mosaicterm.toml
    │
    ├─ Create RuntimeConfig:
    │   ├─ Load Config (TOML/JSON)
    │   ├─ Initialize ThemeManager (3 built-in themes)
    │   ├─ Initialize ShellManager (detect current shell)
    │   └─ Apply theme_name preset
    │
    └─ Pass to MosaicTermApp::with_config()
        ├─ Build PromptFormatter from prompt config
        ├─ Build UiColors from theme
        └─ Apply font settings
```

---

## Threading Model

MosaicTerm uses a **hybrid threading model**:

```
┌──────────────────────────────────────────────────┐
│                  Main Thread                      │
│  egui UI loop (~60 FPS)                          │
│  - Render UI                                     │
│  - Handle keyboard/mouse events                  │
│  - Poll PTY channels (non-blocking try_recv)     │
│  - Process ANSI output                           │
│  - Poll AtomicBool flags for native menu actions │
│  - Request repaints (immediate or 100ms delayed) │
└──────────────┬───────────────────────────────────┘
               │ tokio channels
    ┌──────────┴──────────┐
    │                     │
    ▼                     ▼
┌──────────────┐  ┌──────────────┐
│ PTY Reader   │  │ PTY Writer   │
│ Thread       │  │ Thread       │
│              │  │              │
│ Blocking     │  │ Blocking     │
│ read() loop  │  │ recv() +     │
│ → send to    │  │ write() loop │
│   channel    │  │              │
└──────┬───────┘  └──────┬───────┘
       │                 │
       └────────┬────────┘
                │ OS file descriptors
                ▼
       ┌─────────────────┐
       │   PTY Master    │
       │   (OS Resource) │
       └────────┬────────┘
                │
                ▼
       ┌─────────────────┐
       │   PTY Slave     │
       │   (Shell/CMD)   │
       └─────────────────┘
```

Additional threads:
- **Notification threads**: Background threads for desktop notifications (spawned per event, non-blocking)
- **Background tasks**: Environment queries, `zoxide add`, tmux operations

### Synchronization

- `PtyManager` protected by `Arc<Mutex<>>` with 3-retry lock acquisition (100µs sleep between)
- Unbounded tokio channels for PTY output (bounded would risk data loss)
- `AtomicBool` flags for native macOS menu actions (polled in `update()`)

---

## UI Update Cycle

MosaicTerm uses **on-demand rendering**:

```rust
if has_running_command || has_pending_output || completion_popup_visible {
    ctx.request_repaint();                                    // Immediate (next frame)
} else {
    ctx.request_repaint_after(Duration::from_millis(100));    // Delayed polling
}
```

This achieves ~1% CPU when idle, full 60 FPS when commands are running.

### Layout

```
┌─────────────────────────────────────────────────┐
│  Central Panel (Scrollable Command History)      │
│  ┌────────────────────────────────────────────┐ │
│  │ CommandBlock 1                             │ │
│  │ $ echo "hello"                             │ │
│  │ hello                                      │ │
│  │ [✓ Completed in 50ms]                      │ │
│  └────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────┐ │
│  │ CommandBlock 2                             │ │
│  │ $ ls -la                                   │ │
│  │ [... output ...]                           │ │
│  │ [⏱ Running for 2.3s...]                    │ │
│  └────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────┐
│  Bottom Panel (Pinned Input) - ALWAYS VISIBLE   │
│  ~/project > █                                   │
│  (Tab completion popup appears above if active)  │
└─────────────────────────────────────────────────┘
```

---

## Key Subsystems

### Command Blocks

Each command execution creates a `CommandBlock`:
- Command text
- Output lines (with ANSI codes preserved)
- Timestamp
- Status: `Pending` → `Running` → `Completed` / `Failed` / `Timedout`
- Working directory
- Exit code

Limits: 50K lines per block, 10K chars per line (truncation with user notice).

### PTY Management

`PtyManager` coordinates PTY lifecycle:
1. **Create**: Spawn shell in PTY via `portable-pty`, start reader/writer threads
2. **Communicate**: `send_input()` / `try_read_output_now()` via channels
3. **Monitor**: `is_alive()`, process info
4. **Terminate**: Graceful SIGTERM, fallback to SIGKILL

### Prompt System

Six styles rendered by `PromptFormatter::render_segments()` → `Vec<PromptSegment>`. Each segment has fg/bg colors and bold flag. Entry point: `build_prompt_segments()` in `app/prompt.rs`. Variable substitution: `$USER`, `$HOSTNAME`, `$PWD`, `$GIT_BRANCH`, `$GIT_STATUS`, `$VENV`, `$NODE_VERSION`, `$RUBY_VERSION`, `$DOCKER`, `$KUBE`.

### Theme System

`ThemeManager` in `config/theme.rs`:
- 3 built-in themes: `default-dark`, `default-light`, `high-contrast`
- 5 ANSI color presets: Monokai, Solarized Dark/Light, Dracula, Nord
- Custom themes: JSON import/export
- Each theme defines: `ColorPalette`, `Typography`, `UiStyles`

`UiColors` in `ui/colors.rs` converts theme colors to `egui::Color32` for rendering.

### Completion System

`CompletionProvider` provides tab completion:
- Command completion (scans `$PATH`, refreshes every 5 minutes)
- File/directory completion
- **fzf backend**: If installed, used for fuzzy matching and Ctrl+R history search
- **Ghost completion**: Inline dimmed suggestions, accepted with Tab or Right arrow

### TUI Overlay

Interactive apps (vim, top, htop, etc.) detected by `TuiAppConfig::fullscreen_commands` run in a fullscreen overlay with VTE-based screen buffer. Features:
- Alternate screen buffer tracking
- 800ms grace period to prevent false exits
- All Ctrl key combinations forwarded
- Double-Escape to close

### Environment Context Detection

`ContextDetector` identifies 20+ development environments:
- Python venv/conda, Node.js nvm, Ruby rbenv/rvm
- Rust, Go, Java (project-aware: only shown near `Cargo.toml`/`go.mod`/`pom.xml`)
- Docker, Kubernetes, AWS, Terraform, mise/asdf

### Split Panes

`PaneTree` with `PaneNode` (Leaf/Branch) for native multi-pane support:
- `Ctrl+Shift+D`: Split right
- `Ctrl+Shift+E`: Split down
- `Ctrl+Shift+W`: Close pane
- `Ctrl+Shift+Arrows`: Navigate

### Desktop Notifications

`send_system_notification` spawns background threads. On macOS: tries `terminal-notifier` first, falls back to `osascript`. On Linux: `notify-send`. Sent when long-running commands (≥10s) complete while window is unfocused.

### Native macOS Menu Bar

`setup_native_menu_bar()` uses `cocoa`/`objc` FFI. Menu actions set `AtomicBool` flags polled by the main thread in `update()`, avoiding cross-thread UI mutation. Includes About dialog and Dev menu (Performance Metrics, Startup Log).

---

## Performance

### Targets

| Metric | Target |
|--------|--------|
| Frame time | <16ms (60 FPS) |
| Command latency | <100ms input to display |
| Memory | <200MB typical usage |
| Startup | <2s to first prompt |

### Optimizations

- **Output batching**: All lines processed in batch, single UI update
- **Viewport culling**: Only render visible command blocks
- **Conditional repaints**: Immediate when active, 100ms polling when idle
- **Size limits**: 50K lines/block, 10K chars/line
- **Lazy ANSI parsing**: Parsed on-demand during rendering
- **Completion cache**: Refreshed every 5 minutes, not every frame
- **Buffered I/O**: 128KB read, 8KB write buffers

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| egui immediate mode | No retained widget state; re-render every frame. Simple and fast. |
| `block_on` for async in UI | Acceptable because PTY reads are non-blocking `try_read` |
| Two Config structs | `models/config.rs` (serde) and `config/mod.rs` (runtime). Both must stay in sync. |
| Unbounded output channels | Prevents data loss; bounded would require backpressure. |
| `AtomicBool` for menu actions | Avoids cross-thread UI mutation on macOS. |
| `portable-pty` | Cross-platform PTY abstraction; platform code in `src/platform/`. |
| System font search | Recursive OS directory search + `fc-list` fallback on Linux. Default: JetBrains Mono. |

---

## Error Handling

All fallible operations return `Result<T, Error>`. No panics in production.

Patterns:
- **Fallback to defaults**: `Config::load().unwrap_or_default()`
- **Log and continue**: Debug log failures, don't crash
- **User feedback**: Status messages for visible errors

---

## Related Documentation

- [Quick Start](QUICKSTART.md)
- [Configuration Reference](CONFIGURATION.md)
- [Custom Prompt Guide](CUSTOM_PROMPT.md)
- [Theming Guide](THEMING.md)
