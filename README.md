# MosaicTerm

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/djdanielsson/mosaicterm/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![CI](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml/badge.svg)](https://github.com/djdanielsson/mosaicterm/actions/workflows/ci.yml)

A modern terminal emulator that groups commands and their outputs into discrete, scrollable blocks. Built in Rust with [egui](https://github.com/emilk/egui).

![MosaicTerm Screenshot](https://github.com/djdanielsson/mosaicterm/blob/main/icon.png)

## Why MosaicTerm?

- **Block-based history** -- Each command and its output is a separate, scrollable block with status indicators
- **Always-visible prompt** -- Input stays pinned at the bottom, always ready
- **Works with your shell** -- zsh, bash, fish with full Oh My Zsh, plugin, and completion support
- **Smart environment detection** -- Automatically shows Python venv, nvm, Docker, Kubernetes, Rust, Go, and 20+ more contexts in your prompt
- **TUI app support** -- Run vim, htop, top, nano in a fullscreen overlay
- **Split panes** -- Native multi-pane support
- **Configurable prompts** -- 6 built-in styles (Classic, Minimal, Powerline, Starship, OhMyZsh, Custom) with full color control
- **Tool integrations** -- fzf (fuzzy completion), zoxide (smart cd), tmux (session persistence) detected automatically
- **Cross-platform** -- macOS, Linux, and Windows

## Quick Start

**Download** the latest release from [Releases](https://github.com/djdanielsson/mosaicterm/releases), or build from source:

```bash
git clone https://github.com/djdanielsson/mosaicterm.git
cd mosaicterm
cargo run --release
```

Requires Rust 1.90+. See the [Quick Start Guide](docs/QUICKSTART.md) for platform-specific install instructions.

## Configuration

Create `~/.config/mosaicterm/config.toml`:

```toml
[ui]
font_family = "JetBrainsMono Nerd Font"
font_size = 13
theme_name = "default-dark"

[prompt]
style = "ohmyzsh"
show_git = true
show_env = true
```

See the [Configuration Reference](docs/CONFIGURATION.md) for all options.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+R` | History search (fuzzy with fzf) |
| `Tab` (2x) | Completion popup |
| `Tab` / `Right` | Accept ghost completion |
| `Ctrl+Shift+D` | Split pane right |
| `Ctrl+Shift+E` | Split pane down |
| `Ctrl+Shift+W` | Close pane |
| `Ctrl+Shift+Arrows` | Navigate panes |
| `Double-Escape` | Exit TUI overlay |

## Documentation

| Guide | Description |
|-------|-------------|
| [Quick Start](docs/QUICKSTART.md) | Install and get running in minutes |
| [Configuration](docs/CONFIGURATION.md) | Every config option with defaults |
| [Custom Prompts](docs/CUSTOM_PROMPT.md) | Prompt styles, segments, and variables |
| [Theming](docs/THEMING.md) | Colors, built-in themes, custom themes |
| [Roadmap](docs/ROADMAP.md) | Release history and planned features |
| [Architecture](docs/ARCHITECTURE.md) | How MosaicTerm works internally |

## Linux Notes

- Uses XDG Base Directory for config (`~/.config/mosaicterm/config.toml`)
- Works with X11 and Wayland. If you hit buffer size errors on Wayland with fractional scaling, run with `WINIT_UNIX_BACKEND=x11 mosaicterm`
- Build dependencies: `sudo apt-get install build-essential libssl-dev pkg-config libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libgtk-3-dev`

## Development

```bash
cargo build              # Debug build
cargo test               # Run tests
cargo clippy             # Lint
cargo fmt                # Format
cargo run --release      # Release build + run
```

## Contributing

1. Fork and clone
2. `cargo test` to verify
3. Create a feature branch
4. Make changes with tests
5. Submit a pull request

## License

MIT -- see [LICENSE](LICENSE).

## Acknowledgments

Inspired by [Warp](https://warp.dev). Built with [egui](https://github.com/emilk/egui), [portable-pty](https://github.com/wez/wezterm/tree/main/crates/portable-pty), and [vte](https://github.com/alacritty/vte).
