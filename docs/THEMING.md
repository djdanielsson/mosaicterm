# Theming Guide

MosaicTerm has a comprehensive theming system with built-in themes, ANSI color scheme presets, and full custom theme support.

---

## Built-in Themes

Set the theme in your config file (`~/.config/mosaicterm/config.toml`):

```toml
[ui]
theme_name = "default-dark"
```

| Theme | Description |
|-------|-------------|
| `default-dark` | Modern dark theme optimized for long coding sessions (default) |
| `default-light` | Clean light theme for daytime use |
| `high-contrast` | High contrast theme for accessibility |

## Customizing Colors

Override individual colors in the `[ui.theme]` section. Colors accept hex strings (`"#RRGGBB"` or `"#RRGGBBAA"`) or `{r, g, b, a}` float structs.

### Base Colors

```toml
[ui.theme]
background = "#1A1A25"
foreground = "#E5E5E5"
accent = "#6496FF"
success = "#2ECC71"
error = "#F14C4C"
warning = "#F1C40F"
selection = "#264060"
```

### ANSI Terminal Colors

Override the 16 standard ANSI colors used for command output:

```toml
[ui.theme.ansi]
black = "#000000"
red = "#CD3131"
green = "#0DBC79"
yellow = "#E5E510"
blue = "#2472C8"
magenta = "#BC3FBC"
cyan = "#11A8CD"
white = "#E5E5E5"
bright_black = "#666666"
bright_red = "#F14C4C"
bright_green = "#23D18B"
bright_yellow = "#F5F543"
bright_blue = "#3B8EEA"
bright_magenta = "#D670D6"
bright_cyan = "#29B8DB"
bright_white = "#E5E5E5"
```

### Command Block Colors

Customize how command blocks appear:

```toml
[ui.theme.blocks]
background = "#191923B4"
border = "#2D2D41"
header_background = "#0F0F19C8"
command_text = "#C8C8FF"
output_text = "#B4B4C8"
timestamp = "#78788C"
prompt = "#9696AA"
status_running = "#FFC800"
status_completed = "#00FF64"
status_failed = "#FF6464"
status_cancelled = "#FFA500"
status_pending = "#969696"
status_tui = "#9664FF"
hover_border = "#3C3C50"
selected_border = "#6496FF"
```

### Input Field Colors

```toml
[ui.theme.input]
background = "#191923"
text = "#FFFFFF"
placeholder = "#78788C"
cursor = "#6496FF"
border = "#3C3C50"
focused_border = "#6496FF"
prompt = "#64C864"
```

### Status Bar Colors

```toml
[ui.theme.status_bar]
background = "#23232D"
text = "#C8C8C8"
path = "#96C8FF"
branch = "#C8C8FF"
environment = "#FFC864"
ssh_indicator = "#96FF96"
border = "#505064"
```

## ANSI Color Scheme Presets

The theme manager supports popular color scheme presets that can be applied programmatically. These modify the ANSI colors of the current theme:

- **Monokai** -- Classic Monokai Pro colors
- **Solarized Dark** -- Ethan Schoonover's Solarized (dark variant)
- **Solarized Light** -- Ethan Schoonover's Solarized (light variant)
- **Dracula** -- The popular Dracula color scheme
- **Nord** -- Arctic, north-bluish color palette

## Creating a Full Custom Theme

For complete control, you can define every aspect of a theme. Themes are structured as JSON and can be imported/exported through the `ThemeManager` API.

A theme consists of three major sections:

### Color Palette

```
colors:
  background:  primary, secondary, tertiary, hover, selected
  text:        primary, secondary, tertiary, muted, error, success, warning
  accent:      primary, secondary, tertiary, link, border
  status:      success, error, warning, info, running
  ansi_colors: black, red, green, yellow, blue, magenta, cyan, white
               + bright variants of each
```

### Typography

```
typography:
  terminal_font:  name, weight (100-900), style (Normal/Italic/Oblique)
  ui_font:        name, weight, style
  terminal_size:  float (default: 12.0)
  ui_size:        float (default: 14.0)
  heading_size:   float (default: 18.0)
  line_height:    float (default: 1.4)
```

Font weights: `Thin` (100), `ExtraLight` (200), `Light` (300), `Normal` (400), `Medium` (500), `SemiBold` (600), `Bold` (700), `ExtraBold` (800), `Black` (900).

### UI Styles

```
styles:
  border_radius:  float (default: 6.0)
  border_width:   float (default: 1.0)
  padding:        { top, right, bottom, left }
  spacing:        float (default: 8.0)
  shadow:         { color, offset_x, offset_y, blur, spread } (optional)
```

### Example: Solarized Dark Theme (JSON)

