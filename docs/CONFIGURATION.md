# Configuration Reference

Complete reference for all MosaicTerm configuration options.

---

## Config File Location

MosaicTerm searches for config files in this order:

1. `$MOSAICTERM_CONFIG` (environment variable override)
2. `$XDG_CONFIG_HOME/mosaicterm/config.toml` (typically `~/.config/mosaicterm/config.toml`)
3. `$XDG_CONFIG_HOME/mosaicterm.toml`
4. `./mosaicterm.toml` (current directory)
5. `./.mosaicterm.toml` (current directory, hidden)

Supported formats: TOML (default), JSON.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `MOSAICTERM_CONFIG` | Override config file path |
| `MOSAICTERM_LOG` | Set log level: `error`, `warn`, `info`, `debug`, `trace` |

---

## Full Example Config

Below is a complete config file showing every option with its default value.

```toml
# ─── UI Settings ─────────────────────────────────────────────

[ui]
# Font family for terminal text. MosaicTerm searches OS font directories.
# Install a Nerd Font for Powerline/icon support.
font_family = "JetBrains Mono"

# Font size in points
font_size = 12

# Maximum lines kept in scrollback history
scrollback_lines = 100000

# Built-in theme preset: "default-dark" | "default-light" | "high-contrast"
theme_name = "default-dark"

# Enable smooth scrolling
smooth_scrolling = true

# Animation duration in milliseconds
animation_duration_ms = 200

# Show line numbers in output
show_line_numbers = false

# Wrap long lines
word_wrap = true

# Custom theme color overrides (see THEMING.md for all options)
# [ui.theme]
# background = "#1A1A25"
# foreground = "#E5E5E5"
# accent = "#6496FF"

# ─── Terminal Settings ───────────────────────────────────────

[terminal]
# Shell type: "Bash" | "Zsh" | "Fish" | "PowerShell" | "Cmd"
shell_type = "Bash"

# Path to shell executable (auto-detected from $SHELL if not set)
shell_path = "/bin/bash"

# Arguments passed to shell on startup
shell_args = ["--login", "-i"]

# Starting working directory (defaults to $HOME)
# working_directory = "/path/to/dir"

# Terminal dimensions (columns, rows)
dimensions = [120, 30]

# Enable mouse support
mouse_support = true

# Scrollback buffer size (in lines)
scrollback_buffer = 1000000

# Bell style: "None" | "Sound" | "Visual"
bell_style = "Sound"

# Prompt format string for Classic/Minimal styles.
# Variables: $USER, $HOSTNAME, $PWD, $HOME, $SHELL
prompt_format = "$USER@$HOSTNAME:$PWD$ "

# ─── Command Timeout ────────────────────────────────────────

[terminal.timeout]
# Timeout for regular commands in seconds (0 = disabled)
regular_command_timeout_secs = 0

# Timeout for interactive commands in seconds (0 = disabled)
interactive_command_timeout_secs = 0

# Kill commands that exceed timeout (false = just mark completed)
kill_on_timeout = false

# Grace period in seconds before force-kill (only if kill_on_timeout = true)
kill_grace_period_secs = 5

# ─── PTY Settings ────────────────────────────────────────────

[pty]
# Additional environment variables for the PTY process
# [pty.environment]
# MY_VAR = "my_value"

# Inherit environment variables from parent process
inherit_env = true

# PTY I/O buffer size in bytes
buffer_size = 262144  # 256KB

# Enable raw mode for PTY
raw_mode = true

# Timeout for PTY operations in milliseconds
timeout_ms = 10

# ─── Prompt Settings ────────────────────────────────────────

[prompt]
# Prompt style: "classic" | "minimal" | "powerline" | "starship" | "ohmyzsh" | "custom"
style = "minimal"

# Template format string (used by classic and minimal styles)
format = "$PWD > "

# Show git branch/status in prompt
show_git = true

# Show environment contexts (venv, nvm, etc.) in prompt
show_env = true

# Custom prompt segments (only used when style = "custom")
# See CUSTOM_PROMPT.md for details.
#
# [[prompt.segments]]
# content = "$USER@$HOSTNAME"
# fg = "#00D2D2"
# bold = true
#
# [[prompt.segments]]
# content = "$PWD"
# fg = "#50B4FF"
# bold = true
#
# [[prompt.segments]]
# content = "($GIT_BRANCH)"
# fg = "#C8C8FF"
# condition = "git"
#
# [[prompt.segments]]
# content = "> "
# fg = "#64DC64"
# bold = true

# ─── Input Settings ──────────────────────────────────────────

[input]
# Maximum number of commands kept in the input history (navigable with up/down arrows)
max_history = 100

# ─── Session Persistence ────────────────────────────────────

[session]
# Enable tmux-backed session persistence (requires tmux installed)
persistence = false

# Automatically restore previous session on startup
auto_restore = false

# ─── Key Bindings ────────────────────────────────────────────

[key_bindings.bindings.interrupt]
key = "Ctrl+C"
enabled = true

[key_bindings.bindings.copy]
key = "Ctrl+Shift+C"
enabled = true

[key_bindings.bindings.paste]
key = "Ctrl+V"
enabled = true

[key_bindings.bindings.new_tab]
key = "Ctrl+T"
enabled = true

[key_bindings.bindings.close_tab]
key = "Ctrl+W"
enabled = true

[key_bindings.bindings.next_tab]
key = "Ctrl+Tab"
enabled = true

[key_bindings.bindings.prev_tab]
key = "Ctrl+Shift+Tab"
enabled = true

[key_bindings.bindings.clear]
key = "Ctrl+L"
enabled = true

[key_bindings.bindings.quit]
key = "Ctrl+Q"
enabled = true

# ─── TUI App Detection ──────────────────────────────────────

[tui_apps]
# Commands that open in fullscreen TUI overlay.
# Defaults include: vim, nvim, vi, nano, emacs, helix, micro,
# top, htop, btop, gotop, ytop, atop, less, more, man, tmux,
# screen, ranger, nnn, mc, vifm, ncdu, cmus, weechat, irssi,
# mutt, ncmpcpp.
#
# Add your own TUI apps (these are MERGED with defaults, not replacing):
# fullscreen_commands = ["my-tui-app", "another-app"]

# ─── Theme Color Overrides ──────────────────────────────────
# See THEMING.md for complete color customization.
#
# [ui.theme.ansi]
# black = "#000000"
# red = "#CD3131"
# ...
#
# [ui.theme.blocks]
# background = "#191923B4"
# border = "#2D2D41"
# ...
#
# [ui.theme.input]
# background = "#191923"
# text = "#FFFFFF"
# ...
#
# [ui.theme.status_bar]
# background = "#23232D"
# text = "#C8C8C8"
# ...
```

