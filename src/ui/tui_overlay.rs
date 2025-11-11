//! Fullscreen TUI application overlay
//!
//! This module provides a fullscreen overlay for running interactive TUI applications
//! like vim, htop, etc. The overlay captures all input and displays the raw PTY output.

use eframe::egui;

/// Simple virtual screen buffer for TUI apps
struct ScreenBuffer {
    /// 2D grid of characters (row, col)
    grid: Vec<Vec<char>>,
    /// Current cursor position
    cursor_row: usize,
    cursor_col: usize,
    /// Screen dimensions
    rows: usize,
    cols: usize,
}

impl ScreenBuffer {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            grid: vec![vec![' '; cols]; rows],
            cursor_row: 0,
            cursor_col: 0,
            rows,
            cols,
        }
    }

    fn clear(&mut self) {
        for row in &mut self.grid {
            row.fill(' ');
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn write_char(&mut self, ch: char) {
        if ch == '\n' {
            self.cursor_row = (self.cursor_row + 1).min(self.rows - 1);
            self.cursor_col = 0;
        } else if ch == '\r' {
            self.cursor_col = 0;
        } else if ch == '\t' {
            // Tab = 8 spaces
            for _ in 0..8 {
                self.write_char(' ');
            }
        } else if ch >= ' ' && self.cursor_row < self.rows && self.cursor_col < self.cols {
            self.grid[self.cursor_row][self.cursor_col] = ch;
            self.cursor_col += 1;
            if self.cursor_col >= self.cols {
                self.cursor_col = 0;
                self.cursor_row = (self.cursor_row + 1).min(self.rows - 1);
            }
        }
    }

    fn move_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.saturating_sub(1).min(self.rows - 1);
        self.cursor_col = col.saturating_sub(1).min(self.cols - 1);
    }

    fn render_to_string(&self) -> String {
        self.grid
            .iter()
            .map(|row| row.iter().collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Fullscreen TUI overlay component
pub struct TuiOverlay {
    /// Whether the overlay is currently active
    active: bool,
    /// The command being run in the overlay
    command: Option<String>,
    /// PTY handle ID for the running TUI app
    pty_handle_id: Option<usize>,
    /// Virtual screen buffer
    screen_buffer: ScreenBuffer,
    /// Whether the TUI app has exited
    has_exited: bool,
}

impl Default for TuiOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiOverlay {
    /// Create a new TUI overlay
    pub fn new() -> Self {
        Self {
            active: false,
            command: None,
            pty_handle_id: None,
            screen_buffer: ScreenBuffer::new(50, 120), // 50 rows x 120 cols
            has_exited: false,
        }
    }

    /// Check if overlay is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Start the overlay with a command
    pub fn start(&mut self, command: String, pty_handle_id: usize) {
        self.active = true;
        self.command = Some(command);
        self.pty_handle_id = Some(pty_handle_id);
        self.screen_buffer.clear(); // Clear old output
        self.has_exited = false;
    }

    /// Stop the overlay
    pub fn stop(&mut self) {
        self.active = false;
        self.command = None;
        self.pty_handle_id = None;
        self.screen_buffer.clear();
        self.has_exited = false;
    }

    /// Get the PTY handle ID
    pub fn pty_handle(&self) -> Option<usize> {
        self.pty_handle_id
    }

    /// Get the command being run
    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// Mark that the TUI app has exited
    pub fn mark_exited(&mut self) {
        self.has_exited = true;
    }

    /// Check if the TUI app has exited
    pub fn has_exited(&self) -> bool {
        self.has_exited
    }

    /// Add raw output data and process ANSI sequences
    pub fn add_raw_output(&mut self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        self.process_ansi_text(&text);
    }

    /// Process text with ANSI escape sequences and update screen buffer
    fn process_ansi_text(&mut self, text: &str) {
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // ANSI escape sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    let mut params = String::new();

                    // Collect parameters
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch.is_ascii_digit() || next_ch == ';' || next_ch == '?' {
                            params.push(next_ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    // Get command character
                    if let Some(cmd) = chars.next() {
                        self.handle_ansi_command(&params, cmd);
                    }
                } else if chars.peek() == Some(&']') || chars.peek() == Some(&'P') {
                    // OSC or DCS - skip until terminator
                    chars.next();
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == '\x07' || (next_ch == '\x1b' && chars.peek() == Some(&'\\')) {
                            if next_ch == '\x1b' {
                                chars.next();
                            }
                            break;
                        }
                    }
                } else {
                    // Other escape sequences - skip next 1-2 chars
                    chars.next();
                }
            } else {
                // Regular character
                self.screen_buffer.write_char(ch);
            }
        }
    }

    /// Handle ANSI CSI commands
    fn handle_ansi_command(&mut self, params: &str, cmd: char) {
        match cmd {
            'H' | 'f' => {
                // Cursor position
                let parts: Vec<&str> = params.split(';').collect();
                let row = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
                let col = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                self.screen_buffer.move_cursor(row, col);
            }
            'J' => {
                // Clear screen
                if params.is_empty() || params == "0" || params == "2" {
                    self.screen_buffer.clear();
                }
            }
            'K' => {
                // Clear line
                let row = self.screen_buffer.cursor_row;
                let col = self.screen_buffer.cursor_col;
                if row < self.screen_buffer.rows {
                    for c in col..self.screen_buffer.cols {
                        self.screen_buffer.grid[row][c] = ' ';
                    }
                }
            }
            _ => {
                // Ignore other commands (colors, styles, etc.)
            }
        }
    }

    /// Render the overlay
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        if !self.active {
            return false;
        }

        let mut should_close = false;

        // Request focus for the window to capture keyboard input
        ctx.memory_mut(|mem| {
            mem.request_focus(egui::Id::new("tui_overlay_window"));
        });

        // Fullscreen modal window
        egui::Window::new("TUI Application")
            .id(egui::Id::new("tui_overlay_window"))
            .title_bar(true)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size(ctx.available_rect().size() * 0.95) // 95% of available space
            .show(ctx, |ui| {
                // Show command in header
                if let Some(cmd) = &self.command {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Running: {}", cmd))
                                .strong()
                                .color(egui::Color32::from_rgb(100, 200, 255)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕ Exit (Ctrl+D)").clicked() || self.has_exited {
                                should_close = true;
                            }
                        });
                    });
                    ui.separator();
                }

                // Terminal output area
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(false) // Don't auto-scroll - TUI apps manage their own display
                    .show(ui, |ui| {
                        ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                        // Display the virtual screen buffer
                        let screen_text = self.screen_buffer.render_to_string();

                        ui.add(
                            egui::TextEdit::multiline(&mut screen_text.as_ref())
                                .font(egui::TextStyle::Monospace)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
                    });

                // Show exit message if app has exited
                if self.has_exited {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("✓ Application has exited")
                                .color(egui::Color32::from_rgb(100, 255, 100)),
                        );
                        if ui.button("Close").clicked() {
                            should_close = true;
                        }
                    });
                }
            });

        if should_close {
            self.stop();
            return true; // Signal that we closed
        }

        false
    }

    /// Handle keyboard input for the TUI app
    pub fn handle_input(&self, ctx: &egui::Context) -> Option<Vec<u8>> {
        if !self.active {
            return None;
        }

        let mut input_data = Vec::new();

        // Capture all keyboard events
        ctx.input(|i| {
            // Handle special keys first (before text input)
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    // Ctrl+D to exit
                    if *key == egui::Key::D && modifiers.ctrl {
                        input_data.extend_from_slice(b"\x04"); // EOT
                    }
                    // Convert egui keys to terminal sequences
                    else {
                        let terminal_seq = key_to_terminal_sequence(*key, modifiers);
                        if !terminal_seq.is_empty() {
                            input_data.extend_from_slice(&terminal_seq);
                        }
                    }
                }
            }

            // Handle text input (regular character typing)
            for event in &i.events {
                if let egui::Event::Text(s) = event {
                    input_data.extend_from_slice(s.as_bytes());
                }
            }
        });

        if input_data.is_empty() {
            None
        } else {
            Some(input_data)
        }
    }
}

