# Roadmap: Interactive Program Support

## Goal
Enable MosaicTerm to properly handle interactive TUI programs (vim, htop, nano, etc.) that require full terminal control.

## Current Status
- ✅ Detection of interactive programs
- ✅ Warning messages to users
- ✅ Comprehensive documentation
- ❌ No actual support for running them

## Implementation Phases

### Phase 1: External Terminal Integration (Recommended First Step)
**Goal**: Automatically open interactive programs in external terminal  
**Complexity**: Medium  
**Timeline**: 1-2 weeks

#### Features
- Detect interactive command (already done!)
- Show dialog: "This will open in external terminal"
- Launch platform-specific terminal with the command
- Track process completion
- Return focus to MosaicTerm when done

#### Implementation Tasks
1. **Platform detection** (`src/platform/mod.rs`)
   ```rust
   pub enum Platform {
       MacOS,
       Linux,
       Windows,
   }
   
   pub fn detect_platform() -> Platform
   pub fn get_terminal_command(platform: Platform) -> Vec<String>
   ```

2. **External launcher** (`src/external_terminal.rs`)
   ```rust
   pub struct ExternalTerminal {
       platform: Platform,
   }
   
   impl ExternalTerminal {
       pub fn launch_command(&self, cmd: &str, working_dir: &Path) -> Result<Child>
       pub fn available_terminals() -> Vec<String>
   }
   ```

3. **Platform-specific commands**:
   - **macOS**: 
     ```bash
     osascript -e 'tell app "Terminal" to do script "cd $(pwd) && vim file.txt"'
     # or iTerm2
     ```
   - **Linux**:
     ```bash
     gnome-terminal -- bash -c "cd $(pwd) && vim file.txt"
     x-terminal-emulator -e "vim file.txt"
     ```
   - **Windows**:
     ```bash
     start cmd /k "cd %cd% && vim file.txt"
     wt.exe -d . vim file.txt  # Windows Terminal
     ```

4. **User configuration** (`config.toml`):
   ```toml
   [terminal]
   external_terminal_app = "auto"  # or "iTerm", "gnome-terminal", etc.
   auto_launch_interactive = true
   ask_before_external_launch = true
   ```

5. **UI Flow**:
   - User types: `vim file.txt`
   - MosaicTerm shows modal: "Open vim in external terminal? [Yes] [No] [Always]"
   - If Yes: Launch and show status "vim running in external terminal..."
   - If No: Run in MosaicTerm (with artifacts warning)

#### Files to Create/Modify
- `src/platform/mod.rs` - Platform detection
- `src/external_terminal.rs` - External terminal launcher
- `src/ui/dialogs.rs` - Modal dialog for confirmation
- `src/app.rs` - Integration with command handler
- `src/config/mod.rs` - Configuration options

### Phase 2: Embedded Full-Screen Mode
**Goal**: Support interactive programs within MosaicTerm window  
**Complexity**: High  
**Timeline**: 4-6 weeks

#### Features
- Toggle between block-mode and terminal-mode
- Full VT100/ANSI sequence support
- Proper alternate screen buffer handling
- Raw input mode
- Detect program exit and return to block-mode

#### Architecture Changes Needed

1. **Dual rendering modes**:
   ```rust
   pub enum RenderMode {
       BlockBased,    // Current mode: discrete command blocks
       FullTerminal,  // New: full-screen terminal emulation
   }
   ```

2. **Terminal state machine**:
   ```rust
   pub struct TerminalRenderer {
       mode: RenderMode,
       screen_buffer: ScreenBuffer,
       alternate_buffer: Option<ScreenBuffer>,
       cursor: CursorState,
   }
   ```

3. **Input routing**:
   ```rust
   match self.render_mode {
       RenderMode::BlockBased => {
           // Current behavior: command prompt at bottom
           self.render_input_prompt(ui);
       }
       RenderMode::FullTerminal => {
           // New: send ALL input directly to PTY
           self.route_raw_input_to_pty(ui);
       }
   }
   ```

4. **Screen buffer management**:
   - Maintain a 2D grid of cells (like alacritty/wezterm)
   - Each cell: character + attributes (color, bold, etc.)
   - Handle cursor positioning: `\x1b[row;colH`
   - Handle alternate screen: `\x1b[?1049h/l`

5. **Detection of exit**:
   - Process termination
   - Alternate screen buffer exit sequence
   - User presses Ctrl+C or Esc

#### Implementation Tasks

