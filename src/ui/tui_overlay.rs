//! Fullscreen TUI application overlay
//!
//! This module provides a fullscreen overlay for running interactive TUI applications
//! like vim, htop, top, etc. The overlay captures all input and renders a virtual
//! terminal screen buffer with ANSI color and cursor positioning support.

use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq)]
struct AnsiColor {
    r: u8,
    g: u8,
    b: u8,
}

impl AnsiColor {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn to_color32(self) -> egui::Color32 {
        egui::Color32::from_rgb(self.r, self.g, self.b)
    }

    fn from_256(idx: u8) -> Self {
        match idx {
            0 => Self::rgb(0, 0, 0),
            1 => Self::rgb(205, 0, 0),
            2 => Self::rgb(0, 205, 0),
            3 => Self::rgb(205, 205, 0),
            4 => Self::rgb(0, 0, 238),
            5 => Self::rgb(205, 0, 205),
            6 => Self::rgb(0, 205, 205),
            7 => Self::rgb(229, 229, 229),
            8 => Self::rgb(127, 127, 127),
            9 => Self::rgb(255, 0, 0),
            10 => Self::rgb(0, 255, 0),
            11 => Self::rgb(255, 255, 0),
            12 => Self::rgb(92, 92, 255),
            13 => Self::rgb(255, 0, 255),
            14 => Self::rgb(0, 255, 255),
            15 => Self::rgb(255, 255, 255),
            16..=231 => {
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
                Self::rgb(to_val(r), to_val(g), to_val(b))
            }
            232..=255 => {
                let gray = 8 + 10 * (idx - 232);
                Self::rgb(gray, gray, gray)
            }
        }
    }
}

const DEFAULT_FG: AnsiColor = AnsiColor::rgb(204, 204, 204);
const DEFAULT_BG: AnsiColor = AnsiColor::rgb(30, 30, 46);

const ANSI_COLORS: [AnsiColor; 8] = [
    AnsiColor::rgb(0, 0, 0),
    AnsiColor::rgb(205, 0, 0),
    AnsiColor::rgb(0, 205, 0),
    AnsiColor::rgb(205, 205, 0),
    AnsiColor::rgb(0, 0, 238),
    AnsiColor::rgb(205, 0, 205),
    AnsiColor::rgb(0, 205, 205),
    AnsiColor::rgb(229, 229, 229),
];

const ANSI_BRIGHT_COLORS: [AnsiColor; 8] = [
    AnsiColor::rgb(127, 127, 127),
    AnsiColor::rgb(255, 0, 0),
    AnsiColor::rgb(0, 255, 0),
    AnsiColor::rgb(255, 255, 0),
    AnsiColor::rgb(92, 92, 255),
    AnsiColor::rgb(255, 0, 255),
    AnsiColor::rgb(0, 255, 255),
    AnsiColor::rgb(255, 255, 255),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct CellStyle {
    fg: AnsiColor,
    bg: AnsiColor,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    reverse: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            fg: DEFAULT_FG,
            bg: DEFAULT_BG,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
        }
    }
}

impl CellStyle {
    fn effective_fg(&self) -> AnsiColor {
        if self.reverse {
            self.bg
        } else {
            self.fg
        }
    }

    fn effective_bg(&self) -> AnsiColor {
        if self.reverse {
            self.fg
        } else {
            self.bg
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    ch: char,
    style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: CellStyle::default(),
        }
    }
}

/// Virtual screen buffer for TUI apps with per-cell styling
struct ScreenBuffer {
    grid: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    rows: usize,
    cols: usize,
    current_style: CellStyle,
    scroll_top: usize,
    scroll_bottom: usize,
    saved_cursor: Option<(usize, usize)>,
}

