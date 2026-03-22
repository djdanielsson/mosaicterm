# Custom Prompt Configuration

MosaicTerm allows you to fully customize your command prompt using a flexible template system with variable substitution.

## Configuration Location

The configuration file is located at:
- **Linux/macOS**: `~/.config/mosaicterm/config.toml`
- **Windows**: `%APPDATA%\mosaicterm\config.toml`

## Prompt Format Setting

MosaicTerm supports two configuration approaches:

### Basic: Template String

In your `config.toml`, set `prompt_format` in the `[terminal]` section (used by Classic and Minimal styles):

```toml
[terminal]
prompt_format = "$USER@$HOSTNAME:$PWD$ "
```

### Advanced: Prompt Style

Use the `[prompt]` section for styled prompts with colored segments:

```toml
[prompt]
style = "ohmyzsh"  # classic | minimal | powerline | starship | ohmyzsh | custom
show_git = true
show_env = true
```

See [Built-in Styles](#built-in-styles) below for details on each style.

## Supported Variables

The following variables are available for use in `prompt_format` and custom segments:

| Variable | Description | Example |
|----------|-------------|---------|
| `$USER` | Current username | `ddaniels` |
| `$HOSTNAME` | System hostname | `macbook-pro` |
| `$PWD` | Current working directory (with `~` for home) | `~/workspace` or `/usr/local` |
| `$HOME` | Home directory path | `/Users/ddaniels` |
| `$SHELL` | Current shell path | `/bin/zsh` |
| `$GIT_BRANCH` | Current git branch name | `main` |
| `$GIT_STATUS` | Git status indicators | `+2 !3 ?1` |
| `$VENV` | Active Python virtual environment | `myproject` |
| `$NODE_VERSION` | Active Node.js version (via nvm) | `18.20.0` |
| `$RUBY_VERSION` | Active Ruby version (via rbenv/rvm) | `3.2.0` |
| `$DOCKER` | Docker context (if active) | `default` |
| `$KUBE` | Kubernetes context (if active) | `production` |

## Example Configurations

### 1. Standard Unix Prompt
```toml
prompt_format = "$USER@$HOSTNAME:$PWD$ "
```
**Output**: `ddaniels@macbook-pro:~/workspace$ `

### 2. Minimalist
```toml
prompt_format = "$PWD > "
```
**Output**: `~/workspace > `

### 3. Bracketed User with Path
```toml
prompt_format = "[$USER] $PWD > "
```
**Output**: `[ddaniels] ~/workspace > `

### 4. Multi-line Prompt
```toml
prompt_format = "$USER@$HOSTNAME [$PWD]\n$ "
```
**Output**:
```
ddaniels@macbook-pro [~/workspace]
$
```

### 5. With Emojis 🎨
```toml
prompt_format = "🚀 $PWD ▸ "
```
**Output**: `🚀 ~/workspace ▸ `

### 6. Git-style with Arrow
```toml
prompt_format = "$USER@$HOSTNAME [$PWD]\n❯ "
```
**Output**:
```
ddaniels@macbook-pro [~/workspace]
❯
```

### 7. Simple Dollar Sign
```toml
prompt_format = "$ "
```
**Output**: `$ `

### 8. Powerline-inspired
```toml
prompt_format = " $USER  $PWD  "
```
**Output**: ` ddaniels  ~/workspace  `

## Escaping Variables

If you want to display a literal `$VAR` in your prompt, use double dollar signs:

```toml
prompt_format = "Price: $$100 | User: $USER$ "
```
**Output**: `Price: $100 | User: ddaniels$ `

## Tips and Best Practices

1. **Keep it Readable**: Your prompt should be easy to read at a glance. Don't overcomplicate it.

2. **End with a Space**: Most prompts should end with a space to separate the prompt from your command input:
   ```toml
   prompt_format = "$PWD$ "  # Note the space after $
   ```

3. **Test Your Prompt**: After changing your prompt, restart MosaicTerm to see the changes.

4. **Path Abbreviation**: The `$PWD` variable automatically abbreviates your home directory as `~`:
   - `/Users/ddaniels/workspace` becomes `~/workspace`
   - `/Users/ddaniels` becomes `~`

5. **Unicode Support**: MosaicTerm supports Unicode characters and emojis in prompts:
   - Arrows: `→`, `⇒`, `▸`, `❯`, `►`
   - Symbols: `⚡`, `🚀`, `⭐`, `✓`, `◆`
   - Brackets: `【`, `】`, `「`, `」`, `『`, `』`

## Dynamic Updates

The prompt automatically updates when you:
- Change directories with `cd`, `pushd`, or `popd`
- Start a new terminal session

## Troubleshooting

### Prompt Not Updating?
1. Check that your `config.toml` is in the correct location
2. Ensure the TOML syntax is valid (no missing quotes or brackets)
3. Restart MosaicTerm after making changes

### Variables Not Expanding?
- Make sure you're using `$` before the variable name (e.g., `$USER`, not `USER`)
- Check for typos in variable names (they are case-sensitive)

### Characters Not Displaying?
- Ensure your terminal font supports the characters you're using
- Try a font like "JetBrains Mono", "Fira Code", or "Cascadia Code" for best Unicode support

## Default Prompt

If no custom prompt is configured, MosaicTerm uses the **Minimal** style:
```
~/workspace >
```

This corresponds to `style = "minimal"` and `format = "$PWD > "`.

## Built-in Styles

| Style | Appearance |
|-------|-----------|
| `classic` | `user@host:~/workspace$` |
| `minimal` | `~/workspace >` (default) |
| `powerline` | Colored segments with arrow separators: ` user  ~/workspace  main ` |
| `starship` | Colored text segments with icons |
| `ohmyzsh` | `user@host ~/workspace (main*) > ` |
| `custom` | User-defined segments (see README) |

The **ohmyzsh** style renders: `user@host` (cyan), space, `pwd` (blue), git branch with `*` for dirty repos, and `> ` as the prompt character.

## Reloading Configuration

Currently, you need to restart MosaicTerm to apply prompt changes. Live configuration reloading is planned for a future release.

## Advanced Examples

### Context-Aware Prompt

Git and environment variables are already supported. Example with git context:

```toml
[prompt]
style = "custom"

[[prompt.segments]]
content = "$USER@$HOSTNAME"
fg = "#00D2D2"
bold = true

[[prompt.segments]]
content = " $PWD "
fg = "#50B4FF"
bold = true

[[prompt.segments]]
content = "($GIT_BRANCH)"
fg = "#C8C8FF"
condition = "git"

[[prompt.segments]]
content = " > "
fg = "#64DC64"
```

Future additions planned: `$EXIT_CODE`, `$TIME`.

## Related Documentation

- [Configuration Guide](../README.md#configuration)
- [Key Bindings](./KEY_BINDINGS.md)
- [Themes](./THEMES.md)
