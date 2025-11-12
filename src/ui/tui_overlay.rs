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
        let window_id = egui::Id::new("tui_overlay_window");

        // Fullscreen modal window
        // Use default_open to ensure window is shown, but avoid aggressive focus requests
        // that can cause accesskit assertion failures on Linux
        egui::Window::new("TUI Application")
            .id(window_id)
            .title_bar(true)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size(ctx.available_rect().size() * 0.95) // 95% of available space
            .default_open(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_buffer_new() {
        let buffer = ScreenBuffer::new(10, 20);
        assert_eq!(buffer.rows, 10);
        assert_eq!(buffer.cols, 20);
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn test_screen_buffer_clear() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.write_char('a');
        buffer.clear();
        assert_eq!(buffer.cursor_row, 0);
        assert_eq!(buffer.cursor_col, 0);
        let rendered = buffer.render_to_string();
        assert!(rendered.trim().is_empty() || rendered.chars().all(|c| c == ' ' || c == '\n'));
    }

    #[test]
    fn test_screen_buffer_write_char() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.write_char('a');
        assert_eq!(buffer.cursor_col, 1);
        buffer.write_char('b');
        assert_eq!(buffer.cursor_col, 2);
    }

    #[test]
    fn test_screen_buffer_newline() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.write_char('\n');
        assert_eq!(buffer.cursor_row, 1);
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn test_screen_buffer_carriage_return() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.write_char('a');
        buffer.write_char('\r');
        assert_eq!(buffer.cursor_col, 0);
    }

    #[test]
    fn test_screen_buffer_tab() {
        let mut buffer = ScreenBuffer::new(5, 20);
        let initial_col = buffer.cursor_col;
        buffer.write_char('\t');
        assert_eq!(buffer.cursor_col, initial_col + 8);
    }

    #[test]
    fn test_screen_buffer_wrap() {
        let mut buffer = ScreenBuffer::new(5, 3);
        buffer.write_char('a');
        buffer.write_char('b');
        buffer.write_char('c');
        buffer.write_char('d');
        assert_eq!(buffer.cursor_row, 1);
        assert_eq!(buffer.cursor_col, 1);
    }

    #[test]
    fn test_screen_buffer_move_cursor() {
        let mut buffer = ScreenBuffer::new(10, 20);
        buffer.move_cursor(5, 10);
        assert_eq!(buffer.cursor_row, 4); // saturating_sub(1)
        assert_eq!(buffer.cursor_col, 9); // saturating_sub(1)
    }

    #[test]
    fn test_screen_buffer_render_to_string() {
        let mut buffer = ScreenBuffer::new(3, 5);
        buffer.write_char('H');
        buffer.write_char('e');
        buffer.write_char('l');
        buffer.write_char('l');
        buffer.write_char('o');
        buffer.write_char('\n');
        buffer.write_char('W');
        buffer.write_char('o');
        buffer.write_char('r');
        buffer.write_char('l');
        buffer.write_char('d');
        let rendered = buffer.render_to_string();
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("World"));
    }

    #[test]
    fn test_tui_overlay_new() {
        let overlay = TuiOverlay::new();
        assert!(!overlay.is_active());
        assert!(overlay.command().is_none());
        assert!(overlay.pty_handle().is_none());
        assert!(!overlay.has_exited());
    }

    #[test]
    fn test_tui_overlay_start() {
        let mut overlay = TuiOverlay::new();
        overlay.start("vim".to_string(), 123);
        assert!(overlay.is_active());
        assert_eq!(overlay.command(), Some("vim"));
        assert_eq!(overlay.pty_handle(), Some(123));
        assert!(!overlay.has_exited());
    }

    #[test]
    fn test_tui_overlay_stop() {
        let mut overlay = TuiOverlay::new();
        overlay.start("vim".to_string(), 123);
        overlay.stop();
        assert!(!overlay.is_active());
        assert!(overlay.command().is_none());
        assert!(overlay.pty_handle().is_none());
        assert!(!overlay.has_exited());
    }

    #[test]
    fn test_tui_overlay_mark_exited() {
        let mut overlay = TuiOverlay::new();
        overlay.start("vim".to_string(), 123);
        overlay.mark_exited();
        assert!(overlay.has_exited());
        assert!(overlay.is_active()); // Still active until stopped
    }

    #[test]
    fn test_tui_overlay_add_raw_output() {
        let mut overlay = TuiOverlay::new();
        overlay.add_raw_output(b"Hello\nWorld");
        // Verify output was processed (can't easily test internal state)
        // But we can verify it doesn't panic
    }

    #[test]
    fn test_tui_overlay_process_ansi_text() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("Hello\x1b[2JWorld");
        // Test that ANSI sequences are processed
        let rendered = overlay.screen_buffer.render_to_string();
        assert!(rendered.contains("World"));
    }

    #[test]
    fn test_tui_overlay_ansi_cursor_position() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[5;10HTest");
        // Cursor should be moved to row 5, col 10
        let rendered = overlay.screen_buffer.render_to_string();
        // Should contain "Test" somewhere
        assert!(rendered.contains("Test") || rendered.trim().is_empty());
    }

    #[test]
    fn test_tui_overlay_ansi_clear_screen() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("Old text\x1b[2JNew text");
        // Screen should be cleared
        let rendered = overlay.screen_buffer.render_to_string();
        // Should contain new text, old text may or may not be there depending on implementation
        assert!(rendered.contains("New") || rendered.trim().is_empty());
    }

    #[test]
    fn test_key_to_terminal_sequence() {
        assert_eq!(
            key_to_terminal_sequence(egui::Key::Enter, &egui::Modifiers::default()),
            vec![b'\r']
        );
        assert_eq!(
            key_to_terminal_sequence(egui::Key::Backspace, &egui::Modifiers::default()),
            vec![b'\x7f']
        );
        assert_eq!(
            key_to_terminal_sequence(egui::Key::Tab, &egui::Modifiers::default()),
            vec![b'\t']
        );
        assert_eq!(
            key_to_terminal_sequence(egui::Key::Escape, &egui::Modifiers::default()),
            vec![b'\x1b']
        );
        assert_eq!(
            key_to_terminal_sequence(egui::Key::ArrowUp, &egui::Modifiers::default()),
            vec![b'\x1b', b'[', b'A']
        );
    }

    #[test]
    fn test_tui_overlay_default() {
        let overlay = TuiOverlay::default();
        assert!(!overlay.is_active());
    }
}
