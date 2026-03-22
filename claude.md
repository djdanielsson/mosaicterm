# MosaicTerm — AI Agent Reference

## Project Overview

MosaicTerm is a Rust GUI terminal emulator inspired by Warp. Commands and their outputs are grouped into discrete, scrollable "blocks" with a permanently pinned input prompt at the bottom. It runs zsh (with Oh My Zsh, plugins, themes, completions, fzf, etc.) inside a PTY and renders the UI with egui/eframe.

- **Language**: Rust (stable, 1.90+)
- **Version**: 0.4.0
- **License**: MIT
- **Platforms**: macOS (primary), Linux, Windows

## Constitution (Non-Negotiable Principles)

1. **TDD Methodology** — Tests written FIRST, Red-Green-Refactor cycle, minimum 80% coverage.
2. **Integration-First** — Don't reinvent the wheel. Integrate with zsh, fzf, bat, rg, fd, eza, jq, Oh My Zsh. PTY must be transparent to the underlying shell.
3. **Block-Based UI** — Commands and outputs grouped into discrete, scrollable blocks. Pinned input prompt at bottom. Native feel with proper fonts, ANSI colors, and escape codes.
4. **Cross-Platform Foundation** — Abstract PTY handling into platform modules. macOS is the MVP target.
5. **Latest Versions Policy** — Use latest stable crate versions. Immediate security updates. Full test suite must pass after updates.

## Architecture

```
src/
├── app/              # Application logic, UI rendering, input handling
│   ├── mod.rs        # Core app struct, update loop, rendering, native menu bar,
│   │                 # system font loading, notifications, icon discovery
│   ├── input.rs      # Keyboard shortcuts, pane management
│   ├── prompt.rs     # Prompt building (segments with colors)
│   ├── context.rs    # Git status + environment context detection
│   ├── commands.rs   # Command classification (cd, ssh, tui, etc.)
│   └── pane_tree.rs  # Split pane tree data structure
├── config/
│   ├── mod.rs        # Runtime Config struct, loading, merging, themes
│   └── prompt.rs     # PromptFormatter, segment rendering, style dispatch
├── session/
│   ├── mod.rs        # Session module entry
│   └── tmux_backend.rs  # TmuxSessionManager (CLI interaction)
├── pty/
│   ├── mod.rs        # PTY module, PtyHandle
│   └── manager.rs    # PtyManager (async read/write, lifecycle)
├── terminal/
│   └── mod.rs        # Terminal struct (state, working dir, PTY handle)
├── ui/
│   ├── mod.rs        # UiColors, theme structs
│   ├── text.rs       # AnsiTextRenderer
│   ├── input.rs      # InputPrompt
│   ├── command_block.rs    # CommandBlocks component
│   ├── completion_popup.rs # Tab completion popup
│   ├── scrollable_history.rs
│   ├── metrics.rs         # Performance metrics (render_with_ctx)
│   ├── ssh_prompt_overlay.rs
│   └── tui_overlay.rs     # Fullscreen TUI overlay (grace period, alt screen tracking)
├── completion.rs     # CompletionProvider (fzf integration)
├── context.rs        # ContextDetector (env detection: venv, nvm, rust, etc.)
├── models/
│   ├── mod.rs
│   ├── config.rs     # Serde config structs (PromptStyle, PromptConfig, etc.)
│   └── command_block.rs  # CommandBlock, OutputLine, ExecutionStatus
├── lib.rs            # Public API, module declarations, init functions
├── main.rs           # Entry point
├── error.rs          # Error types
├── state_manager.rs  # Global state (sessions, history, contexts)
├── commands/         # Command parsing utilities
├── execution/        # DirectExecutor
├── history.rs        # Persistent command history
├── security_audit.rs # Security event logging
├── ansi/             # ANSI escape code types
└── platform/         # Platform-specific utilities
```

## Core Dependencies

| Crate | Purpose |
|-------|---------|
| `eframe` 0.24 | GUI framework (re-exports `egui`) |
| `portable-pty` 0.9 | Cross-platform PTY |
| `vte` 0.15 | Terminal escape code parsing |
| `tokio` 1.49 | Async runtime |
| `serde` + `toml` | Configuration (TOML format) |
| `git2` 0.20 | Git status detection |
| `chrono` | Timestamps |
| `tracing` | Structured logging |
| `arboard` | Clipboard |
| `notify` | File watching |
| `cocoa` 0.24 | macOS native menu bar (macOS only) |
| `objc` 0.2 | Objective-C FFI for macOS APIs (macOS only) |

