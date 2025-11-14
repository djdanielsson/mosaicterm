# MosaicTerm

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/djdanielsson/mosaicterm/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml/badge.svg)](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml)

A modern GUI terminal emulator written in Rust, inspired by [Warp](https://warp.dev). MosaicTerm groups commands and their outputs into discrete, scrollable blocks while maintaining a permanently pinned input prompt at the bottom - creating a clean, organized terminal experience that feels native to your workflow.

![MosaicTerm Screenshot](https://github.com/djdanielsson/mosaicterm/blob/main/icon.png)

## ✨ Key Features

- **Block-Based History**: Commands and their outputs are grouped into discrete, scrollable blocks
- **Pinned Input Prompt**: Always-visible input field at the bottom for seamless command entry
- **Environment Support**: Full support for Python venv, nvm, conda, rbenv, rvm, direnv and other environment tools
- **Custom Prompts**: Fully customizable prompt format with variable substitution ($USER, $HOSTNAME, $PWD, etc.)
- **Tab Completion**: Intelligent command and path completion with popup UI (double-tab to activate)
- **Native ANSI Support**: Full color support for `ls`, `bat`, `fzf`, and other CLI tools
- **Zsh Integration**: Seamless support for zsh with Oh My Zsh, plugins, and completions
- **Modern GUI**: Built with [egui](https://github.com/emilk/egui) for native performance and feel
- **Cross-Platform Ready**: Designed for macOS, Linux, and Windows compatibility

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.90+ stable toolchain
- **macOS**: 14.0+ (fully supported) or **Linux** (Ubuntu 20.04+, Fedora 34+, Debian 11+, or similar)
- **Shell**: bash, zsh, or fish
- **Optional**: fzf, eza, bat, rg, fd, jq (for enhanced CLI experience)

### Installation

#### Option 1: Download Pre-built Release (Recommended)

**macOS:**
1. Download the latest `MosaicTerm-macos-{arm64|x64}.app.tar.gz` from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page
2. Extract: `tar xzf MosaicTerm-macos-*.app.tar.gz`
3. Move to Applications: `mv MosaicTerm.app /Applications/`
4. Launch from Applications folder or Spotlight

**Linux:**
1. Download the latest `mosaicterm-linux-x64.tar.gz` from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page
2. Extract: `tar xzf mosaicterm-linux-x64.tar.gz`
3. Move to PATH: `sudo mv mosaicterm /usr/local/bin/`
4. Run: `mosaicterm`

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
├── app.rs               # Main GUI application
├── pty/                 # Pseudoterminal management
│   ├── manager.rs       # PTY lifecycle
│   ├── process.rs       # Process spawning
│   └── streams.rs       # Async I/O handling
├── terminal/            # Terminal emulation
│   ├── ansi_parser.rs   # ANSI escape sequences
│   ├── state.rs         # Terminal state
│   └── prompt.rs        # Prompt detection
├── ui/                  # GUI components
│   ├── blocks.rs        # Command block rendering
│   ├── input.rs         # Input prompt component
│   └── scroll.rs        # Scrollable history
└── models/              # Data structures
    ├── command_block.rs # Command + output blocks
    └── terminal_session.rs # Session management
```

### Key Technologies

- **[egui/eframe](https://github.com/emilk/egui)**: Immediate mode GUI framework
- **[portable-pty](https://github.com/wez/wezterm/tree/main/crates/portable-pty)**: Cross-platform PTY support
- **[vte](https://github.com/alacritty/vte)**: Terminal emulation and ANSI parsing
- **[tokio](https://tokio.rs/)**: Async runtime for I/O operations

## 🔧 Configuration

MosaicTerm supports TOML-based configuration. Create `~/.config/mosaicterm/config.toml`:

```toml
[ui]
font_family = "JetBrains Mono"
font_size = 12

[key_bindings.bindings]
# Interrupt/kill running command (default: Ctrl+C)
interrupt = { key = "Ctrl+C", enabled = true }
# Copy text (default: Ctrl+Shift+C to avoid conflict with interrupt)
copy = { key = "Ctrl+Shift+C", enabled = true }
# Clear screen (default: Ctrl+L)
clear = { key = "Ctrl+L", enabled = true }
theme_name = "default-dark"

[terminal]
shell_type = "Bash"
shell_path = "/bin/bash"
# Customize your prompt with variables like $USER, $HOSTNAME, $PWD
prompt_format = "$USER@$HOSTNAME:$PWD$ "

[pty]
buffer_size = 1048576
```

### Custom Prompts

MosaicTerm supports fully customizable prompts with variable substitution (`$USER`, `$HOSTNAME`, `$PWD`, etc.). See the **[Custom Prompt Guide](docs/CUSTOM_PROMPT.md)** for details and examples.

### Environment Variables

- `MOSAICTERM_CONFIG`: Override default config path
- `MOSAICTERM_LOG`: Set logging level (`error`, `warn`, `info`, `debug`, `trace`)

## 🌍 Environment Management

MosaicTerm fully supports environment management tools like Python virtual environments, Node Version Manager (nvm), Conda, and more. Unlike some terminal emulators, MosaicTerm loads your shell RC files (`.bashrc`, `.zshrc`, etc.) by default, making these tools work seamlessly.

**Smart Prompt Integration**: MosaicTerm automatically detects and displays active environments in your prompt:
- **Virtual Environments**: Shows `(venv:myproject)` when Python venv, Conda, or similar is active
- **Git Repositories**: Shows `[main *]` with branch name and dirty status on the right side
- **Automatic Updates**: Prompt updates after each command to reflect your current environment

The active environment indicator appears only when relevant, keeping your prompt clean when no special environments are active.

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

## 🎯 Limitations

### Interactive Programs (TUI Applications)

MosaicTerm uses a **block-based command history** model that works great for standard CLI commands, but has limitations with full-screen interactive (TUI) applications like `vim`, `htop`, `nano`, `less`, `tmux`, etc.

**What happens**: These programs expect full terminal control and may display garbled output or escape sequences as text. MosaicTerm will show a warning when you attempt to run them.

**If you accidentally run one**: Right-click the command block and select "Kill Process", or press **Ctrl+C** to terminate it.

**Recommended alternatives**:
- **Text editing**: Use external editors (`open file.txt` on macOS, `xdg-open file.txt` on Linux)
- **File viewing**: Use `cat`, `bat`, `head`, `tail` instead of `less`/`more`
- **System monitoring**: Use `ps aux` instead of `htop`/`top`
- **Git operations**: Use `git log`, `git status`, `git diff` instead of `tig`/`gitui`

For full-screen interactive programs, use a traditional terminal emulator. Support for interactive programs is planned for a future release.

## 🗺️ Roadmap

### Phase 4: Application Launch & UI Polish (Current)
- [x] Basic application window and layout
- [x] PTY process spawning and I/O
- [x] Command execution and block rendering
- [x] ANSI color support
- [ ] Block-based history UI refinement
- [ ] Smooth scrolling and navigation
- [ ] Theme system and customization
- [ ] Performance optimizations

### Phase 5: Advanced Features
- [ ] Command history persistence
- [ ] Block re-run functionality
- [ ] Inline search and filtering
- [ ] Configuration hot-reload
- [ ] Export blocks to markdown
- [x] Cross-platform builds (Linux support complete)

### Future Goals
- [ ] Plugin system architecture
- [ ] Remote shell support (SSH)
- [ ] Multi-tab interface
- [ ] Customizable block actions

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
