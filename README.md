# MosaicTerm

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/djdanielsson/mosaicterm/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/djdanielsson/mosaicterm/actions/workflows/release.yml/badge.svg)](https://github.com/djdanielsson/mosaicterm/actions/workflows/release.yml)

A modern GUI terminal emulator written in Rust, inspired by [Warp](https://warp.dev). MosaicTerm groups commands and their outputs into discrete, scrollable blocks while maintaining a permanently pinned input prompt at the bottom - creating a clean, organized terminal experience that feels native to your workflow.

![MosaicTerm Screenshot](https://github.com/djdanielsson/mosaicterm/blob/main/icon.png)

## ✨ Key Features

- **Block-Based History**: Commands and their outputs are grouped into discrete, scrollable blocks
- **Pinned Input Prompt**: Always-visible input field at the bottom for seamless command entry
- **Native ANSI Support**: Full color support for `ls`, `bat`, `fzf`, and other CLI tools
- **Zsh Integration**: Seamless support for zsh with Oh My Zsh, plugins, and completions
- **Modern GUI**: Built with [egui](https://github.com/emilk/egui) for native performance and feel
- **Cross-Platform Ready**: Designed for macOS, Linux, and Windows compatibility

## 🚀 Quick Start

### Prerequisites

- **Rust**: 1.70+ stable toolchain
- **macOS**: 14.0+ (MVP platform)
- **Dependencies**: zsh, fzf, eza, bat, rg, fd, jq (Oh My Zsh recommended)

### Installation

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

## 📋 Requirements

### System Requirements
- **Operating System**: macOS 14.0+ (primary), Linux/Windows (planned)
- **Memory**: 200MB RAM minimum
- **Storage**: 50MB disk space

### CLI Tool Integration
MosaicTerm works best with modern CLI tools:
- **Shell**: zsh (with Oh My Zsh)
- **Search**: fzf, rg (ripgrep), fd
- **Display**: bat (syntax highlighting), eza (modern ls)
- **Processing**: jq (JSON), various development tools

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
[theme]
name = "dark"
background = "#1a1a1a"
foreground = "#ffffff"

[shell]
type = "zsh"
config_path = "~/.zshrc"

[ui]
font_size = 12
block_spacing = 8
```

### Environment Variables

- `MOSAICTERM_CONFIG`: Override default config path
- `MOSAICTERM_LOG`: Set logging level (`error`, `warn`, `info`, `debug`, `trace`)

## 🎯 Current Limitations

### MVP Scope (Phase 4 - Active Development)
- **Platform**: macOS 14+ only (Linux/Windows support planned)
- **Shell**: zsh with basic integration (advanced plugin features developing)
- **Features**: Core terminal functionality complete, advanced UI polish in progress
- **Performance**: Target <16ms frame time, <200MB memory usage

### Known Issues
- Some placeholder implementations in test suite
- Limited cross-platform testing
- Advanced UI features still in development (see roadmap)

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
- [ ] Cross-platform builds (Linux/Windows)

### Future Goals
- [ ] Plugin system architecture
- [ ] Remote shell support (SSH)
- [ ] Multi-tab interface
- [ ] Customizable block actions
- [ ] AI-assisted command suggestions

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

MosaicTerm uses GitHub Actions for automated releases. To create a new release:

1. **Update version** in `Cargo.toml`:
   ```toml
   version = "0.2.0"
   ```

2. **Create and push a version tag**:
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. **GitHub Actions will automatically**:
   - Build binaries for Linux, macOS, and Windows
   - Run the full test suite
   - Create a GitHub release with downloadable binaries
   - Generate release notes

Alternatively, you can trigger a release manually from the [Actions tab](https://github.com/djdanielsson/mosaicterm/actions) by running the "Release" workflow.

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

1. Fork and clone the repository
2. Install Rust 1.70+ stable toolchain
3. Run `cargo test` to ensure everything works
4. Create a feature branch: `git checkout -b feature/your-feature`
5. Make your changes with tests
6. Run the full test suite: `cargo test --all-features`
7. Submit a pull request

### Project Structure

- `specs/`: Feature specifications and implementation plans
- `src/`: Main application code
- `tests/`: Test suites (unit, integration, contract)
- `benches/`: Performance benchmarks
- `docs/`: Documentation (planned)

## 📚 Documentation

- **[Specification](specs/001-mosaicterm-terminal-emulator/spec.md)**: Feature requirements and design
- **[Implementation Plan](specs/001-mosaicterm-terminal-emulator/plan.md)**: Technical architecture
- **[Tasks](specs/001-mosaicterm-terminal-emulator/tasks.md)**: Development roadmap
- **[Contracts](specs/001-mosaicterm-terminal-emulator/contracts/)**: API specifications

## 🐛 Issue Tracking

Found a bug or have a feature request? Please [open an issue](https://github.com/djdanielsson/mosaicterm/issues) with:

- Clear description of the issue
- Steps to reproduce (if applicable)
- Expected vs. actual behavior
- System information (OS, Rust version)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Warp**: Inspiration for the block-based terminal interface
- **Alacritty**: Terminal emulation techniques and ANSI handling
- **WezTerm**: Cross-platform PTY management patterns
- **egui**: Modern immediate mode GUI framework

## 📞 Contact

- **Issues**: [GitHub Issues](https://github.com/djdanielsson/mosaicterm/issues)
- **Discussions**: [GitHub Discussions](https://github.com/djdanielsson/mosaicterm/discussions)

---

**MosaicTerm** - Redefining the terminal experience, one block at a time.
