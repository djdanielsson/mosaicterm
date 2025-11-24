//! Input prompt component
//!
//! This module handles the pinned input prompt at the bottom
//! of the MosaicTerm interface.

use eframe::egui;
use std::collections::VecDeque;
use tracing::debug;

/// Input prompt component
pub struct InputPrompt {
    /// Current input text
    current_input: String,
    /// Input history
    history: VecDeque<String>,
    /// Current history position (None = not browsing history)
    history_position: Option<usize>,
    /// Cursor position in the input
    cursor_position: usize,
    /// Whether the prompt is focused
    focused: bool,
    /// Prompt text (e.g., "$ ", ">>> ", etc.)
    prompt_text: String,
    /// Maximum history size
    max_history: usize,
    /// Flag to request focus on next render (for completion)
    request_focus: bool,
}

#[derive(Debug, Clone)]
pub struct InputConfig {
    /// Maximum input length
    pub max_length: usize,
    /// Font size for input text
    pub font_size: f32,
    /// Input field height
    pub height: f32,
    /// Background color
    pub background_color: egui::Color32,
    /// Text color
    pub text_color: egui::Color32,
    /// Cursor color
    pub cursor_color: egui::Color32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            max_length: 1000,
            font_size: 12.0,
            height: 30.0,
            background_color: egui::Color32::from_rgb(25, 25, 35),
            text_color: egui::Color32::WHITE,
            cursor_color: egui::Color32::from_rgb(100, 150, 255),
        }
    }
}

impl InputPrompt {
    /// Create a new input prompt
    pub fn new() -> Self {
        Self {
            current_input: String::new(),
            history: VecDeque::new(),
            history_position: None,
            cursor_position: 0,
            focused: true,
            prompt_text: "$ ".to_string(),
            max_history: 100,
            request_focus: false,
        }
    }
}

impl Default for InputPrompt {
    fn default() -> Self {
        Self::new()
    }
}

impl InputPrompt {
    /// Create with custom prompt text
    pub fn with_prompt(prompt: &str) -> Self {
        Self {
            prompt_text: prompt.to_string(),
            ..Self::new()
        }
    }

    /// Create with custom configuration
    pub fn with_config(_config: InputConfig) -> Self {
        Self {
            current_input: String::new(),
            history: VecDeque::new(),
            history_position: None,
            cursor_position: 0,
            focused: true,
            prompt_text: "$ ".to_string(),
            max_history: 100,
            request_focus: false,
        }
    }

    /// Render the input prompt with enhanced styling
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut submitted_command = None;