## Prompt Template Variables

Available in `prompt_format` and custom segment `content` fields:

| Variable | Description | Example |
|----------|-------------|---------|
| `$USER` | Current username | `ddaniels` |
| `$HOSTNAME` | System hostname | `macbook-pro` |
| `$PWD` | Current directory (with `~` for home) | `~/workspace` |
| `$HOME` | Home directory path | `/Users/ddaniels` |
| `$SHELL` | Current shell path | `/bin/zsh` |
| `$GIT_BRANCH` | Git branch name | `main` |
| `$GIT_STATUS` | Git status summary | `main +2 !3 ?1` |
| `$VENV` | Python venv/conda name | `myproject` |
| `$NODE_VERSION` | Node.js version (nvm) | `18.20.0` |
| `$RUBY_VERSION` | Ruby version (rbenv/rvm) | `3.2.0` |
| `$DOCKER` | Docker context | `default` |
| `$KUBE` | Kubernetes context | `production` |

Use `$$` to escape (e.g., `$$USER` renders as literal `$USER`).

## Custom Segment Conditions

When using `style = "custom"`, segments support a `condition` field:

| Condition | Shown when |
|-----------|------------|
| `"git"` | Inside a git repository |
| `"venv"` | Python venv is active |
| `"conda"` | Conda env is active |
| `"node"` | nvm Node.js is active |
| `"docker"` | Docker context is set |

## Related Documentation

- [Theming Guide](THEMING.md) -- detailed color customization
- [Custom Prompt Guide](CUSTOM_PROMPT.md) -- prompt styles and segments
- [Quick Start](QUICKSTART.md) -- getting started
