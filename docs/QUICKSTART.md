# Quick Start Guide

Get MosaicTerm running in minutes.

---

## Install

### Pre-built Release (Recommended)

Download the latest release for your platform from the [Releases](https://github.com/djdanielsson/mosaicterm/releases) page.

**macOS:**

```bash
tar xzf MosaicTerm-macos-*.app.tar.gz
xattr -d com.apple.quarantine MosaicTerm.app
mv MosaicTerm.app /Applications/
```

**Linux:**

```bash
tar xzf mosaicterm-linux-*.tar.gz
sudo mv mosaicterm /usr/local/bin/
mosaicterm
```

**Windows:**

```powershell
Expand-Archive mosaicterm-windows-*.zip
# Run mosaicterm.exe directly or add to PATH
```

### Build from Source

Requires Rust 1.90+ stable.

```bash
git clone https://github.com/djdanielsson/mosaicterm.git
cd mosaicterm
cargo run --release
```

## First Launch

MosaicTerm starts your default shell automatically. The interface has two areas:

- **Top**: Scrollable command history -- each command and its output is a separate "block"
- **Bottom**: Pinned input prompt that's always visible

Type a command and press Enter. Output appears in a new block above.

## Optional Tools

These are detected automatically if installed -- no configuration needed:

| Tool | What it enables |
|------|----------------|
| **fzf** | Fuzzy matching for tab completion and Ctrl+R history search |
| **zoxide** | Smart `z` / `zi` directory jumping |
| **tmux** | Session persistence across restarts |

## Basic Configuration

Create `~/.config/mosaicterm/config.toml`:

```toml
[ui]
font_family = "JetBrainsMono Nerd Font"
font_size = 13
theme_name = "default-dark"  # default-dark | default-light | high-contrast

[terminal]
shell_path = "/bin/zsh"

[prompt]
style = "minimal"  # classic | minimal | powerline | starship | ohmyzsh | custom
show_git = true
show_env = true
```

See the [Configuration Reference](CONFIGURATION.md) for all options.

## Key Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+R` | Fuzzy history search (uses fzf if installed) |
| `Ctrl+L` | Clear screen |
| `Ctrl+C` | Interrupt current command |
| `Tab` (2x) | Open completion popup |
| `Tab` / `Right` | Accept ghost completion |
| `Ctrl+Shift+D` | Split pane right |
| `Ctrl+Shift+E` | Split pane down |
| `Ctrl+Shift+W` | Close pane |
| `Ctrl+Shift+Arrows` | Navigate panes |
| `Double-Escape` | Exit TUI overlay (vim, top, etc.) |

## TUI Apps

Run `vim`, `htop`, `top`, `nano`, and other interactive terminal apps -- they open in a fullscreen overlay. Use the app's normal exit command or double-press Escape.

## Environment Detection

MosaicTerm auto-detects active development contexts (Python venv, nvm, Docker, Kubernetes, Rust, Go, etc.) and shows them in your prompt. Project-specific contexts only appear inside relevant project directories.

## Next Steps

- [Custom Prompt Guide](CUSTOM_PROMPT.md) -- configure prompt styles and segments
- [Theming Guide](THEMING.md) -- customize colors and create themes
- [Configuration Reference](CONFIGURATION.md) -- every config option explained
- [Architecture](ARCHITECTURE.md) -- how MosaicTerm works internally
