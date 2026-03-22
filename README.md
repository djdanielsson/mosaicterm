# MosaicTerm

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/djdanielsson/mosaicterm/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml/badge.svg)](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml)

A modern GUI terminal emulator written in Rust, inspired by [Warp](https://warp.dev). MosaicTerm groups commands and their outputs into discrete, scrollable blocks while maintaining a permanently pinned input prompt at the bottom - creating a clean, organized terminal experience that feels native to your workflow.

![MosaicTerm Screenshot](https://github.com/djdanielsson/mosaicterm/blob/main/icon.png)

## ✨ Key Features

- **Block-Based History**: Commands and their outputs are grouped into discrete, scrollable blocks with color-coded status stripes
- **Pinned Input Prompt**: Always-visible input field at the bottom for seamless command entry
- **Configurable Prompt Styles**: Classic, Minimal, Powerline, Starship, Oh My Zsh, and fully Custom prompt styles
- **Split Panes**: Native multi-pane support with keyboard shortcuts (Ctrl+Shift+D/E/W)
- **TUI App Support**: Run vim, top, htop, and other interactive terminal apps in a fullscreen overlay with stable exit detection
- **SSH Session Support**: Seamless SSH connections with interactive prompt overlays for passwords and passphrases
- **Comprehensive Theming**: Full color customization via config file with hex color support (Solarized, etc.)
- **System Font Loading**: Uses any installed system font -- searches OS font directories with `fc-list` fallback on Linux (default: JetBrains Mono)
- **Smart Environment Detection**: Auto-detects Python venv, conda, nvm, rbenv, Go, Rust, Java, Docker, Kubernetes, AWS, Terraform, and more -- only shown when relevant to the current project directory
- **fzf Integration**: If fzf is installed, tab completion and history search (Ctrl+R) automatically use fuzzy matching
- **zoxide Integration**: If zoxide is installed, `z` and `zi` commands are automatically intercepted for smart directory jumping
- **Tmux Session Persistence**: Optional tmux-backed sessions for persistence across app restarts
- **Ghost Completion**: Inline dimmed suggestions appear as you type -- press Tab or Right arrow to accept
- **Desktop Notifications**: Get notified when long-running commands (>10s) complete while the window is unfocused
- **Native macOS Menu Bar**: About dialog and Dev menu (Performance Metrics, Startup Log) integrated into the native menu bar
- **Tab Completion**: Intelligent command and path completion with popup UI (double-tab to activate)
- **Native ANSI Support**: Full color support for `ls`, `bat`, `fzf`, and other CLI tools
- **Zsh Integration**: Seamless support for zsh with Oh My Zsh, plugins, and completions
- **Modern GUI**: Built with [egui](https://github.com/emilk/egui) for native performance and feel
- **Cross-Platform Ready**: Designed for macOS (Intel & Apple Silicon), Linux (x86_64 & ARM64), and Windows (x86_64 & ARM64)

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.90+ stable toolchain
- **macOS**: 14.0+ (Intel and Apple Silicon) or **Linux** (Ubuntu 20.04+, Fedora 34+, Debian 11+, or similar) on x86_64 or ARM64
- **Windows**: Windows 10+ on x86_64 or ARM64
- **Shell**: bash, zsh, or fish (Unix) / PowerShell or cmd.exe (Windows)
- **Optional**: fzf (fuzzy completion), zoxide (smart cd), tmux (session persistence), eza, bat, rg, fd

### Installation

#### Option 1: Download Pre-built Release (Recommended)

**macOS:**
1. Download the latest `MosaicTerm-macos-{arm64|x64}.app.tar.gz` from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page
2. Extract: `tar xzf MosaicTerm-macos-*.app.tar.gz`
3. **Allow the app to run** (choose one method):
   - **Terminal method**: `xattr -d com.apple.quarantine MosaicTerm.app`
   - **Alternative**: `sudo spctl --add /path/to/MosaicTerm.app` (allows this specific app)
4. Move to Applications: `mv MosaicTerm.app /Applications/`
5. Launch from Applications folder or Spotlight

**Linux:**
1. Download the latest `mosaicterm-linux-{x64|arm64}.tar.gz` from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page
2. Extract: `tar xzf mosaicterm-linux-*.tar.gz`
3. Move to PATH: `sudo mv mosaicterm /usr/local/bin/`
4. Run: `mosaicterm`

**Windows:**
1. Download the latest `mosaicterm-windows-{x64|arm64}.zip` from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page
2. Extract: `Expand-Archive mosaicterm-windows-*.zip`
3. Add to PATH or run directly: `mosaicterm.exe`

#### Option 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/djdanielsson/mosaicterm.git
cd mosaicterm

# Build and run
cargo run --release
```

### First Launch

1. MosaicTerm automatically starts zsh in a PTY
2. Type commands in the bottom input field
3. Press Enter to execute - output appears in a new block above
4. Scroll through command history while keeping the input prompt always visible

**macOS**: Use the native menu bar for About MosaicTerm, Dev tools (Performance Metrics, Startup Log), and standard app controls.

**All platforms**: Press `Ctrl+Shift+P` to toggle the Performance Metrics panel.

## 📋 Linux-Specific Notes
- **Config Location**: Uses XDG Base Directory specification
  - Primary: `$XDG_CONFIG_HOME/mosaicterm/config.toml` (defaults to `~/.config/mosaicterm/config.toml`)
  - Fallback: `~/.mosaicterm/config.toml`
- **Display Server**: Works with both X11 and Wayland
- **Wayland Support**:
  - MosaicTerm automatically detects and configures for Wayland
  - The application automatically forces integer DPI scaling (1x or 2x) to help prevent buffer size errors
  - **If you still encounter "buffer size must be integer multiple of buffer_scale" errors:**
    ```bash
    # Option 1: Use X11 instead (MOST RELIABLE - recommended)
    WINIT_UNIX_BACKEND=x11 mosaicterm
    # Or set environment variable before running:
    export WINIT_UNIX_BACKEND=x11
    mosaicterm

    # Option 2: Disable fractional scaling in system settings
    # GNOME: Settings > Displays > Scale to 100% or 200% (not 125%, 150%, etc.)
    # KDE: System Settings > Display and Monitor > Scale Display to 100% or 200%
    # Fedora: Settings > Displays > Fractional Scaling OFF, use 100% or 200%

    # Option 3: Force X11 via environment variable (persists across sessions)
    echo 'export WINIT_UNIX_BACKEND=x11' >> ~/.bashrc  # or ~/.zshrc
    ```
  - **Why this happens**: Wayland requires buffer sizes to be integer multiples of the buffer scale. Even with integer DPI scaling, some window operations can create odd-sized buffers. This is a limitation in how egui/winit handles Wayland buffer creation.
  - **Best solution**: Use X11 (`WINIT_UNIX_BACKEND=x11`) which doesn't have this limitation.
- **Dependencies**: Most Linux distributions include required system libraries. If you encounter build issues, install:
  ```bash
  # Ubuntu/Debian
  sudo apt-get install build-essential libssl-dev pkg-config libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libgtk-3-dev

  # Fedora/RHEL
  sudo dnf install gcc openssl-devel pkg-config libX11-devel libxcb-devel libxkbcommon-devel gtk3-devel
  ```

## 🏗️ Architecture

MosaicTerm is built with a modular architecture:

```
src/
├── main.rs              # Application entry point
├── app/                 # Main GUI application
│   ├── mod.rs           # Core app logic and UI rendering
│   ├── input.rs         # Keyboard shortcuts and input handling
│   ├── commands.rs      # Command detection (TUI, cd, etc.)
│   ├── context.rs       # Git and environment context detection
│   ├── prompt.rs        # Prompt building with style support
│   └── pane_tree.rs     # Split pane tree data structure
├── config/              # Configuration management
│   ├── mod.rs           # Runtime config, themes, hot-reload
│   ├── prompt.rs        # Prompt segment rendering (6 styles)
│   └── loader.rs        # Config file discovery and loading
├── session/             # Session persistence
│   ├── mod.rs           # Session module
│   └── tmux_backend.rs  # Tmux CLI integration
├── pty/                 # Pseudoterminal management
│   ├── manager.rs       # PTY lifecycle
│   ├── process.rs       # Process spawning
│   └── streams.rs       # Async I/O handling
├── terminal/            # Terminal emulation
├── ui/                  # GUI components
│   ├── tui_overlay.rs   # Fullscreen TUI app overlay
│   ├── completion_popup.rs # Tab completion popup
│   ├── metrics.rs       # Performance metrics panel
│   ├── ssh_prompt_overlay.rs # SSH password/passphrase prompts
│   └── ...              # Blocks, input, scroll, colors
├── completion.rs        # Command/path completion with fzf support
├── context.rs           # Environment context detection (20+ tools)
└── models/              # Data structures
```

### Key Technologies

- **[egui/eframe](https://github.com/emilk/egui)**: Immediate mode GUI framework
- **[portable-pty](https://github.com/wez/wezterm/tree/main/crates/portable-pty)**: Cross-platform PTY support
- **[vte](https://github.com/alacritty/vte)**: Terminal emulation and ANSI parsing
- **[tokio](https://tokio.rs/)**: Async runtime for I/O operations
- **[cocoa](https://crates.io/crates/cocoa) / [objc](https://crates.io/crates/objc)** (macOS only): Native menu bar integration

## 🔧 Configuration

MosaicTerm supports TOML-based configuration. Create `~/.config/mosaicterm/config.toml`:

```toml
[ui]
font_family = "JetBrainsMono Nerd Font"  # Any installed system font
font_size = 13
theme_name = "default-dark"

[terminal]
shell_type = "Zsh"
shell_path = "/bin/zsh"
prompt_format = "$USER@$HOSTNAME:$PWD$ "

[pty]
buffer_size = 1048576

# Prompt style configuration
[prompt]
# Available styles: "classic", "minimal", "powerline", "starship", "ohmyzsh", "custom"
style = "minimal"
show_git = true
show_env = true

# Session persistence (requires tmux installed)
[session]
persistence = false
auto_restore = false
```

### Prompt Styles

MosaicTerm supports six built-in prompt styles. Set `[prompt].style` in your config:

| Style | Description | Example |
|-------|-------------|---------|
| `classic` | Traditional `user@host:path$` format | `ddaniels@mac:~/projects$` |
| `minimal` | Clean path-only with `>` (default) | `~/projects >` |
| `powerline` | Colored segments with arrow separators | `user  ~/projects  main +2 !1` |
| `starship` | Colored text segments with emoji icons | `~/projects  main +2 !1` |
| `ohmyzsh` | Oh My Zsh-inspired with arrow prompt | `ddaniels at mac in ~/projects git:(main) ->` |
| `custom` | User-defined segments from config | (see below) |

#### Custom Prompt Segments

For full control, use `style = "custom"` with segment definitions:

```toml
[prompt]
style = "custom"

[[prompt.segments]]
content = "$USER@$HOSTNAME"
fg = "#00D2D2"
bold = true

[[prompt.segments]]
content = "$PWD"
fg = "#50B4FF"
bold = true

[[prompt.segments]]
content = "$GIT_BRANCH"
fg = "#C8C8FF"
condition = "git"

[[prompt.segments]]
content = "> "
fg = "#64DC64"
bold = true
```

#### Template Variables

Available variables for `prompt_format` and custom segments:

| Variable | Description |
|----------|-------------|
| `$USER` | Current username |
| `$HOSTNAME` | Machine hostname |
| `$PWD` | Current working directory (with `~` substitution) |
| `$GIT_BRANCH` | Current git branch name |
| `$GIT_STATUS` | Git status indicators (+staged !modified ?untracked) |
| `$VENV` | Active Python virtual environment name |
| `$NODE_VERSION` | Active Node.js version (via nvm) |
| `$RUBY_VERSION` | Active Ruby version (via rbenv/rvm) |
| `$DOCKER` | Docker context (if active) |
| `$KUBE` | Kubernetes context (if active) |

See the **[Custom Prompt Guide](docs/CUSTOM_PROMPT.md)** for more details and examples.

### Environment Variables

- `MOSAICTERM_CONFIG`: Override default config path
- `MOSAICTERM_LOG`: Set logging level (`error`, `warn`, `info`, `debug`, `trace`)

## 🌍 Environment Management

MosaicTerm fully supports environment management tools and automatically detects active development contexts. Unlike some terminal emulators, MosaicTerm loads your shell RC files (`.bashrc`, `.zshrc`, etc.) by default, making these tools work seamlessly.

**Smart Context Detection**: MosaicTerm detects and displays active environments in your prompt:

| Context | Detection | Display |
|---------|-----------|---------|
| Python venv | `VIRTUAL_ENV` env var | `venv:myproject` |
| Conda | `CONDA_DEFAULT_ENV` env var | `conda:myenv` |
| Node.js | `NVM_BIN` env var (nvm) | `node:18.20.0` |
| Ruby | `RBENV_VERSION` or `rvm_ruby_string` | `ruby:3.2.0` |
| Rust | `RUSTUP_TOOLCHAIN` + `Cargo.toml` present | `rust:stable` |
| Go | `GOVERSION` + `go.mod` present | `go:1.22.0` |
| Java | `JAVA_HOME` + `pom.xml`/`build.gradle` present | `java:jdk-21` |
| Docker | `DOCKER_HOST` or `DOCKER_CONTEXT` | `docker:default` |
| Kubernetes | `KUBECONFIG` | `k8s:production` |
| AWS | `AWS_PROFILE` or `AWS_DEFAULT_PROFILE` | `aws:staging` |
| Terraform | `TF_WORKSPACE` | `tf:dev` |
| mise/asdf | `MISE_`* or `ASDF_`* env vars | `mise` |
| Git | Git repository detection via `git2` | `main +2 !3 ?1` |

**Project-aware detection**: Language-specific contexts (Rust, Go, Java) are only shown when you're inside a relevant project directory (checked up to 5 parent levels for project files like `Cargo.toml`, `go.mod`, `pom.xml`).

### How It Works

MosaicTerm maintains a **persistent shell session** where environment changes naturally persist across commands:

1. Shell RC files (`.bashrc`, `.zshrc`, etc.) are loaded on startup
2. Environment tools (nvm, conda, etc.) are initialized from your RC files
3. When you activate an environment (e.g., `source venv/bin/activate`), it stays active
4. The prompt automatically updates to show the active environment name
5. All subsequent commands run within that environment until you explicitly deactivate it
6. Use the environment's deactivation command (e.g., `deactivate` for venv, `conda deactivate` for Conda) to exit

**Note**: If you prefer isolated shell sessions without RC files, you can disable this in the configuration:

```toml
[terminal]
# Disable RC file loading for isolated shell environment
load_rc_files = false
```

## 🎯 TUI Applications

MosaicTerm supports fullscreen TUI applications via an overlay mode. When you run apps like `vim`, `top`, `htop`, `nano`, `less`, etc., they open in a fullscreen overlay with proper terminal emulation.

**Supported TUI apps** (auto-detected): vim, nvim, vi, nano, emacs, helix, micro, top, htop, btop, less, more, man, tmux, screen, ranger, nnn, mc, and more.

**Controls**:
- **Double-Escape**: Close the TUI overlay and return to block mode
- **Exit button**: Click the Exit button in the overlay header
- All standard keys (including Escape for vim normal mode) are forwarded to the app

**Keyboard shortcuts** work inside TUI apps: Ctrl+C, Ctrl+Z, Ctrl+D, arrow keys, function keys, and all Ctrl+letter combinations.

**Stable exit detection**: An 800ms grace period after TUI launch prevents shell startup noise from accidentally closing the overlay.

### Pane Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+D` | Split pane right |
| `Ctrl+Shift+E` | Split pane down |
| `Ctrl+Shift+W` | Close current pane |
| `Ctrl+Shift+Arrows` | Navigate between panes |

## 🔌 Tool Integrations

MosaicTerm automatically detects and integrates with common CLI tools when they are installed. No configuration required -- if the tool is in your PATH, it's used automatically. If not, MosaicTerm falls back to built-in behavior.

| Tool | Integration |
|------|-------------|
| **fzf** | Fuzzy matching for tab completion and Ctrl+R history search |
| **zoxide** | Smart directory jumping via `z` and `zi` commands; silent `zoxide add` after `cd` |
| **tmux** | Session persistence (opt-in via `[session].persistence = true`) |
| **fd** | Detected for future fast file search integration |
| **bat** | Detected for future syntax-highlighted previews |
| **eza** | Detected for future enhanced `ls` integration |

## 🗺️ Roadmap

### Completed
- [x] SSH session support with interactive prompts
- [x] Cross-platform builds (Linux, macOS, Windows)
- [x] Configuration hot-reload
- [x] Command history persistence
- [x] TUI overlay for fullscreen apps (vim, top, htop, etc.)
- [x] Configurable prompt styles (Classic, Minimal, Powerline, Starship, OhMyZsh, Custom)
- [x] Expanded environment detection (20+ tools)
- [x] Native split panes (PaneTree)
- [x] Tmux session persistence backend
- [x] fzf integration for completions and history search
- [x] zoxide integration for smart directory jumping
- [x] Project-aware context detection (Rust, Go, Java only in project dirs)
- [x] Event-driven architecture for PTY output (PtyEventBus)
- [x] Native macOS menu bar (About, Dev menu with Performance Metrics and Startup Log)
- [x] Desktop notifications for long-running commands
- [x] System font loading from OS font directories
- [x] Ghost completion (inline dimmed suggestions)
- [x] Performance metrics panel with live stats
- [x] Stable TUI exit detection with grace period
- [x] Comprehensive code review (50+ bug fixes)
- [x] Security hardening (config injection prevention, input validation, password isolation)

### Future Goals
- [ ] Full PTY resize propagation for TUI overlay
- [ ] Inline search and filtering within output blocks
- [ ] Virtual scrolling for large output
- [ ] Draggable pane dividers for resize
- [ ] Plugin system architecture
- [ ] Multi-tab interface
- [ ] Customizable block actions
- [ ] Workspace/project support

## 🧪 Development

### Building from Source

```bash
# Debug build
cargo build

# Release build with optimizations
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

### Testing Strategy

MosaicTerm follows TDD principles with comprehensive test coverage:

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration

# Performance benchmarks
cargo bench

# Contract tests (API compliance)
cargo test --test contract
```

### Code Quality

```bash
# Lint code
cargo clippy

# Format code
cargo fmt

# Check documentation
cargo doc --open
```

### Releases

Releases are automated via GitHub Actions. Create a version tag (e.g., `v0.2.0`) and push it to trigger automated builds for all platforms.

## 🤝 Contributing

We welcome contributions!

### Development Setup

1. Fork and clone the repository
2. Install Rust 1.90+ stable toolchain
3. Run `cargo test` to ensure everything works
4. Create a feature branch: `git checkout -b feature/your-feature`
5. Make your changes with tests
6. Run the full test suite: `cargo test --all-features`
7. Submit a pull request

### Automated Quality Checks

MosaicTerm uses GitHub Actions for automated code quality checks. The following checks run automatically on every commit and pull request:

- **Compilation**: `cargo check` on Linux, macOS, and Windows
- **Tests**: Full test suite execution
- **Formatting**: `cargo fmt --check` to ensure consistent code style
- **Linting**: `cargo clippy` with warnings treated as errors
- **Documentation**: `cargo doc` to verify all docs compile
- **Security**: `cargo audit` for dependency vulnerabilities (temporarily disabled)
- **MSRV**: Ensures compatibility with Rust 1.90+

**Before pushing**: Run `cargo fmt` and `cargo clippy` locally to fix any issues before CI catches them.

### Project Structure

- `specs/`: Feature specifications and implementation plans
- `src/`: Main application code
- `tests/`: Test suites (unit, integration, contract)
- `benches/`: Performance benchmarks
- `docs/`: Documentation (architecture, guides)

## 📚 Documentation

- **[Architecture Guide](docs/ARCHITECTURE.md)**: System architecture and design patterns
- **[Custom Prompt Guide](docs/CUSTOM_PROMPT.md)**: How to customize your command prompt
- **[Specification](specs/001-mosaicterm-terminal-emulator/spec.md)**: Feature requirements and design
- **[Implementation Plan](specs/001-mosaicterm-terminal-emulator/plan.md)**: Technical architecture
- **[Tasks](specs/001-mosaicterm-terminal-emulator/tasks.md)**: Development roadmap
- **[Contracts](specs/001-mosaicterm-terminal-emulator/contracts/)**: API specifications

## 🐛 Issues & Support

Found a bug or have a feature request? Please [open an issue](https://github.com/djdanielsson/mosaicterm/issues) or start a [discussion](https://github.com/djdanielsson/mosaicterm/discussions).

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Warp**: Inspiration for the block-based terminal interface
- **Alacritty**: Terminal emulation techniques and ANSI handling
- **WezTerm**: Cross-platform PTY management patterns
- **egui**: Modern immediate mode GUI framework

---

**MosaicTerm** - Redefining the terminal experience, one block at a time.