impl ScreenBuffer {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            grid: vec![vec![Cell::default(); cols]; rows],
            cursor_row: 0,
            cursor_col: 0,
            rows,
            cols,
            current_style: CellStyle::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            saved_cursor: None,
        }
    }

    fn resize(&mut self, new_rows: usize, new_cols: usize) {
        if new_rows == self.rows && new_cols == self.cols {
            return;
        }
        let mut new_grid = vec![vec![Cell::default(); new_cols]; new_rows];
        let copy_rows = self.rows.min(new_rows);
        let copy_cols = self.cols.min(new_cols);
        for (new_row, old_row) in new_grid.iter_mut().zip(self.grid.iter()).take(copy_rows) {
            new_row[..copy_cols].copy_from_slice(&old_row[..copy_cols]);
        }
        self.grid = new_grid;
        self.rows = new_rows;
        self.cols = new_cols;
        self.scroll_bottom = new_rows.saturating_sub(1);
        self.cursor_row = self.cursor_row.min(new_rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(new_cols.saturating_sub(1));
    }

    fn clear(&mut self) {
        let blank = Cell {
            ch: ' ',
            style: self.current_style,
        };
        for row in &mut self.grid {
            row.fill(blank);
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn clear_from_cursor_to_end_of_screen(&mut self) {
        let blank = Cell {
            ch: ' ',
            style: self.current_style,
        };
        if self.cursor_row < self.rows {
            for c in self.cursor_col..self.cols {
                self.grid[self.cursor_row][c] = blank;
            }
            for r in (self.cursor_row + 1)..self.rows {
                self.grid[r].fill(blank);
            }
        }
    }

    fn clear_from_start_to_cursor(&mut self) {
        let blank = Cell {
            ch: ' ',
            style: self.current_style,
        };
        for r in 0..self.cursor_row {
            self.grid[r].fill(blank);
        }
        if self.cursor_row < self.rows {
            for c in 0..=self.cursor_col.min(self.cols - 1) {
                self.grid[self.cursor_row][c] = blank;
            }
        }
    }

    fn clear_line_from_cursor(&mut self) {
        if self.cursor_row < self.rows {
            let blank = Cell {
                ch: ' ',
                style: self.current_style,
            };
            for c in self.cursor_col..self.cols {
                self.grid[self.cursor_row][c] = blank;
            }
        }
    }

    fn clear_line_to_cursor(&mut self) {
        if self.cursor_row < self.rows {
            let blank = Cell {
                ch: ' ',
                style: self.current_style,
            };
            for c in 0..=self.cursor_col.min(self.cols - 1) {
                self.grid[self.cursor_row][c] = blank;
            }
        }
    }

    fn clear_entire_line(&mut self) {
        if self.cursor_row < self.rows {
            let blank = Cell {
                ch: ' ',
                style: self.current_style,
            };
            self.grid[self.cursor_row].fill(blank);
        }
    }

    fn write_char(&mut self, ch: char) {
        match ch {
            '\n' => {
                if self.cursor_row >= self.scroll_bottom {
                    self.scroll_up(1);
                } else {
                    self.cursor_row += 1;
                }
                self.cursor_col = 0;
            }
            '\r' => {
                self.cursor_col = 0;
            }
            '\t' => {
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols.saturating_sub(1));
            }
            '\x08' => {
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            c if c >= ' ' => {
                if self.cursor_row < self.rows && self.cursor_col < self.cols {
                    self.grid[self.cursor_row][self.cursor_col] = Cell {
                        ch: c,
                        style: self.current_style,
                    };
                    self.cursor_col += 1;
                    if self.cursor_col >= self.cols {
                        self.cursor_col = 0;
                        if self.cursor_row >= self.scroll_bottom {
                            self.scroll_up(1);
                        } else {
                            self.cursor_row += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let blank = Cell {
            ch: ' ',
            style: CellStyle::default(),
        };
        for _ in 0..n {
            if self.scroll_top < self.scroll_bottom && self.scroll_bottom < self.rows {
                for r in self.scroll_top..self.scroll_bottom {
                    self.grid[r] = self.grid[r + 1].clone();
                }
                self.grid[self.scroll_bottom].fill(blank);
            }
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let blank = Cell {
            ch: ' ',
            style: CellStyle::default(),
        };
        for _ in 0..n {
            if self.scroll_top < self.scroll_bottom && self.scroll_bottom < self.rows {
                for r in (self.scroll_top + 1..=self.scroll_bottom).rev() {
                    self.grid[r] = self.grid[r - 1].clone();
                }
                self.grid[self.scroll_top].fill(blank);
            }
        }
    }

    fn insert_lines(&mut self, n: usize) {
        let blank = Cell {
            ch: ' ',
            style: CellStyle::default(),
        };
        for _ in 0..n {
            if self.cursor_row <= self.scroll_bottom && self.scroll_bottom < self.rows {
                for r in (self.cursor_row + 1..=self.scroll_bottom).rev() {
                    self.grid[r] = self.grid[r - 1].clone();
                }
                self.grid[self.cursor_row].fill(blank);
            }
        }
    }

    fn delete_lines(&mut self, n: usize) {
        let blank = Cell {
            ch: ' ',
            style: CellStyle::default(),
        };
        for _ in 0..n {
            if self.cursor_row <= self.scroll_bottom && self.scroll_bottom < self.rows {
                for r in self.cursor_row..self.scroll_bottom {
                    self.grid[r] = self.grid[r + 1].clone();
                }
                self.grid[self.scroll_bottom].fill(blank);
            }
        }
    }

    fn insert_chars(&mut self, n: usize) {
        if self.cursor_row < self.rows {
            let blank = Cell {
                ch: ' ',
                style: self.current_style,
            };
            for _ in 0..n {
                if self.cursor_col < self.cols {
                    self.grid[self.cursor_row].pop();
                    self.grid[self.cursor_row].insert(self.cursor_col, blank);
                }
            }
            self.grid[self.cursor_row].truncate(self.cols);
        }
    }

    fn delete_chars(&mut self, n: usize) {
        if self.cursor_row < self.rows {
            let blank = Cell {
                ch: ' ',
                style: self.current_style,
            };
            for _ in 0..n {
                if self.cursor_col < self.grid[self.cursor_row].len() {
                    self.grid[self.cursor_row].remove(self.cursor_col);
                    self.grid[self.cursor_row].push(blank);
                }
            }
            self.grid[self.cursor_row].truncate(self.cols);
        }
    }

    fn move_cursor(&mut self, row: usize, col: usize) {
        self.cursor_row = row.saturating_sub(1).min(self.rows.saturating_sub(1));
        self.cursor_col = col.saturating_sub(1).min(self.cols.saturating_sub(1));
    }

    fn render_to_layout_job(&self, font_id: egui::FontId) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob {
            wrap: egui::text::TextWrapping {
                max_width: f32::INFINITY,
                ..Default::default()
            },
            ..Default::default()
        };

        for (row_idx, row) in self.grid.iter().enumerate() {
            let trimmed_len = row.iter().rposition(|c| c.ch != ' ').map_or(0, |p| p + 1);

            let mut run_start = 0;
            while run_start < trimmed_len {
                let style = row[run_start].style;
                let mut run_end = run_start + 1;
                while run_end < trimmed_len && row[run_end].style == style {
                    run_end += 1;
                }

                let text: String = row[run_start..run_end].iter().map(|c| c.ch).collect();
                let fg = style.effective_fg();
                let bg = style.effective_bg();

                let mut text_format = egui::TextFormat {
                    font_id: font_id.clone(),
                    color: fg.to_color32(),
                    ..Default::default()
                };
                if bg != DEFAULT_BG {
                    text_format.background = bg.to_color32();
                }
                if style.bold {
                    text_format.color = brighten(text_format.color);
                }
                if style.underline {
                    text_format.underline = egui::Stroke::new(1.0, text_format.color);
                }
                if style.italic {
                    text_format.italics = true;
                }

                job.append(&text, 0.0, text_format);
                run_start = run_end;
            }

            if row_idx < self.rows - 1 {
                job.append(
                    "\n",
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: DEFAULT_FG.to_color32(),
                        ..Default::default()
                    },
                );
            }
        }

        job
    }
}

fn brighten(color: egui::Color32) -> egui::Color32 {
    let [r, g, b, a] = color.to_array();
    egui::Color32::from_rgba_premultiplied(
        r.saturating_add(40),
        g.saturating_add(40),
        b.saturating_add(40),
        a,
    )
}

/// Fullscreen TUI overlay component
pub struct TuiOverlay {
    /// Whether the overlay is currently active
    active: bool,
    /// The command being run in the overlay
    command: Option<String>,
    /// PTY handle ID for the running TUI app
    pty_handle_id: Option<String>,
    /// Virtual screen buffer
    screen_buffer: ScreenBuffer,
    /// Whether the TUI app has exited
    has_exited: bool,
    /// Last measured available size in character cells (rows, cols)
    last_char_size: Option<(usize, usize)>,
    /// Timestamp of last Escape press for double-Escape detection
    last_escape_time: Option<std::time::Instant>,
    /// Pending resize (rows, cols) that the caller should apply to the PTY
    pending_resize: Option<(u16, u16)>,
    /// When the overlay was activated (for grace-period exit detection)
    started_at: Option<std::time::Instant>,
    /// Whether the TUI app has entered alternate screen buffer
    saw_alt_screen_enter: bool,
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
            screen_buffer: ScreenBuffer::new(50, 120),
            has_exited: false,
            last_char_size: None,
            last_escape_time: None,
            pending_resize: None,
            started_at: None,
            saw_alt_screen_enter: false,
        }
    }

    /// Check if overlay is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Start the overlay with a command
    pub fn start(&mut self, command: String, pty_handle_id: String) {
        self.active = true;
        self.command = Some(command);
        self.pty_handle_id = Some(pty_handle_id);
        self.screen_buffer.clear();
        self.has_exited = false;
        self.started_at = Some(std::time::Instant::now());
        self.saw_alt_screen_enter = false;
    }

    /// Stop the overlay
    pub fn stop(&mut self) {
        self.active = false;
        self.command = None;
        self.pty_handle_id = None;
        self.screen_buffer.clear();
        self.has_exited = false;
        self.started_at = None;
        self.saw_alt_screen_enter = false;
    }

    /// Whether the overlay is still in the startup grace period (ignore exit sequences)
    pub fn in_grace_period(&self) -> bool {
        self.started_at
            .map(|t| t.elapsed() < std::time::Duration::from_millis(800))
            .unwrap_or(false)
    }

    /// Record that we saw the alt screen enter sequence
    pub fn note_alt_screen_enter(&mut self) {
        self.saw_alt_screen_enter = true;
    }

    /// Whether the TUI app has entered alternate screen at least once
    pub fn saw_alt_screen(&self) -> bool {
        self.saw_alt_screen_enter
    }

    /// Get the PTY handle ID
    pub fn pty_handle(&self) -> Option<&str> {
        self.pty_handle_id.as_deref()
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

    /// Get the current buffer dimensions for PTY size reporting
    pub fn buffer_size(&self) -> (usize, usize) {
        (self.screen_buffer.rows, self.screen_buffer.cols)
    }

    /// Get the last measured character cell size (rows, cols), if available.
    pub fn last_size(&self) -> Option<(usize, usize)> {
        self.last_char_size
    }

    /// Take the pending resize request, if any.
    /// Returns (rows, cols) as u16 suitable for PTY resize calls.
    pub fn take_pending_resize(&mut self) -> Option<(u16, u16)> {
        self.pending_resize.take()
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
                if chars.peek() == Some(&'[') {
                    chars.next();
                    let mut params = String::new();

                    while let Some(&next_ch) = chars.peek() {
                        if next_ch.is_ascii_digit()
                            || next_ch == ';'
                            || next_ch == '?'
                            || next_ch == ' '
                            || next_ch == '>'
                            || next_ch == '!'
                        {
                            params.push(next_ch);
                            chars.next();
                        } else {
                            break;
                        }
                    }

                    if let Some(cmd) = chars.next() {
                        self.handle_csi_command(&params, cmd);
                    }
                } else if chars.peek() == Some(&']') {
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
                } else if chars.peek() == Some(&'P')
                    || chars.peek() == Some(&'^')
                    || chars.peek() == Some(&'_')
                {
                    chars.next();
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                } else if chars.peek() == Some(&'(') || chars.peek() == Some(&')') {
                    chars.next();
                    chars.next();
                } else if chars.peek() == Some(&'7') {
                    chars.next();
                    self.screen_buffer.saved_cursor =
                        Some((self.screen_buffer.cursor_row, self.screen_buffer.cursor_col));
                } else if chars.peek() == Some(&'8') {
                    chars.next();
                    if let Some((r, c)) = self.screen_buffer.saved_cursor {
                        self.screen_buffer.cursor_row = r;
                        self.screen_buffer.cursor_col = c;
                    }
                } else if chars.peek() == Some(&'M') {
                    chars.next();
                    if self.screen_buffer.cursor_row == self.screen_buffer.scroll_top {
                        self.screen_buffer.scroll_down(1);
                    } else {
                        self.screen_buffer.cursor_row =
                            self.screen_buffer.cursor_row.saturating_sub(1);
                    }
                } else if chars.peek() == Some(&'D') {
                    chars.next();
                    if self.screen_buffer.cursor_row >= self.screen_buffer.scroll_bottom {
                        self.screen_buffer.scroll_up(1);
                    } else {
                        self.screen_buffer.cursor_row += 1;
                    }
                } else if chars.peek() == Some(&'E') {
                    chars.next();
                    if self.screen_buffer.cursor_row >= self.screen_buffer.scroll_bottom {
                        self.screen_buffer.scroll_up(1);
                    } else {
                        self.screen_buffer.cursor_row += 1;
                    }
                    self.screen_buffer.cursor_col = 0;
                } else if chars.peek() == Some(&'c') {
                    chars.next();
                    self.screen_buffer.clear();
                    self.screen_buffer.current_style = CellStyle::default();
                } else {
                    chars.next();
                }
            } else {
                self.screen_buffer.write_char(ch);
            }
        }
    }

    /// Handle CSI (Control Sequence Introducer) commands
    fn handle_csi_command(&mut self, params: &str, cmd: char) {
        let clean_params = params.trim_start_matches('?').trim_start_matches('>');

        match cmd {
            'H' | 'f' => {
                let parts: Vec<&str> = clean_params.split(';').collect();
                let row = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
                let col = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                self.screen_buffer.move_cursor(row, col);
            }
            'A' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_row = self.screen_buffer.cursor_row.saturating_sub(n);
            }
            'B' | 'e' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_row = (self.screen_buffer.cursor_row + n)
                    .min(self.screen_buffer.rows.saturating_sub(1));
            }
            'C' | 'a' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_col = (self.screen_buffer.cursor_col + n)
                    .min(self.screen_buffer.cols.saturating_sub(1));
            }
            'D' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_col = self.screen_buffer.cursor_col.saturating_sub(n);
            }
            'E' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_row = (self.screen_buffer.cursor_row + n)
                    .min(self.screen_buffer.rows.saturating_sub(1));
                self.screen_buffer.cursor_col = 0;
            }
            'F' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.cursor_row = self.screen_buffer.cursor_row.saturating_sub(n);
                self.screen_buffer.cursor_col = 0;
            }
            'G' | '`' => {
                let col: usize = clean_params.parse().unwrap_or(1);
                self.screen_buffer.cursor_col = col
                    .saturating_sub(1)
                    .min(self.screen_buffer.cols.saturating_sub(1));
            }
            'd' => {
                let row: usize = clean_params.parse().unwrap_or(1);
                self.screen_buffer.cursor_row = row
                    .saturating_sub(1)
                    .min(self.screen_buffer.rows.saturating_sub(1));
            }
            'J' => {
                let mode: u8 = clean_params.parse().unwrap_or(0);
                match mode {
                    0 => self.screen_buffer.clear_from_cursor_to_end_of_screen(),
                    1 => self.screen_buffer.clear_from_start_to_cursor(),
                    2 | 3 => self.screen_buffer.clear(),
                    _ => {}
                }
            }
            'K' => {
                let mode: u8 = clean_params.parse().unwrap_or(0);
                match mode {
                    0 => self.screen_buffer.clear_line_from_cursor(),
                    1 => self.screen_buffer.clear_line_to_cursor(),
                    2 => self.screen_buffer.clear_entire_line(),
                    _ => {}
                }
            }
            'm' => {
                self.handle_sgr(clean_params);
            }
            'r' => {
                let parts: Vec<&str> = clean_params.split(';').collect();
                let top = parts
                    .first()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);
                let bottom = parts
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(self.screen_buffer.rows);
                self.screen_buffer.scroll_top = top.saturating_sub(1);
                self.screen_buffer.scroll_bottom =
                    (bottom.saturating_sub(1)).min(self.screen_buffer.rows.saturating_sub(1));
                self.screen_buffer.cursor_row = 0;
                self.screen_buffer.cursor_col = 0;
            }
            'S' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.scroll_up(n);
            }
            'T' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.scroll_down(n);
            }
            'L' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.insert_lines(n);
            }
            'M' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.delete_lines(n);
            }
            '@' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.insert_chars(n);
            }
            'P' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                self.screen_buffer.delete_chars(n);
            }
            'X' => {
                let n: usize = clean_params.parse().unwrap_or(1).max(1);
                let row = self.screen_buffer.cursor_row;
                let col = self.screen_buffer.cursor_col;
                if row < self.screen_buffer.rows {
                    let blank = Cell {
                        ch: ' ',
                        style: self.screen_buffer.current_style,
                    };
                    for c in col..(col + n).min(self.screen_buffer.cols) {
                        self.screen_buffer.grid[row][c] = blank;
                    }
                }
            }
            's' => {
                self.screen_buffer.saved_cursor =
                    Some((self.screen_buffer.cursor_row, self.screen_buffer.cursor_col));
            }
            'u' => {
                if let Some((r, c)) = self.screen_buffer.saved_cursor {
                    self.screen_buffer.cursor_row = r;
                    self.screen_buffer.cursor_col = c;
                }
            }
            'h' | 'l' => {
                // DEC private mode set/reset
                let set = cmd == 'h';
                for p in params.replace('?', "").split(';') {
                    match p.trim() {
                        "1049" | "47" | "1047" => {
                            // Alternate screen buffer
                            if set {
                                self.screen_buffer.clear();
                            }
                        }
                        "25" => {
                            // Cursor visibility (not visually rendered yet)
                        }
                        "1" => {
                            // Application cursor keys mode
                        }
                        "7" => {
                            // Auto-wrap mode
                        }
                        _ => {}
                    }
                }
            }
            'n' | 'c' => {
                // Device status reports / device attributes - consumed
            }
            _ => {}
        }
    }

    /// Handle SGR (Select Graphic Rendition) parameters
    fn handle_sgr(&mut self, params: &str) {
        if params.is_empty() {
            self.screen_buffer.current_style = CellStyle::default();
            return;
        }

        let codes: Vec<u16> = params.split(';').filter_map(|s| s.parse().ok()).collect();

        let mut i = 0;
        while i < codes.len() {
            match codes[i] {
                0 => self.screen_buffer.current_style = CellStyle::default(),
                1 => self.screen_buffer.current_style.bold = true,
                2 => self.screen_buffer.current_style.dim = true,
                3 => self.screen_buffer.current_style.italic = true,
                4 => self.screen_buffer.current_style.underline = true,
                7 => self.screen_buffer.current_style.reverse = true,
                21 | 22 => {
                    self.screen_buffer.current_style.bold = false;
                    self.screen_buffer.current_style.dim = false;
                }
                23 => self.screen_buffer.current_style.italic = false,
                24 => self.screen_buffer.current_style.underline = false,
                27 => self.screen_buffer.current_style.reverse = false,
                30..=37 => {
                    self.screen_buffer.current_style.fg = ANSI_COLORS[(codes[i] - 30) as usize];
                }
                38 => {
                    if i + 2 < codes.len() && codes[i + 1] == 5 {
                        self.screen_buffer.current_style.fg =
                            AnsiColor::from_256(codes[i + 2] as u8);
                        i += 2;
                    } else if i + 4 < codes.len() && codes[i + 1] == 2 {
                        self.screen_buffer.current_style.fg = AnsiColor::rgb(
                            codes[i + 2] as u8,
                            codes[i + 3] as u8,
                            codes[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                39 => self.screen_buffer.current_style.fg = DEFAULT_FG,
                40..=47 => {
                    self.screen_buffer.current_style.bg = ANSI_COLORS[(codes[i] - 40) as usize];
                }
                48 => {
                    if i + 2 < codes.len() && codes[i + 1] == 5 {
                        self.screen_buffer.current_style.bg =
                            AnsiColor::from_256(codes[i + 2] as u8);
                        i += 2;
                    } else if i + 4 < codes.len() && codes[i + 1] == 2 {
                        self.screen_buffer.current_style.bg = AnsiColor::rgb(
                            codes[i + 2] as u8,
                            codes[i + 3] as u8,
                            codes[i + 4] as u8,
                        );
                        i += 4;
                    }
                }
                49 => self.screen_buffer.current_style.bg = DEFAULT_BG,
                90..=97 => {
                    self.screen_buffer.current_style.fg =
                        ANSI_BRIGHT_COLORS[(codes[i] - 90) as usize];
                }
                100..=107 => {
                    self.screen_buffer.current_style.bg =
                        ANSI_BRIGHT_COLORS[(codes[i] - 100) as usize];
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// Render the overlay using the full window area with top and bottom bars
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        if !self.active {
            return false;
        }

        let mut should_close = false;

        let header_color = egui::Color32::from_rgb(20, 20, 35);
        let border_color = egui::Color32::from_rgb(60, 60, 90);

        // Top bar with command name and Escape hint
        egui::TopBottomPanel::top("tui_header")
            .frame(
                egui::Frame::none()
                    .fill(header_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if let Some(cmd) = &self.command {
                        ui.label(
                            egui::RichText::new(format!(" {}", cmd))
                                .font(egui::FontId::monospace(13.0))
                                .color(egui::Color32::from_rgb(100, 200, 255))
                                .strong(),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Esc to close")
                                .font(egui::FontId::proportional(11.0))
                                .color(egui::Color32::from_rgb(120, 120, 140)),
                        );
                    });
                });
            });

        // Bottom bar with exit button and status
        egui::TopBottomPanel::bottom("tui_footer")
            .frame(
                egui::Frame::none()
                    .fill(header_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .inner_margin(egui::Margin::symmetric(12.0, 5.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.has_exited {
                        ui.label(
                            egui::RichText::new("Process exited")
                                .font(egui::FontId::proportional(12.0))
                                .color(egui::Color32::from_rgb(100, 255, 100)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Running")
                                .font(egui::FontId::proportional(12.0))
                                .color(egui::Color32::from_rgb(100, 200, 100)),
                        );
                        ui.label(
                            egui::RichText::new("  (double-Esc to close)")
                                .font(egui::FontId::proportional(11.0))
                                .color(egui::Color32::from_rgb(140, 140, 160)),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn = ui.button(
                            egui::RichText::new("  Exit  ").font(egui::FontId::proportional(12.0)),
                        );
                        if btn.clicked() || self.has_exited {
                            should_close = true;
                        }
                    });
                });
            });

        // Central terminal area fills the rest
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(DEFAULT_BG.to_color32())
                    .inner_margin(egui::Margin::symmetric(4.0, 2.0)),
            )
            .show(ctx, |ui| {
                let mono_font = egui::FontId::new(13.0, egui::FontFamily::Monospace);
                let char_width = ui.fonts(|fonts| fonts.glyph_width(&mono_font, 'M'));
                let line_height = ui.text_style_height(&egui::TextStyle::Monospace);

                let available = ui.available_size();
                let cols = ((available.x / char_width) as usize).max(40);
                let rows = ((available.y / line_height) as usize).max(10);

                let new_size = (rows, cols);
                if self.last_char_size != Some(new_size) {
                    self.last_char_size = Some(new_size);
                    self.screen_buffer.resize(rows, cols);
                    self.pending_resize = Some((rows as u16, cols as u16));
                }

                let job = self.screen_buffer.render_to_layout_job(mono_font);
                let galley = ui.fonts(|fonts| fonts.layout_job(job));
                let (response, painter) = ui.allocate_painter(galley.size(), egui::Sense::hover());
                painter.galley(response.rect.min, galley);
            });

        // Double-Escape to close overlay (single Escape is forwarded to the app)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            let now = std::time::Instant::now();
            if let Some(last) = self.last_escape_time {
                if now.duration_since(last).as_millis() < 400 {
                    should_close = true;
                    self.last_escape_time = None;
                } else {
                    self.last_escape_time = Some(now);
                }
            } else {
                self.last_escape_time = Some(now);
            }
        }

        if should_close {
            self.stop();
            return true;
        }

        false
    }

    /// Handle keyboard input for the TUI app.
    /// Double-Escape closes the overlay; single Escape is forwarded to the app.
    pub fn handle_input(&self, ctx: &egui::Context) -> Option<Vec<u8>> {
        if !self.active {
            return None;
        }

        let mut input_data = Vec::new();

        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    if *key == egui::Key::D && modifiers.ctrl {
                        input_data.extend_from_slice(b"\x04");
                    } else if *key == egui::Key::Escape {
                        // Forward Escape to the PTY (double-Escape close is handled in render())
                        input_data.extend_from_slice(b"\x1b");
                    } else {
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
        egui::Key::A if modifiers.ctrl => vec![b'\x01'],
        egui::Key::B if modifiers.ctrl => vec![b'\x02'],
        egui::Key::C if modifiers.ctrl => vec![b'\x03'],
        egui::Key::E if modifiers.ctrl => vec![b'\x05'],
        egui::Key::F if modifiers.ctrl => vec![b'\x06'],
        egui::Key::G if modifiers.ctrl => vec![b'\x07'],
        egui::Key::H if modifiers.ctrl => vec![b'\x08'],
        egui::Key::K if modifiers.ctrl => vec![b'\x0b'],
        egui::Key::L if modifiers.ctrl => vec![b'\x0c'],
        egui::Key::N if modifiers.ctrl => vec![b'\x0e'],
        egui::Key::O if modifiers.ctrl => vec![b'\x0f'],
        egui::Key::P if modifiers.ctrl => vec![b'\x10'],
        egui::Key::R if modifiers.ctrl => vec![b'\x12'],
        egui::Key::S if modifiers.ctrl => vec![b'\x13'],
        egui::Key::T if modifiers.ctrl => vec![b'\x14'],
        egui::Key::U if modifiers.ctrl => vec![b'\x15'],
        egui::Key::V if modifiers.ctrl => vec![b'\x16'],
        egui::Key::W if modifiers.ctrl => vec![b'\x17'],
        egui::Key::X if modifiers.ctrl => vec![b'\x18'],
        egui::Key::Y if modifiers.ctrl => vec![b'\x19'],
        egui::Key::Z if modifiers.ctrl => vec![b'\x1a'],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_text(buffer: &ScreenBuffer) -> String {
        buffer
            .grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| c.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

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
        let rendered = render_text(&buffer);
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
        buffer.write_char('\t');
        assert_eq!(buffer.cursor_col, 8);
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
        assert_eq!(buffer.cursor_row, 4);
        assert_eq!(buffer.cursor_col, 9);
    }

    #[test]
    fn test_screen_buffer_render_text() {
        let mut buffer = ScreenBuffer::new(5, 10);
        for ch in "Hello".chars() {
            buffer.write_char(ch);
        }
        buffer.write_char('\n');
        for ch in "World".chars() {
            buffer.write_char(ch);
        }
        let rendered = render_text(&buffer);
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("World"));
    }

    #[test]
    fn test_screen_buffer_resize() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.write_char('X');
        buffer.resize(10, 20);
        assert_eq!(buffer.rows, 10);
        assert_eq!(buffer.cols, 20);
        assert_eq!(buffer.grid[0][0].ch, 'X');
    }

    #[test]
    fn test_clear_modes() {
        let mut buffer = ScreenBuffer::new(5, 10);
        for ch in "Hello".chars() {
            buffer.write_char(ch);
        }
        buffer.move_cursor(1, 3);
        buffer.clear_line_from_cursor();
        assert_eq!(buffer.grid[0][0].ch, 'H');
        assert_eq!(buffer.grid[0][1].ch, 'e');
        assert_eq!(buffer.grid[0][4].ch, ' ');
    }

    #[test]
    fn test_scroll_region() {
        let mut buffer = ScreenBuffer::new(5, 10);
        buffer.scroll_top = 1;
        buffer.scroll_bottom = 3;
        for ch in "Line0".chars() {
            buffer.write_char(ch);
        }
        buffer.move_cursor(2, 1);
        for ch in "Line1".chars() {
            buffer.write_char(ch);
        }
        buffer.scroll_up(1);
        assert_eq!(buffer.grid[0][0].ch, 'L');
    }

    #[test]
    fn test_sgr_colors() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[31mRed\x1b[0mNormal");
        assert_eq!(overlay.screen_buffer.grid[0][0].style.fg, ANSI_COLORS[1]);
        assert_eq!(overlay.screen_buffer.grid[0][3].style.fg, DEFAULT_FG);
    }

    #[test]
    fn test_sgr_bold_reverse() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[1;7mBR\x1b[0m");
        assert!(overlay.screen_buffer.grid[0][0].style.bold);
        assert!(overlay.screen_buffer.grid[0][0].style.reverse);
    }

    #[test]
    fn test_cursor_movement_csi() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[5;10Hx");
        assert_eq!(overlay.screen_buffer.grid[4][9].ch, 'x');

        overlay.process_ansi_text("\x1b[2Ay");
        assert_eq!(overlay.screen_buffer.cursor_row, 2);
    }

    #[test]
    fn test_erase_in_display_modes() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("ABCDE\x1b[1;3H\x1b[0J");
        assert_eq!(overlay.screen_buffer.grid[0][0].ch, 'A');
        assert_eq!(overlay.screen_buffer.grid[0][1].ch, 'B');
        assert_eq!(overlay.screen_buffer.grid[0][4].ch, ' ');
    }

    #[test]
    fn test_erase_in_line_modes() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("ABCDE\x1b[1;3H\x1b[1K");
        assert_eq!(overlay.screen_buffer.grid[0][0].ch, ' ');
        assert_eq!(overlay.screen_buffer.grid[0][1].ch, ' ');
        assert_eq!(overlay.screen_buffer.grid[0][2].ch, ' ');
        assert_eq!(overlay.screen_buffer.grid[0][3].ch, 'D');
    }

    #[test]
    fn test_insert_delete_lines() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("AAA\nBBB\nCCC");
        overlay.screen_buffer.cursor_row = 1;
        overlay.screen_buffer.insert_lines(1);
        assert_eq!(overlay.screen_buffer.grid[1][0].ch, ' ');
        assert_eq!(overlay.screen_buffer.grid[2][0].ch, 'B');
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
        overlay.start("vim".to_string(), "pty-123".to_string());
        assert!(overlay.is_active());
        assert_eq!(overlay.command(), Some("vim"));
        assert_eq!(overlay.pty_handle(), Some("pty-123"));
        assert!(!overlay.has_exited());
    }

    #[test]
    fn test_tui_overlay_stop() {
        let mut overlay = TuiOverlay::new();
        overlay.start("vim".to_string(), "pty-123".to_string());
        overlay.stop();
        assert!(!overlay.is_active());
        assert!(overlay.command().is_none());
        assert!(overlay.pty_handle().is_none());
        assert!(!overlay.has_exited());
    }

    #[test]
    fn test_tui_overlay_mark_exited() {
        let mut overlay = TuiOverlay::new();
        overlay.start("vim".to_string(), "pty-123".to_string());
        overlay.mark_exited();
        assert!(overlay.has_exited());
        assert!(overlay.is_active());
    }

    #[test]
    fn test_tui_overlay_add_raw_output() {
        let mut overlay = TuiOverlay::new();
        overlay.add_raw_output(b"Hello\nWorld");
    }

    #[test]
    fn test_tui_overlay_process_ansi_clear() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("Hello\x1b[2JWorld");
        let rendered = render_text(&overlay.screen_buffer);
        assert!(rendered.contains("World"));
        assert!(!rendered.contains("Hello"));
    }

    #[test]
    fn test_tui_overlay_ansi_cursor_position() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[5;10HTest");
        let rendered = render_text(&overlay.screen_buffer);
        assert!(rendered.contains("Test"));
    }

    #[test]
    fn test_tui_overlay_256_color() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[38;5;196mRed256\x1b[0m");
        let expected = AnsiColor::from_256(196);
        assert_eq!(overlay.screen_buffer.grid[0][0].style.fg, expected);
    }

    #[test]
    fn test_tui_overlay_truecolor() {
        let mut overlay = TuiOverlay::new();
        overlay.process_ansi_text("\x1b[38;2;100;150;200mTC\x1b[0m");
        assert_eq!(
            overlay.screen_buffer.grid[0][0].style.fg,
            AnsiColor::rgb(100, 150, 200)
        );
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

    #[test]
    fn test_buffer_size() {
        let overlay = TuiOverlay::new();
        let (rows, cols) = overlay.buffer_size();
        assert_eq!(rows, 50);
        assert_eq!(cols, 120);
    }
}