## Key Concepts

### Command Blocks
Each command execution creates a `CommandBlock` containing the command text, output lines (with ANSI codes), timestamp, status (`Pending` → `Running` → `Completed`/`Failed`), and working directory.

### PTY Management
`PtyManager` (async, `Arc`-wrapped) manages PTY processes. Each terminal gets a `PtyHandle`. Reader/writer threads communicate via `tokio::mpsc` channels. Output is polled in the UI update loop.

### Prompt System
Six configurable styles: Classic, Minimal, Powerline, Starship, OhMyZsh, Custom. Entry point: `build_prompt_segments()` in `src/app/prompt.rs` returns `Vec<PromptSegment>` with per-segment fg/bg/bold colors. Style dispatch happens in `PromptFormatter::render_segments()` in `src/config/prompt.rs`. Template variables: `$USER`, `$HOSTNAME`, `$PWD`, `$GIT_BRANCH`, `$GIT_STATUS`, `$VENV`, `$NODE_VERSION`, `$RUBY_VERSION`, `$DOCKER`, `$KUBE`.

### Environment Context Detection
`ContextDetector` identifies active environments from env vars and project marker files. Detected contexts: Python venv/conda, Node.js nvm, Ruby rbenv/rvm, Go, Rust, Java, Docker, Kubernetes, AWS, Terraform, mise/asdf. Rust/Go/Java are project-aware (only shown when `Cargo.toml`/`go.mod`/`pom.xml` exist in the directory tree).

### TUI Overlay
Interactive terminal apps (vim, top, htop, etc.) run in a fullscreen overlay using a `ScreenBuffer` with VTE parser. Supports alternate screen buffer detection, Ctrl key combinations, and double-Escape to close. An 800ms grace period (`in_grace_period()`) after activation prevents shell startup noise from triggering false exits. The overlay tracks `saw_alt_screen_enter` so exit sequences are only processed if an enter sequence was observed first.

### Tool Integrations
- **fzf**: If installed, used as backend for tab completion and Ctrl+R history search via `fzf --filter`.
- **zoxide**: If installed, `z`/`zi` commands are intercepted and resolved. `zoxide add` is called silently after `cd`.
- **tmux**: Optional session persistence via `TmuxSessionManager` CLI wrapper.

### Ghost Completion
Inline dimmed suggestion appears after cursor as you type. Press Tab or Right arrow (at end of input) to accept.

### Native macOS Menu Bar
On macOS, `setup_native_menu_bar()` uses `cocoa`/`objc` to add items to the native app menu: "About MosaicTerm" and a "Dev" menu with "Performance Metrics" and "Startup Log". Menu actions set `AtomicBool` flags (`NATIVE_MENU_ABOUT`, `NATIVE_MENU_DEV`, `NATIVE_MENU_PERF`) which are polled in the egui `update()` loop. On non-macOS platforms, keyboard shortcuts provide fallbacks (Ctrl+Shift+A, Ctrl+Shift+D).

### System Notifications
`send_system_notification` spawns a background thread calling `send_system_notification_sync`. On macOS: tries `terminal-notifier` with the app icon first, falls back to `osascript`. On Linux: `notify-send`. `find_app_icon_path` resolves `icon.png` relative to the executable (up to 3 parent directories). Notifications are sent for: font-not-found warnings at startup, and long-running commands (≥10s) completing while the window is unfocused.

### Font Loading
`find_system_font` searches OS font directories (macOS: `~/Library/Fonts`, `/Library/Fonts`, `/System/Library/Fonts`; Linux: `~/.local/share/fonts`, `/usr/share/fonts`; Windows: `%WINDIR%\Fonts`). Falls back to `fc-list` on Linux. Walks directories recursively (depth limit 10). Default font family: **JetBrains Mono**.

### Split Panes
`PaneTree` with `PaneNode` (Leaf/Branch) for native multi-pane support. Shortcuts: Ctrl+Shift+D (split right), Ctrl+Shift+E (split down), Ctrl+Shift+W (close), Ctrl+Shift+Arrows (navigate).

## Configuration

Config file: `~/.config/mosaicterm/config.toml` (or `$XDG_CONFIG_HOME/mosaicterm/config.toml`)

```toml
[ui]
theme = "dark"

[terminal]
shell = "/bin/zsh"
prompt_format = "$USER@$HOSTNAME:$PWD$ "

[prompt]
style = "ohmyzsh"  # classic | minimal | powerline | starship | ohmyzsh | custom
show_git = true
show_env = true

[session]
persistence = false
auto_restore = false
```

