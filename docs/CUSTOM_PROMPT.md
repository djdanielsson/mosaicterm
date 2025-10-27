# Custom Prompt Configuration

MosaicTerm allows you to fully customize your command prompt using a flexible template system with variable substitution.

## Configuration Location

The configuration file is located at:
- **Linux/macOS**: `~/.config/mosaicterm/config.toml`
- **Windows**: `%APPDATA%\mosaicterm\config.toml`

## Prompt Format Setting

In your `config.toml`, locate the `[terminal]` section and set the `prompt_format` field:

```toml
[terminal]
prompt_format = "$USER@$HOSTNAME:$PWD$ "
```

## Supported Variables

The following variables are available for use in your prompt:

| Variable | Description | Example |
|----------|-------------|---------|
| `$USER` | Current username | `ddaniels` |
| `$HOSTNAME` | System hostname | `macbook-pro` |
| `$PWD` | Current working directory (with `~` for home) | `~/workspace` or `/usr/local` |
| `$HOME` | Home directory path | `/Users/ddaniels` |
| `$SHELL` | Current shell path | `/bin/zsh` |

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

If no custom prompt is configured, MosaicTerm uses:
```toml
prompt_format = "$USER@$HOSTNAME:$PWD$ "
```

## Reloading Configuration

Currently, you need to restart MosaicTerm to apply prompt changes. Live configuration reloading is planned for a future release.

## Advanced Examples

### Context-Aware Prompt (Future Feature)
While not yet implemented, future versions will support:
- `$GIT_BRANCH` - Current git branch
- `$EXIT_CODE` - Last command exit code
- `$TIME` - Current time
- Color codes for different states

Stay tuned for updates!

## Related Documentation

- [Configuration Guide](../README.md#configuration)
- [Key Bindings](./KEY_BINDINGS.md)
- [Themes](./THEMES.md)

