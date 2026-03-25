# MosaicTerm Roadmap

Current version: **0.4.1**

---

## Release History

### v0.1.0 — Foundation

The initial release establishing the core terminal emulator.

- Block-based command history UI
- Pinned input prompt at bottom
- PTY process management (spawn shell, read/write)
- ANSI color and escape code rendering
- Basic tab completion
- macOS app bundle release
- Cross-platform CI (macOS, Linux, Windows)

### v0.1.1 — Polish

- macOS `.app` bundle in releases
- Shell echo fix
- Additional tests
- Lint and formatting cleanup

### v0.1.2 — Interactivity

- Persistent command history across sessions
- Ctrl+R history search
- Up-arrow command history navigation
- TUI overlay (initial -- vim, top, htop in fullscreen)
- Click-to-kill running commands
- Bug fixes for command status tracking

### v0.2.0 — Dependency Modernization

- Updated to `portable-pty` 0.9, `vte` 0.15, `git2` 0.20, `notify` 8.2
- Ctrl+R auto-focus improvement
- Cross-platform release fixes (Windows)

### v0.2.1 — SSH & Theming

- SSH session support with interactive prompt overlay (password/passphrase)
- Comprehensive theme color configuration via config file
- UI components use theme colors from config
- Hex color support in TOML config (`"#RRGGBB"`)

### v0.3.0 — Architecture & Quality

- Major code architecture refactoring
- Security hardening (config injection prevention, input validation)
- Local system timestamps (replaced UTC)
- 50+ bug fixes across all modules
- Comprehensive code review

### v0.4.1 — Code Quality & Maintenance (Current)

- **Code review fixes**: Three rounds of comprehensive code review addressing correctness, performance, and stability issues across 15+ source files
- **Security hardening**: Improved input validation, safer error handling, and tighter security audit logging
- **Configurable input history**: `max_history` now user-configurable via `InputConfig`
- **Dependency updates**: `cocoa` 0.24 → 0.26, tokio and other crate bumps via Dependabot
- **CI/CD improvements**: Added pre-commit hooks (formatting, linting, secret scanning via gitleaks), Dependabot for Cargo and pre-commit
- **Documentation overhaul**: Rewrote and restructured all docs (README, Architecture, Configuration, Quick Start, Theming, Custom Prompts, Roadmap) for clarity and accuracy
- **Doc linting**: Added markdownlint-cli2 to pre-commit for consistent documentation quality
- **Removed stale specs**: Cleaned up obsolete specification and contract documents
- **Bug fixes**: Addressed issues in PTY stream handling, ANSI text rendering, SSH overlay, completion provider, history management, and platform-specific path handling

### v0.4.0 — Features & Stability

- **6 prompt styles**: Classic, Minimal, Powerline, Starship, OhMyZsh, Custom
- **Split panes**: `Ctrl+Shift+D/E/W` with keyboard navigation
- **fzf integration**: Fuzzy tab completion and history search
- **zoxide integration**: Smart `z`/`zi` directory jumping
- **Ghost completion**: Inline dimmed suggestions (Tab/Right to accept)
- **Native macOS menu bar**: About dialog, Dev menu (Performance Metrics, Startup Log)
- **Desktop notifications**: Long-running commands (>10s) notify when window unfocused
- **System font loading**: Search OS font directories, `fc-list` fallback on Linux
- **Environment detection**: 20+ contexts (Python, Node, Rust, Go, Docker, K8s, AWS, etc.)
- **Performance metrics panel**: Live frame time, FPS, memory stats
- **TUI overlay improvements**: 800ms grace period, stable exit detection, alt screen tracking
- **Tmux session persistence**: Optional tmux backend
- **3 built-in themes**: Default Dark, Default Light, High Contrast
- **5 ANSI color presets**: Monokai, Solarized Dark/Light, Dracula, Nord
- **Clipboard support**: Copy from command blocks via context menu
- **Mouse scrolling**: Scroll wheel and drag support in history
- **Configurable TUI app list**: Add custom apps to fullscreen detection
- **Config hot-reload**: File watcher implemented (not yet wired to live app)

---

## Planned

### Near-term

| Feature | Description | Complexity |
|---------|-------------|------------|
| **PTY resize propagation** | Forward window/pane resize to PTY so TUI apps (vim, htop) get correct dimensions. Infrastructure exists (`pending_resize` in TUI overlay, `resize()` in Terminal) but not wired end-to-end. | Medium |
| **Config hot-reload (complete)** | `ConfigWatcher` is implemented but not started from the app. Wire it up so config changes apply without restart. | Low |
| **Block collapse** | Allow collapsing command block output. The `expanded` flag exists on rendered blocks but no toggle is exposed. | Low |
| **Draggable pane dividers** | Allow mouse-dragging to resize split panes. Currently fixed-width dividers. | Medium |

### Medium-term

| Feature | Description | Complexity |
|---------|-------------|------------|
| **Inline search in output** | Search/filter within command block output (Ctrl+F). | Medium |
| **Virtual scrolling** | Virtualized row rendering for blocks with very large output (100K+ lines). Currently all visible lines are rendered. | High |
| **Multi-tab interface** | Multiple terminal sessions in tabs within a single window. | High |

### Long-term

| Feature | Description | Complexity |
|---------|-------------|------------|
| **Plugin system** | External plugins for custom commands, themes, and completion providers. | High |
| **Workspace/project support** | Named workspaces with project-specific configurations and saved layouts. | High |
| **Remote shell (SSH in PTY)** | Full SSH sessions running inside a PTY (beyond the current interactive prompt overlay). | High |
| **Block actions** | Additional block operations: export to markdown, share, pin, bookmark. | Medium |
| **Image rendering** | Inline image display in terminal output (iTerm2/Kitty protocol). | High |
| **Ligature support** | Font ligature rendering for coding fonts like Fira Code. | Medium |

---

## Contributing

Want to work on one of these? Check the [contributing guide](../README.md#contributing) and open an issue to discuss your approach before starting.