Env vars: `MOSAICTERM_CONFIG` (config path override), `MOSAICTERM_LOG` (log level).

## Build & Test

```bash
cargo build                    # Dev build
cargo run --release            # Release build + run
cargo test                     # All tests
cargo clippy --all-targets     # Lint
cargo fmt --check              # Format check
cargo bench                    # Benchmarks
```

## Testing Structure

```
tests/
├── integration/     # Full workflow tests
├── contract/        # Interface contract tests
├── unit/            # Component unit tests
├── security/        # Security-specific tests
├── proptest/        # Property-based tests
└── test_utils/      # Mock PTY, terminal, fixtures
```

## Threading Model

- **Main thread**: egui rendering loop (~60 FPS), input handling, state updates, polls AtomicBool flags from native menu
- **PTY reader thread**: Reads PTY output via tokio channel, sends to main thread
- **PTY writer thread**: Receives input from main thread, writes to PTY stdin
- **Notification threads**: Background threads for system notifications (spawned per notification, non-blocking)
- **Background tasks**: Environment queries, zoxide add, tmux operations

## Data Flow

1. User types in pinned input → Enter pressed
2. Command sent to PTY via `PtyManager::send_input`
3. PTY reader thread captures output chunks
4. Main thread polls output, creates `CommandBlock`
5. Block rendered in scrollable history area
6. Prompt updated with new working directory and context

## Performance Targets

- Frame time: <16ms (60 FPS)
- Command latency: <100ms input to display
- Memory: <200MB typical usage
- Startup: <2s to first prompt

## Key Design Decisions

- **egui immediate mode** — No retained state for UI widgets; re-render every frame
- **Block-on for async** — Uses `futures::executor::block_on` in the UI thread for PTY operations (acceptable because reads are non-blocking `try_read`)
- **Two Config structs** — `src/models/config.rs` (serde serialization) and `src/config/mod.rs` (runtime with theme application). Both must stay in sync.
- **System font loading** — `find_system_font` recursively searches OS font directories and tries `fc-list` on Linux. Default font: **JetBrains Mono**. Falls back to egui's built-in monospace if not found. For Powerline/Nerd Font glyphs, users should install and configure a Nerd Font.
- **Native macOS menu** — `setup_native_menu_bar()` uses `cocoa`/`objc` FFI. Action handlers set `AtomicBool` flags polled by the main thread in `update()`. This avoids cross-thread UI mutation.
- **Icon resolution** — `find_icon_path` (main.rs) and `find_app_icon_path` (app/mod.rs) search relative to the executable (up to 3 parent dirs) plus standard paths.
- **Platform abstraction** — PTY code uses `portable-pty` for cross-platform support; platform-specific code isolated in `src/platform/`

## Common Patterns

### Adding a new prompt style
1. Add variant to `PromptStyle` enum in `src/models/config.rs`
2. Add `render_<style>()` method in `src/config/prompt.rs`
3. Add dispatch in `render_segments()` match
4. Update README and CUSTOM_PROMPT.md

### Adding environment detection
1. Add variant to `ContextType` in `src/context.rs`
2. Add detection logic in `ContextDetector::detect_contexts_with_dir`
3. Add env var to query in `src/app/mod.rs` `handle_post_command_tasks`
4. Add parsing in `parse_env_and_detect_contexts`

### Adding a keyboard shortcut
1. Add handler in `src/app/input.rs`
2. Wire to app state in `MosaicTermApp`
3. Update README keyboard shortcuts table

## File References

| Document | Path |
|----------|------|
| Architecture | `docs/ARCHITECTURE.md` |
| Custom Prompts | `docs/CUSTOM_PROMPT.md` |
| Feature Spec | `specs/001-mosaicterm-terminal-emulator/spec.md` |
| Implementation Plan | `specs/001-mosaicterm-terminal-emulator/plan.md` |
| Task List | `specs/001-mosaicterm-terminal-emulator/tasks.md` |
| Research | `specs/001-mosaicterm-terminal-emulator/research.md` |
| Quickstart/MVP | `specs/001-mosaicterm-terminal-emulator/quickstart.md` |
| Data Model | `specs/001-mosaicterm-terminal-emulator/data-model.md` |
| Contracts | `specs/001-mosaicterm-terminal-emulator/contracts/` |
