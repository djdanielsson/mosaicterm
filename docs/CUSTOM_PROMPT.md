# Custom Prompt Guide

MosaicTerm supports six prompt styles with full variable substitution and custom segment definitions.

---

## Prompt Styles

Set the style in `~/.config/mosaicterm/config.toml`:

```toml
[prompt]
style = "minimal"  # classic | minimal | powerline | starship | ohmyzsh | custom
show_git = true
show_env = true
```

| Style | Appearance |
|-------|-----------|
| `classic` | `(venv:myproject) user@host:~/workspace$ [main +2 !1]` |
| `minimal` | `(myproject) ~/workspace main +2 !1 >` (default) |
| `powerline` | Colored segments with Powerline arrow () separators |
| `starship` | Colored text with emoji icons per context type |
| `ohmyzsh` | `(myproject) user@host ~/workspace (main*) >` |
| `custom` | User-defined segments (see below) |

## Template Variables

Available in `prompt_format` (used by classic/minimal) and custom segment `content` fields:

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

## Basic Prompt Format

The `prompt_format` string is used by **Classic** and **Minimal** styles for the main prompt text:

```toml
[terminal]
prompt_format = "$USER@$HOSTNAME:$PWD$ "
```

### Examples

```toml
# Standard Unix
prompt_format = "$USER@$HOSTNAME:$PWD$ "
# Output: ddaniels@macbook-pro:~/workspace$

# Minimalist
prompt_format = "$PWD > "
# Output: ~/workspace >

# Multi-line
prompt_format = "$USER@$HOSTNAME [$PWD]\n$ "
# Output:
# ddaniels@macbook-pro [~/workspace]
# $

# With emoji
prompt_format = "🚀 $PWD ▸ "
# Output: 🚀 ~/workspace ▸

# Simple dollar sign
prompt_format = "$ "
# Output: $
```

## Custom Segments

For full control, use `style = "custom"` with segment definitions. Each segment has:

| Field | Type | Description |
|-------|------|-------------|
| `content` | string | Text with variable substitution |
| `fg` | string (optional) | Foreground color as `"#RRGGBB"` |
| `bg` | string (optional) | Background color as `"#RRGGBB"` |
| `bold` | bool (optional) | Bold text, default `false` |
| `condition` | string (optional) | Only show when condition is met |

### Conditions

| Condition | Shown when |
|-----------|------------|
| `"git"` | Inside a git repository |
| `"venv"` | Python venv is active |
| `"conda"` | Conda env is active |
| `"node"` | nvm Node.js is active |
| `"docker"` | Docker context is set |

If a segment has no condition, it always shows.

### Example: Developer Prompt

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
content = "($GIT_BRANCH)"
fg = "#C8C8FF"
condition = "git"

[[prompt.segments]]
content = "($VENV)"
fg = "#FFC864"
condition = "venv"

[[prompt.segments]]
content = "> "
fg = "#64DC64"
bold = true
```

### Example: Minimal Git-Aware

```toml
[prompt]
style = "custom"

[[prompt.segments]]
content = "$PWD"
fg = "#50B4FF"
bold = true

[[prompt.segments]]
content = "$GIT_BRANCH"
fg = "#96DC96"
condition = "git"

[[prompt.segments]]
content = "❯ "
fg = "#64DC64"
```

### Example: Powerline-Style Custom

```toml
[prompt]
style = "custom"

[[prompt.segments]]
content = " $USER "
fg = "#FFFFFF"
bg = "#3C3C64"
bold = true

[[prompt.segments]]
content = " $PWD "
fg = "#FFFFFF"
bg = "#2472C8"
bold = true

[[prompt.segments]]
content = " $GIT_BRANCH "
fg = "#FFFFFF"
bg = "#288C3C"
condition = "git"

[[prompt.segments]]
content = " "
fg = "#B4B4C8"
```

## Style Details

### Classic

Renders: `(envs) user@host:path$ [git]`

- Environment contexts shown in yellow `(name:value)` before the main prompt
- Main prompt in green, bold
- Git status in light purple brackets after

### Minimal

Renders: `(envs) path git >`

- Environment contexts shown in yellow `(value)`
- Path in blue, bold
- Git in green
- Trailing `>` in gray

### Powerline

Renders: ` env ` ` path ` ` git `

- Colored background segments with Powerline arrow () separators
- White text on colored backgrounds
- Git segment: green background for clean, orange for dirty
- Requires a Nerd Font for arrow glyphs

### Starship

Renders: `path git  env1  env2 ❯`

- Path in blue, bold
- Git in green (clean) or red (dirty)
- Environment contexts with emoji icons (🐍 Python, 🦀 Rust, 🐳 Docker, etc.)
- Trailing `❯` in green

### OhMyZsh

Renders: `(envs) user@host path (branch*) >`

- Environments in yellow
- User@host in cyan, bold
- Path in blue, bold
- Git branch with `*` for dirty repos
- Trailing `>` in green, bold

## Font Recommendations

For the best experience with all prompt styles:

- **JetBrains Mono** (default) -- excellent monospace font
- **JetBrainsMono Nerd Font** -- includes Powerline arrows and icons
- **Fira Code** -- ligature support
- **Cascadia Code** -- modern Microsoft font

Powerline styles require a font with the Powerline arrow glyph (). Install any [Nerd Font](https://www.nerdfonts.com/) and set it in your config:

```toml
[ui]
font_family = "JetBrainsMono Nerd Font"
```

## Dynamic Updates

The prompt automatically updates when you:
- Change directories (`cd`, `pushd`, `popd`, `z`)
- Activate/deactivate environments
- Switch git branches

## Related Documentation

- [Configuration Reference](CONFIGURATION.md) -- all config options
- [Theming Guide](THEMING.md) -- color customization