```json
{
  "name": "solarized-dark",
  "description": "Solarized Dark theme",
  "author": "Your Name",
  "version": "1.0.0",
  "colors": {
    "background": {
      "primary": { "r": 0.0, "g": 0.169, "b": 0.212, "a": 1.0 },
      "secondary": { "r": 0.027, "g": 0.212, "b": 0.259, "a": 1.0 },
      "tertiary": { "r": 0.345, "g": 0.431, "b": 0.459, "a": 1.0 },
      "hover": { "r": 0.396, "g": 0.482, "b": 0.514, "a": 1.0 },
      "selected": { "r": 0.514, "g": 0.580, "b": 0.588, "a": 1.0 }
    },
    "text": {
      "primary": { "r": 0.514, "g": 0.580, "b": 0.588, "a": 1.0 },
      "secondary": { "r": 0.396, "g": 0.482, "b": 0.514, "a": 1.0 },
      "tertiary": { "r": 0.345, "g": 0.431, "b": 0.459, "a": 1.0 },
      "muted": { "r": 0.345, "g": 0.431, "b": 0.459, "a": 1.0 },
      "error": { "r": 0.863, "g": 0.196, "b": 0.184, "a": 1.0 },
      "success": { "r": 0.522, "g": 0.600, "b": 0.0, "a": 1.0 },
      "warning": { "r": 0.710, "g": 0.537, "b": 0.0, "a": 1.0 }
    },
    "accent": {
      "primary": { "r": 0.149, "g": 0.545, "b": 0.824, "a": 1.0 },
      "secondary": { "r": 0.424, "g": 0.443, "b": 0.769, "a": 1.0 },
      "tertiary": { "r": 0.827, "g": 0.212, "b": 0.510, "a": 1.0 },
      "link": { "r": 0.149, "g": 0.545, "b": 0.824, "a": 1.0 },
      "border": { "r": 0.027, "g": 0.212, "b": 0.259, "a": 1.0 }
    },
    "status": {
      "success": { "r": 0.522, "g": 0.600, "b": 0.0, "a": 1.0 },
      "error": { "r": 0.863, "g": 0.196, "b": 0.184, "a": 1.0 },
      "warning": { "r": 0.710, "g": 0.537, "b": 0.0, "a": 1.0 },
      "info": { "r": 0.149, "g": 0.545, "b": 0.824, "a": 1.0 },
      "running": { "r": 0.424, "g": 0.443, "b": 0.769, "a": 1.0 }
    },
    "ansi_colors": {
      "black": { "r": 0.0, "g": 0.169, "b": 0.212, "a": 1.0 },
      "red": { "r": 0.863, "g": 0.196, "b": 0.184, "a": 1.0 },
      "green": { "r": 0.522, "g": 0.600, "b": 0.0, "a": 1.0 },
      "yellow": { "r": 0.710, "g": 0.537, "b": 0.0, "a": 1.0 },
      "blue": { "r": 0.149, "g": 0.545, "b": 0.824, "a": 1.0 },
      "magenta": { "r": 0.827, "g": 0.212, "b": 0.510, "a": 1.0 },
      "cyan": { "r": 0.165, "g": 0.631, "b": 0.596, "a": 1.0 },
      "white": { "r": 0.933, "g": 0.910, "b": 0.835, "a": 1.0 },
      "bright_black": { "r": 0.027, "g": 0.212, "b": 0.259, "a": 1.0 },
      "bright_red": { "r": 0.796, "g": 0.294, "b": 0.086, "a": 1.0 },
      "bright_green": { "r": 0.345, "g": 0.431, "b": 0.459, "a": 1.0 },
      "bright_yellow": { "r": 0.396, "g": 0.482, "b": 0.514, "a": 1.0 },
      "bright_blue": { "r": 0.514, "g": 0.580, "b": 0.588, "a": 1.0 },
      "bright_magenta": { "r": 0.424, "g": 0.443, "b": 0.769, "a": 1.0 },
      "bright_cyan": { "r": 0.576, "g": 0.631, "b": 0.631, "a": 1.0 },
      "bright_white": { "r": 0.992, "g": 0.965, "b": 0.890, "a": 1.0 }
    }
  },
  "typography": {
    "terminal_font": { "name": "JetBrains Mono", "weight": "Normal", "style": "Normal" },
    "ui_font": { "name": "Inter", "weight": "Normal", "style": "Normal" },
    "terminal_size": 12.0,
    "ui_size": 14.0,
    "heading_size": 18.0,
    "line_height": 1.4
  },
  "styles": {
    "border_radius": 6.0,
    "border_width": 1.0,
    "padding": { "top": 8.0, "right": 12.0, "bottom": 8.0, "left": 12.0 },
    "spacing": 8.0,
    "shadow": { "color": { "r": 0.0, "g": 0.0, "b": 0.0, "a": 0.25 }, "offset_x": 0.0, "offset_y": 2.0, "blur": 8.0, "spread": 0.0 }
  }
}
```

## Tips

- **Font recommendation**: Install a [Nerd Font](https://www.nerdfonts.com/) (like "JetBrainsMono Nerd Font") for Powerline arrows and icons in prompt styles.
- **System theme detection**: `ThemeManager` supports `SystemTheme::Auto` which defaults to dark. Set `Light` or `Dark` explicitly if needed.
- Colors in the TOML config file use hex strings. Colors in JSON theme files use `{r, g, b, a}` float structs (0.0-1.0).
- Built-in themes (`default-*`) cannot be removed, but custom themes can be added and removed freely.

## Related Documentation

- [Configuration Reference](CONFIGURATION.md) -- all config options
- [Custom Prompt Guide](CUSTOM_PROMPT.md) -- prompt styles and segments