1. **VT100 parser** (use existing crate like `vte`)
   ```toml
   [dependencies]
   vte = "0.13"  # Already in Cargo.toml!
   ```

2. **Screen buffer** (`src/terminal/screen_buffer.rs`):
   ```rust
   pub struct ScreenBuffer {
       cells: Vec<Vec<Cell>>,
       cursor_x: usize,
       cursor_y: usize,
       width: usize,
       height: usize,
   }
   
   pub struct Cell {
       char: char,
       fg_color: Color,
       bg_color: Color,
       attrs: CellAttributes,
   }
   ```

3. **ANSI sequence handler**:
   ```rust
   impl vte::Perform for TerminalRenderer {
       fn print(&mut self, c: char) {
           // Add character at cursor position
       }
       
       fn execute(&mut self, byte: u8) {
           // Handle control characters (newline, tab, etc.)
       }
       
       fn csi_dispatch(&mut self, params: &[i64], _: &[u8], _: bool, c: char) {
           // Handle CSI sequences (cursor movement, colors, etc.)
       }
   }
   ```

4. **Mode switching**:
   ```rust
   fn handle_command_input(&mut self, command: String) {
       if self.is_interactive_command(&command) {
           // Switch to full-terminal mode
           self.enter_fullscreen_mode();
           self.execute_in_terminal_mode(command);
       } else {
           // Stay in block mode
           self.execute_in_block_mode(command);
       }
   }
   ```

5. **Rendering**:
   ```rust
   fn render_fullscreen_terminal(&mut self, ui: &mut egui::Ui) {
       // Render the screen buffer as a grid of monospace text
       egui::Grid::new("terminal_grid")
           .spacing([0.0, 0.0])
           .show(ui, |ui| {
               for row in &self.screen_buffer.cells {
                   for cell in row {
                       ui.label(
                           egui::RichText::new(cell.char.to_string())
                               .color(cell.fg_color)
                               .background_color(cell.bg_color)
                       );
                   }
                   ui.end_row();
               }
           });
   }
   ```

#### Files to Create/Modify
- `src/terminal/screen_buffer.rs` - 2D grid for terminal
- `src/terminal/vt_handler.rs` - VT100 sequence processing
- `src/ui/terminal_view.rs` - Full-screen terminal renderer
- `src/app.rs` - Mode switching logic
- Modify existing PTY code to work with both modes

### Phase 3: Tab System with Mixed Modes
**Goal**: Support multiple tabs, each with different modes  
**Complexity**: Very High  
**Timeline**: 8-12 weeks

#### Features
- Tab bar at top
- Each tab can be block-mode OR terminal-mode
- Switch between tabs with keyboard shortcuts
- Persistent tabs (survive restart)
- Split panes within tabs

#### Architecture
```rust
pub struct Tab {
    id: TabId,
    title: String,
    mode: TabMode,
    content: TabContent,
}

pub enum TabMode {
    BlockBased(BlockState),
    Terminal(TerminalState),
}

pub struct TabManager {
    tabs: Vec<Tab>,
    active_tab: TabId,
}
```

#### Implementation Tasks
1. Tab bar UI component
2. Tab state management
3. Per-tab PTY/session
4. Tab switching and shortcuts
5. Tab persistence
6. Split pane support (optional)

#### Files to Create
- `src/tabs/mod.rs` - Tab management
- `src/ui/tab_bar.rs` - Tab bar UI
- `src/ui/split_pane.rs` - Split pane support
- Major refactoring of `src/app.rs`

## Technical Considerations

### Performance
- Screen buffer updates can be expensive
- Need efficient dirty region tracking
- Consider using GPU rendering for large terminals

### Compatibility
- Must handle ALL VT100/ANSI sequences vim uses
- Test with: vim, neovim, nano, htop, less, man
- Handle edge cases: resizing, colors, unicode

### User Experience
- Clear visual indication of which mode you're in
- Easy way to exit full-screen mode (Esc, Ctrl+Z)
- Preserve command history when switching modes
- Good error messages

## Dependencies

### New Crates Needed
```toml
[dependencies]
vte = "0.13"              # Already have! VT100 parser
unicode-width = "0.1"      # Character width calculations
termwiz = "0.20"           # Terminal capabilities (optional)
```

### Existing Crates to Leverage
- `egui` - Already using for UI
- `portable-pty` - Already using for PTY
- `tokio` - Already using for async

## Testing Strategy

### Phase 1 Testing
1. Test external terminal launch on macOS
2. Test external terminal launch on Linux
3. Test external terminal launch on Windows
4. Verify working directory is preserved
5. Verify environment variables passed correctly