/// Convert egui key to terminal escape sequence
fn key_to_terminal_sequence(key: egui::Key, modifiers: &egui::Modifiers) -> Vec<u8> {
    match key {
        egui::Key::Enter => vec![b'\r'],
        egui::Key::Backspace => vec![b'\x7f'],
        egui::Key::Tab => vec![b'\t'],
        egui::Key::Escape => vec![b'\x1b'],
        egui::Key::ArrowUp => vec![b'\x1b', b'[', b'A'],
        egui::Key::ArrowDown => vec![b'\x1b', b'[', b'B'],
        egui::Key::ArrowRight => vec![b'\x1b', b'[', b'C'],
        egui::Key::ArrowLeft => vec![b'\x1b', b'[', b'D'],
        egui::Key::Home => vec![b'\x1b', b'[', b'H'],
        egui::Key::End => vec![b'\x1b', b'[', b'F'],
        egui::Key::PageUp => vec![b'\x1b', b'[', b'5', b'~'],
        egui::Key::PageDown => vec![b'\x1b', b'[', b'6', b'~'],
        egui::Key::Delete => vec![b'\x1b', b'[', b'3', b'~'],
        egui::Key::Insert => vec![b'\x1b', b'[', b'2', b'~'],
        // Function keys
        egui::Key::F1 => vec![b'\x1b', b'O', b'P'],
        egui::Key::F2 => vec![b'\x1b', b'O', b'Q'],
        egui::Key::F3 => vec![b'\x1b', b'O', b'R'],
        egui::Key::F4 => vec![b'\x1b', b'O', b'S'],
        egui::Key::F5 => vec![b'\x1b', b'[', b'1', b'5', b'~'],
        egui::Key::F6 => vec![b'\x1b', b'[', b'1', b'7', b'~'],
        egui::Key::F7 => vec![b'\x1b', b'[', b'1', b'8', b'~'],
        egui::Key::F8 => vec![b'\x1b', b'[', b'1', b'9', b'~'],
        egui::Key::F9 => vec![b'\x1b', b'[', b'2', b'0', b'~'],
        egui::Key::F10 => vec![b'\x1b', b'[', b'2', b'1', b'~'],
        egui::Key::F11 => vec![b'\x1b', b'[', b'2', b'3', b'~'],
        egui::Key::F12 => vec![b'\x1b', b'[', b'2', b'4', b'~'],
        // Ctrl key combinations
        egui::Key::C if modifiers.ctrl => vec![b'\x03'],
        egui::Key::Z if modifiers.ctrl => vec![b'\x1a'],
        _ => Vec::new(),
    }
}
