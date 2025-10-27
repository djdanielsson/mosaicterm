# Interactive Programs and TUI Applications

## Overview

MosaicTerm uses a **block-based command history** model where each command and its output are grouped into discrete blocks. While this works great for standard CLI commands, it has limitations with **full-screen interactive (TUI) applications** like `vim`, `htop`, `nano`, etc.

## Why Interactive Programs Don't Work Well

Interactive TUI programs like vim, htop, and nano:
- Take over the entire terminal screen
- Use complex ANSI escape sequences for cursor positioning
- Expect to control the alternate screen buffer
- Need real-time keyboard input
- Don't output discrete "results" but continuously update the display

These behaviors conflict with MosaicTerm's block-based model where:
- Output is captured and displayed in fixed blocks
- The input prompt is always pinned at the bottom
- Commands have a clear start and end

## Detected Interactive Programs

MosaicTerm automatically detects and warns you when you attempt to run these programs:

### Text Editors
- `vim`, `vi`, `nvim` - Vi/Vim editors
- `emacs` - Emacs editor
- `nano`, `pico` - Nano editor

### System Monitors
- `htop` - Interactive process viewer
- `top` - Process monitor
- `atop`, `iotop` - Advanced monitors

### Pagers
- `less` - Terminal pager
- `more` - Basic pager
- `man` - Manual page viewer (uses less)

### Terminal Multiplexers
- `tmux` - Terminal multiplexer
- `screen` - Screen multiplexer

### File Managers
- `mc` - Midnight Commander
- `ranger` - Terminal file manager
- `nnn` - n³ file manager

### Other TUI Programs
- `tig`, `gitui` - Git TUIs
- `mutt`, `alpine` - Email clients
- `menuconfig` - Configuration menus
- `cmatrix`, `nethack` - Entertainment

## What Happens When You Run Interactive Programs

When you run an interactive program like `vim file.txt`:

1. ⚠️ **Warning Shown**: Status bar displays: "⚠️ 'vim' is an interactive program and may not work correctly in block mode"

2. **Visual Artifacts**: You may see:
   - Garbled output
   - Escape sequences displayed as text
   - Incorrect cursor positioning
   - Missing or duplicated content

3. **No Interaction**: You cannot:
   - Type into the program
   - Use arrow keys for navigation
   - See proper screen updates

## How to Kill a Stuck Interactive Program

If you accidentally run an interactive program:

### Method 1: Right-Click Menu (Recommended)
1. Right-click on the command block
2. Select **"Kill Process"** from the context menu
3. The process will be terminated and PTY state cleaned up

### Method 2: Keyboard Shortcut
1. Press **Ctrl+C** while the command is running
2. The process receives SIGINT and terminates
3. Terminal state is automatically reset

### Automatic Cleanup

After killing an interactive program, MosaicTerm automatically:
- ✅ Drains remaining output from the PTY buffer
- ✅ Sends terminal reset sequences
- ✅ Exits alternate screen buffer
- ✅ Resets cursor visibility
- ✅ Clears any lingering escape codes
- ✅ Sends a newline to get a fresh prompt

## Recommended Alternatives

Instead of running interactive programs in MosaicTerm, use these alternatives:

### For Text Editing
```bash
# Use external editor
open file.txt                    # macOS - opens in default editor
xdg-open file.txt               # Linux - opens in default editor

# Use non-interactive editors
echo "content" > file.txt
cat > file.txt << EOF
...content...
EOF

# Use quick inline edits
sed -i 's/old/new/' file.txt
```

### For Viewing Files
```bash
# Instead of less/more
cat file.txt
head file.txt
tail file.txt
bat file.txt                     # Better cat with syntax highlighting

# Instead of man pages
man ls | cat                     # Convert to plain output
tldr ls                          # Simplified man pages
```

### For System Monitoring
```bash
# Instead of htop/top
ps aux
ps aux | grep process_name

# Get snapshot info
top -l 1 -n 10                   # macOS: single iteration
top -b -n 1 | head -20          # Linux: batch mode
```

### For Git Operations
```bash
# Instead of tig/gitui
git log --oneline --graph --all
git log --pretty=format:"%h %s" -10
git status
git diff
```

### For File Management
```bash
# Instead of ranger/mc/nnn
ls -la
tree                            # Show directory tree
exa -la                         # Modern ls replacement
find . -name "*.txt"
```

## Advanced: Running Interactive Programs Externally

If you need to use interactive programs, open them in a separate terminal:

### macOS
```bash
# Open new Terminal window
open -a Terminal

# Open iTerm2 window
open -a iTerm

# Run command in new terminal
osascript -e 'tell application "Terminal" to do script "vim file.txt"'
```

### Linux
```bash
# Open new terminal window
gnome-terminal -- vim file.txt
xterm -e vim file.txt
konsole -e vim file.txt
```

## Technical Details

### Why Killing Doesn't Always Clean Up

When you kill an interactive program, it may leave the terminal in an inconsistent state because:
- The program might be in alternate screen buffer mode
- Cursor might be hidden
- Terminal attributes might be changed
- Raw mode might still be enabled

### Reset Sequences Sent

MosaicTerm sends these escape sequences after killing a process:

| Sequence | Purpose |
|----------|---------|
| `\x1b[0m` | Reset all text attributes (color, bold, etc.) |
| `\x1b[?25h` | Show cursor (might be hidden by program) |
| `\x1b[?1049l` | Exit alternate screen buffer |
| `\x1b[2J` | Clear screen |
| `\x1b[H` | Move cursor to home position |
| `\x0c` | Form feed (additional clear) |

### Buffer Draining

After killing, MosaicTerm:
1. Waits 100ms for the process to die
2. Reads up to 1000 bytes from the PTY buffer
3. Discards the output to prevent contamination

## Future Improvements

Planned enhancements for handling interactive programs:

- [ ] **Preview Mode**: Show warning dialog before running
- [ ] **Blocked Mode**: Option to prevent interactive programs entirely
- [ ] **External Terminal**: Automatically open interactive programs in external terminal
- [ ] **Better Detection**: Analyze program behavior to detect TUI usage
- [ ] **State Recovery**: Better terminal state restoration
- [ ] **Configurable List**: Let users add/remove programs from detection list

## Configuration

Currently, the list of interactive programs is hardcoded. Future versions will allow configuration:

```toml
# Future feature
[terminal]
interactive_programs = ["vim", "htop", "custom-tui-app"]
block_interactive = false  # Set to true to prevent running them
warn_interactive = true    # Show warning before running
```

## Related Documentation

- [Configuration Guide](../README.md#configuration)
- [Keyboard Shortcuts](./KEYBOARD_SHORTCUTS.md)
- [Troubleshooting](./TROUBLESHOOTING.md)

## Summary

**Do Not Use**: vim, nano, htop, top, less, more, man, tmux, screen, mc, ranger, etc.

**Use Instead**: cat, bat, tree, exa, ps, git (non-interactive commands)

**If Stuck**: Right-click → Kill Process, or press Ctrl+C

**Recovery**: Automatic cleanup happens after killing interactive programs