### Phase 2 Testing
1. Unit tests for screen buffer operations
2. Unit tests for VT100 sequence parsing
3. Integration tests with actual vim commands
4. Test alternate screen buffer switching
5. Test cursor positioning
6. Test colors and attributes
7. Test resizing
8. Test with: vim, neovim, nano, htop, less, man, tmux

### Phase 3 Testing
1. Test tab creation/deletion
2. Test tab switching
3. Test tab persistence
4. Test mixed-mode tabs (block + terminal)
5. Performance tests with many tabs

## Example User Flows

### Phase 1 (External Terminal)
```
User: vim file.txt
MosaicTerm: [Dialog] "vim requires full terminal. Open in external terminal?"
User: [Clicks Yes]
MosaicTerm: [Shows status] "vim running in Terminal.app..."
[Terminal.app opens with vim]
User: [Edits, saves, quits vim]
[Terminal.app closes]
MosaicTerm: [Status update] "vim completed"
```

### Phase 2 (Embedded Full-Screen)
```
User: vim file.txt
MosaicTerm: [Switches to full-screen mode]
[Entire window now shows vim interface]
[Input prompt hidden, all input goes to vim]
User: [Edits, saves, quits with :wq]
MosaicTerm: [Returns to block mode]
[Shows command block: "vim file.txt" with exit status]
```

### Phase 3 (Tabs)
```
User: [Working in block-mode tab]
User: vim file.txt
MosaicTerm: [Opens new terminal-mode tab]
[Tab bar shows: "Block Mode" | "vim file.txt" (active)]
User: [Edits in vim]
User: [Clicks "Block Mode" tab]
MosaicTerm: [Switches back to block-mode tab]
[vim still running in background tab]
User: [Can switch back to vim tab any time]
```

## Configuration File Examples

### For Phase 1
```toml
[terminal]
# External terminal for interactive programs
external_terminal = "auto"  # auto, iTerm, Terminal, gnome-terminal, konsole, alacritty
auto_launch_interactive = true
ask_before_launch = true
interactive_programs = ["vim", "vi", "nvim", "nano", "htop", "top"]
```

### For Phase 2
```toml
[terminal]
# Embedded full-screen mode
support_fullscreen_mode = true
fullscreen_key_exit = "Escape"
auto_detect_fullscreen_programs = true
```

### For Phase 3
```toml
[tabs]
max_tabs = 10
default_tab_mode = "block"
tab_switch_keys = ["Ctrl+Tab", "Ctrl+Shift+Tab"]
persist_tabs = true
allow_tab_splits = true
```

## Next Steps

1. **Decide on approach**: Which phase to implement first?
2. **Create feature branch**: `feature/interactive-program-support`
3. **Set up project board**: Track tasks and progress
4. **Start with Phase 1**: External terminal integration (easiest win)
5. **Iterate based on feedback**

## Open Questions

1. Should we default to external terminal or embedded mode?
2. How do we handle program detection for less common TUI apps?
3. Should we support tmux/screen nested in MosaicTerm?
4. What's the UX for returning from full-screen to block mode?
5. Should we persist vim sessions across MosaicTerm restarts?

## Resources

- [VTE crate](https://docs.rs/vte/) - VT100 parser
- [Alacritty source](https://github.com/alacritty/alacritty) - Reference terminal emulator
- [VT100 escape codes](https://vt100.net/docs/vt100-ug/chapter3.html)
- [ANSI escape sequences](https://en.wikipedia.org/wiki/ANSI_escape_code)
- [Terminal state machine](https://www.leonerd.org.uk/hacks/hints/xterm-8bit.html)

## Success Criteria

### Phase 1 Success
- ✅ Interactive programs open in external terminal
- ✅ Working directory preserved
- ✅ No crashes or errors
- ✅ Works on macOS, Linux, and Windows
- ✅ User can configure preferred terminal

### Phase 2 Success
- ✅ vim works perfectly within MosaicTerm window
- ✅ Colors, cursor, alternate screen all working
- ✅ Can edit files and save successfully
- ✅ Smooth transition between block and terminal modes
- ✅ No visual artifacts or escape sequence leaks

### Phase 3 Success
- ✅ Multiple tabs work simultaneously
- ✅ Can have block-mode and terminal-mode tabs
- ✅ Tabs persist across restarts
- ✅ Performance remains good with 5+ tabs
- ✅ Intuitive UX for managing tabs