        // Create a styled frame for the input area with better positioning
        let input_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgba_premultiplied(20, 20, 30, 220))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 90)))
            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
            .outer_margin(egui::Margin::symmetric(8.0, 6.0))
            .rounding(egui::Rounding::same(6.0));

        input_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Enhanced prompt text with better visual hierarchy
                ui.label(
                    egui::RichText::new(&self.prompt_text)
                        .font(egui::FontId::monospace(14.0))
                        .color(egui::Color32::from_rgb(120, 230, 120))
                        .strong(),
                );

                ui.add_space(12.0);

                // Enhanced input field with better styling
                let input_response = ui.add(
                    egui::TextEdit::singleline(&mut self.current_input)
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(f32::INFINITY)
                        .hint_text("Enter command...")
                        .margin(egui::Vec2::new(10.0, 8.0))
                        .text_color_opt(Some(egui::Color32::from_rgb(220, 220, 240))),
                );

                // Request focus and move cursor to end if needed (e.g., after completion)
                if self.request_focus {
                    input_response.request_focus();
                    // Force cursor to the stored position by modifying the widget state
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), input_response.id)
                    {
                        let ccursor = egui::text::CCursor::new(self.cursor_position);
                        state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), input_response.id);
                    }
                    self.request_focus = false;
                }

                // Enhanced visual feedback for focused state
                if input_response.has_focus() {
                    // Draw a glowing focus indicator
                    let painter = ui.painter();
                    let rect = input_response.rect.expand(2.0);
                    painter.rect_stroke(
                        rect,
                        4.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                    );

                    // Add subtle glow effect
                    painter.rect_stroke(
                        rect.expand(1.0),
                        4.0,
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_premultiplied(100, 150, 255, 100),
                        ),
                    );
                }

                // Handle input events
                if input_response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !self.current_input.trim().is_empty()
                {
                    submitted_command = Some(self.current_input.clone());
                    self.add_to_history(self.current_input.clone());
                    self.current_input.clear();
                    self.cursor_position = 0;
                    self.history_position = None;
                }

                // Handle arrow keys for history navigation
                if input_response.has_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.navigate_history_previous();
                    } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.navigate_history_next();
                    }
                }
            });

            // Enhanced history hint with better positioning
            if ui.memory(|mem| mem.focus().is_some()) {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("💡")
                            .font(egui::FontId::proportional(13.0))
                            .color(egui::Color32::from_rgb(160, 160, 180)),
                    );

                    ui.add_space(6.0);

                    ui.label(
                        egui::RichText::new("Use ↑↓ arrows to navigate command history")
                            .font(egui::FontId::proportional(12.0))
                            .color(egui::Color32::from_rgb(160, 160, 180)),
                    );
                });

                // Show current history position if browsing
                if let Some(pos) = self.history_position {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "📜 History: {}/{}",
                                pos + 1,
                                self.history.len()
                            ))
                            .font(egui::FontId::proportional(11.0))
                            .color(egui::Color32::from_rgb(140, 140, 160)),
                        );
                    });
                }
            }
        });

        submitted_command
    }

    /// Add command to history
    pub fn add_to_history(&mut self, command: String) {
        if !command.trim().is_empty() {
            debug!("Adding command to history: {}", command);

            // Remove the command if it already exists (to avoid duplicates in different positions)
            // but add it again at the end as the most recent command
            if let Some(pos) = self.history.iter().position(|c| c == &command) {
                self.history.remove(pos);
                debug!("Removed duplicate at position: {}", pos);
            }

            self.history.push_back(command.clone());
            debug!("History size after add: {}", self.history.len());
            debug!("History contents: {:?}", self.history);

            // Maintain history size limit
            while self.history.len() > self.max_history {
                self.history.pop_front();
            }

            // Reset history position when adding new command
            self.history_position = None;
        }
    }

    /// Navigate to previous command in history
    pub fn navigate_history_previous(&mut self) {
        if self.history.is_empty() {
            debug!("History is empty!");
            return;
        }

        debug!(
            "History size: {}, Current position: {:?}",
            self.history.len(),
            self.history_position
        );
        debug!("History contents: {:?}", self.history);

        let position = match self.history_position {
            None => self.history.len().saturating_sub(1),
            Some(pos) if pos > 0 => pos - 1,
            _ => return,
        };

        debug!("Moving to position: {}", position);

        if let Some(command) = self.history.get(position) {
            self.current_input = command.clone();
            self.cursor_position = command.len();
            self.history_position = Some(position);
            debug!("Set input to: {}", command);
        }
    }

    /// Navigate to next command in history
    pub fn navigate_history_next(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let position = match self.history_position {
            Some(pos) if pos < self.history.len() - 1 => pos + 1,
            _ => {
                // End of history, clear input
                self.current_input.clear();
                self.cursor_position = 0;
                self.history_position = None;
                return;
            }
        };

        if let Some(command) = self.history.get(position) {
            self.current_input = command.clone();
            self.cursor_position = command.len();
            self.history_position = Some(position);
        }
    }

    /// Clear current input
    pub fn clear_input(&mut self) {
        self.current_input.clear();
        self.cursor_position = 0;
        self.history_position = None;
    }

    /// Set current input text
    pub fn set_input(&mut self, text: String) {
        // Only reset history position if the text actually changed from user input
        // Don't reset it if we're just syncing the same text back
        if self.current_input != text {
            // If we're browsing history, check if the change is from user editing
            if self.history_position.is_some() {
                // User is editing while browsing history, reset position
                self.history_position = None;
            }
        }
        self.current_input = text;
        self.cursor_position = self.current_input.len();
        self.request_focus = true; // Request focus to move cursor to end
    }

    /// Set current input text and explicit cursor position
    pub fn set_input_with_cursor(&mut self, text: String, cursor_pos: usize) {
        // Only reset history position if the text actually changed from user input
        // Don't reset it if we're just syncing the same text back
        if self.current_input != text {
            // If we're browsing history, check if the change is from user editing
            if self.history_position.is_some() {
                // User is editing while browsing history, reset position
                self.history_position = None;
            }
        }
        self.current_input = text;
        self.cursor_position = cursor_pos.min(self.current_input.len());
        self.request_focus = true; // Request focus to update cursor position
    }

    /// Set prompt text
    pub fn set_prompt(&mut self, prompt: &str) {
        self.prompt_text = prompt.to_string();
    }

    /// Get current prompt text
    pub fn prompt_text(&self) -> &str {
        &self.prompt_text
    }

    /// Get current input text
    pub fn current_input(&self) -> &str {
        &self.current_input
    }

    /// Get input history
    pub fn history(&self) -> &VecDeque<String> {
        &self.history
    }

    /// Check if input is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Get cursor position
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }

    /// Check if currently browsing history
    pub fn is_browsing_history(&self) -> bool {
        self.history_position.is_some()
    }

    /// Get current history position
    pub fn history_position(&self) -> Option<usize> {
        self.history_position
    }
}

/// Input handling utilities
pub mod utils {
    use super::*;

    /// Validate input before submission
    pub fn validate_input(input: &str) -> Result<(), String> {
        if input.trim().is_empty() {
            return Err("Input cannot be empty".to_string());
        }

        if input.len() > 10000 {
            return Err("Input too long (max 10000 characters)".to_string());
        }

        // Check for potentially dangerous commands
        let dangerous_patterns = [
            "rm -rf /",
            "format",
            "fdisk",
            ":(){ :|:& };:", // Fork bomb
        ];

        for pattern in &dangerous_patterns {
            if input.contains(pattern) {
                return Err(format!(
                    "Potentially dangerous command detected: {}",
                    pattern
                ));
            }
        }

        Ok(())
    }

    /// Sanitize input text
    pub fn sanitize_input(input: &str) -> String {
        // Remove null bytes and other problematic characters
        input
            .chars()
            .filter(|&c| c != '\0' && !c.is_control() || c == '\n' || c == '\t' || c == '\r')
            .collect()
    }

    /// Get suggested completions for input
    pub fn get_completions(input: &str, history: &VecDeque<String>) -> Vec<String> {
        if input.is_empty() {
            return Vec::new();
        }

        history
            .iter()
            .filter(|cmd| cmd.starts_with(input))
            .take(10)
            .cloned()
            .collect()
    }

    /// Format prompt based on shell type
    pub fn format_prompt(shell_type: &str) -> String {
        match shell_type {
            "bash" => "$ ".to_string(),
            "zsh" => "% ".to_string(),
            "fish" => "> ".to_string(),
            "powershell" => "PS> ".to_string(),
            _ => "$ ".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_prompt_creation() {
        let prompt = InputPrompt::new();
        assert_eq!(prompt.current_input(), "");
        assert_eq!(prompt.cursor_position(), 0);
        assert_eq!(prompt.prompt_text, "$ ");
        assert!(!prompt.is_browsing_history());
    }

    #[test]
    fn test_input_prompt_with_prompt() {
        let prompt = InputPrompt::with_prompt(">>> ");
        assert_eq!(prompt.prompt_text, ">>> ");
    }

    #[test]
    fn test_add_to_history() {
        let mut prompt = InputPrompt::new();

        prompt.add_to_history("echo hello".to_string());
        prompt.add_to_history("ls -la".to_string());
        prompt.add_to_history("echo hello".to_string()); // Duplicate, should be moved to end

        assert_eq!(prompt.history().len(), 2);
        // After adding duplicate "echo hello", it should be moved to the end
        assert_eq!(prompt.history()[0], "ls -la");
        assert_eq!(prompt.history()[1], "echo hello");
    }

    #[test]
    fn test_history_navigation() {
        let mut prompt = InputPrompt::new();
        prompt.add_to_history("cmd1".to_string());
        prompt.add_to_history("cmd2".to_string());
        prompt.add_to_history("cmd3".to_string());

        // Navigate up
        prompt.navigate_history_previous();
        assert_eq!(prompt.current_input(), "cmd3");
        assert_eq!(prompt.history_position(), Some(2));

        prompt.navigate_history_previous();
        assert_eq!(prompt.current_input(), "cmd2");
        assert_eq!(prompt.history_position(), Some(1));

        prompt.navigate_history_previous();
        assert_eq!(prompt.current_input(), "cmd1");
        assert_eq!(prompt.history_position(), Some(0));

        // Try to go beyond start
        prompt.navigate_history_previous();
        assert_eq!(prompt.current_input(), "cmd1");
        assert_eq!(prompt.history_position(), Some(0));

        // Navigate down
        prompt.navigate_history_next();
        assert_eq!(prompt.current_input(), "cmd2");
        assert_eq!(prompt.history_position(), Some(1));

        prompt.navigate_history_next();
        assert_eq!(prompt.current_input(), "cmd3");
        assert_eq!(prompt.history_position(), Some(2));

        prompt.navigate_history_next();
        assert_eq!(prompt.current_input(), "");
        assert_eq!(prompt.history_position(), None);
    }

    #[test]
    fn test_clear_input() {
        let mut prompt = InputPrompt::new();
        prompt.set_input("test command".to_string());
        prompt.navigate_history_previous(); // Set history position

        prompt.clear_input();
        assert_eq!(prompt.current_input(), "");
        assert_eq!(prompt.cursor_position(), 0);
        assert_eq!(prompt.history_position(), None);
    }

    #[test]
    fn test_set_input() {
        let mut prompt = InputPrompt::new();
        prompt.set_input("new command".to_string());

        assert_eq!(prompt.current_input(), "new command");
        assert_eq!(prompt.cursor_position(), 11); // "new command" is 11 chars
        assert_eq!(prompt.history_position(), None);
    }

    #[test]
    fn test_set_prompt() {
        let mut prompt = InputPrompt::new();
        prompt.set_prompt(">>> ");

        assert_eq!(prompt.prompt_text, ">>> ");
    }

    #[test]
    fn test_utils_validate_input() {
        assert!(utils::validate_input("echo hello").is_ok());
        assert!(utils::validate_input("").is_err());
        assert!(utils::validate_input("rm -rf /").is_err());
    }

    #[test]
    fn test_utils_sanitize_input() {
        let result = utils::sanitize_input("echo\x00hello\tworld");
        assert_eq!(result, "echohello\tworld");
    }

    #[test]
    fn test_utils_get_completions() {
        let mut history = VecDeque::new();
        history.push_back("echo hello".to_string());
        history.push_back("echo world".to_string());
        history.push_back("ls -la".to_string());

        let completions = utils::get_completions("echo", &history);
        assert_eq!(completions.len(), 2);
        assert!(completions.contains(&"echo hello".to_string()));
        assert!(completions.contains(&"echo world".to_string()));
    }

    #[test]
    fn test_utils_format_prompt() {
        assert_eq!(utils::format_prompt("bash"), "$ ");
        assert_eq!(utils::format_prompt("zsh"), "% ");
        assert_eq!(utils::format_prompt("fish"), "> ");
        assert_eq!(utils::format_prompt("powershell"), "PS> ");
        assert_eq!(utils::format_prompt("unknown"), "$ ");
    }

    #[test]
    fn test_input_config_defaults() {
        let config = InputConfig::default();
        assert_eq!(config.max_length, 1000);
        assert_eq!(config.font_size, 12.0);
        assert_eq!(config.height, 30.0);
    }
}
