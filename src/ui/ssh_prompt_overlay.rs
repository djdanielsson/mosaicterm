//! SSH prompt overlay for interactive authentication
//!
//! This module provides a small popup overlay for handling SSH interactive prompts
//! like host key verification, passphrase entry, and password authentication.

use eframe::egui;

/// Types of SSH prompts we can detect and handle
#[derive(Debug, Clone, PartialEq)]
pub enum SshPromptType {
    /// Host key verification (yes/no/fingerprint)
    HostKeyVerification,
    /// SSH key passphrase entry
    Passphrase,
    /// Password authentication
    Password,
    /// Generic interactive prompt
    Generic,
}

impl SshPromptType {
    /// Get a user-friendly title for the prompt type
    pub fn title(&self) -> &'static str {
        match self {
            SshPromptType::HostKeyVerification => "SSH Host Verification",
            SshPromptType::Passphrase => "SSH Key Passphrase",
            SshPromptType::Password => "SSH Password",
            SshPromptType::Generic => "SSH Authentication",
        }
    }

    /// Check if input should be hidden (passwords/passphrases)
    pub fn is_secret(&self) -> bool {
        matches!(self, SshPromptType::Passphrase | SshPromptType::Password)
    }
}

/// SSH prompt overlay component for handling interactive SSH prompts
pub struct SshPromptOverlay {
    /// Whether the overlay is currently active
    active: bool,
    /// The type of prompt being shown
    prompt_type: SshPromptType,
    /// The prompt message from SSH
    prompt_message: String,
    /// User input buffer
    input_buffer: String,
    /// Whether the input should be submitted
    should_submit: bool,
    /// Whether the prompt was cancelled
    was_cancelled: bool,
}

impl Default for SshPromptOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl SshPromptOverlay {
    /// Create a new SSH prompt overlay
    pub fn new() -> Self {
        Self {
            active: false,
            prompt_type: SshPromptType::Generic,
            prompt_message: String::new(),
            input_buffer: String::new(),
            should_submit: false,
            was_cancelled: false,
        }
    }

    /// Check if overlay is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Start showing the overlay with a specific prompt
    pub fn show(&mut self, prompt_type: SshPromptType, message: String) {
        self.active = true;
        self.prompt_type = prompt_type;
        self.prompt_message = message;
        self.input_buffer.clear();
        self.should_submit = false;
        self.was_cancelled = false;
    }

    /// Hide the overlay
    pub fn hide(&mut self) {
        self.active = false;
        self.prompt_message.clear();
        self.input_buffer.clear();
        self.should_submit = false;
        self.was_cancelled = false;
    }

    /// Get the input to send (if submitted) and clear it
    pub fn take_input(&mut self) -> Option<String> {
        if self.should_submit {
            self.should_submit = false;
            let input = std::mem::take(&mut self.input_buffer);
            Some(input)
        } else {
            None
        }
    }

    /// Check if the prompt was cancelled
    pub fn was_cancelled(&self) -> bool {
        self.was_cancelled
    }

    /// Render the overlay and return true if it should be closed
    pub fn render(&mut self, ctx: &egui::Context) -> bool {
        if !self.active {
            return false;
        }

        let mut should_close = false;
        let window_id = egui::Id::new("ssh_prompt_overlay");

        // Create a modal overlay
        egui::Area::new(egui::Id::new("ssh_prompt_backdrop"))
            .fixed_pos(egui::Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                ui.allocate_space(screen_rect.size());

                // Semi-transparent backdrop
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                );
            });

        // Calculate window size based on content
        let window_width = 450.0;
        let window_height = if self.prompt_type == SshPromptType::HostKeyVerification {
            280.0 // More space for the host key message
        } else {
            180.0
        };

        egui::Window::new(self.prompt_type.title())
            .id(window_id)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([window_width, window_height])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);

                // Icon and message area
                ui.horizontal(|ui| {
                    // SSH icon
                    ui.label(egui::RichText::new("🔐").size(24.0));

                    ui.vertical(|ui| {
                        // Prompt message - wrap long text
                        let message = self.prompt_message.trim();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(message)
                                    .color(egui::Color32::from_rgb(220, 220, 220)),
                            )
                            .wrap(true),
                        );
                    });
                });

                ui.add_space(8.0);

                // Input field
                let input_hint = match self.prompt_type {
                    SshPromptType::HostKeyVerification => "Type 'yes' to connect or 'no' to abort",
                    SshPromptType::Passphrase => "Enter your SSH key passphrase",
                    SshPromptType::Password => "Enter your password",
                    SshPromptType::Generic => "Enter your response",
                };

                ui.horizontal(|ui| {
                    ui.label("Response:");

                    // Use a unique ID for the text field
                    let text_field_id = egui::Id::new("ssh_prompt_input_field");

                    let response = if self.prompt_type.is_secret() {
                        // Password field
                        let text_edit = egui::TextEdit::singleline(&mut self.input_buffer)
                            .id(text_field_id)
                            .password(true)
                            .hint_text(input_hint)
                            .desired_width(window_width - 100.0)
                            .lock_focus(true); // Prevent Tab from moving focus away
                        ui.add(text_edit)
                    } else {
                        // Regular text field
                        let text_edit = egui::TextEdit::singleline(&mut self.input_buffer)
                            .id(text_field_id)
                            .hint_text(input_hint)
                            .desired_width(window_width - 100.0)
                            .lock_focus(true); // Prevent Tab from moving focus away
                        ui.add(text_edit)
                    };

                    // Always request focus while overlay is active to prevent focus stealing
                    response.request_focus();

                    // Handle Enter key - check if Enter was pressed while we have focus
                    if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.should_submit = true;
                        should_close = true;
                    }
                });

                ui.add_space(8.0);

                // Buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Cancel button
                        if ui.button("Cancel (Esc)").clicked() {
                            self.was_cancelled = true;
                            should_close = true;
                        }

                        // Submit button
                        let submit_text = match self.prompt_type {
                            SshPromptType::HostKeyVerification => "Connect",
                            _ => "Submit",
                        };
                        if ui.button(submit_text).clicked() {
                            self.should_submit = true;
                            should_close = true;
                        }
                    });
                });

                // Handle Escape key
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.was_cancelled = true;
                    should_close = true;
                }
            });

        if should_close {
            // Don't hide yet - let the caller retrieve the input first
        }

        should_close
    }

    /// Detect SSH prompts in output text and return the prompt type if found
    pub fn detect_ssh_prompt(text: &str) -> Option<(SshPromptType, String)> {
        let text_lower = text.to_lowercase();

        // Host key verification patterns
        if text_lower.contains("the authenticity of host")
            || text_lower.contains("are you sure you want to continue connecting")
            || (text_lower.contains("(yes/no")
                && (text_lower.contains("fingerprint") || text_lower.contains("?")))
        {
            // Extract the relevant message
            let message = Self::extract_prompt_message(text);
            return Some((SshPromptType::HostKeyVerification, message));
        }

        // SSH key passphrase patterns
        if text_lower.contains("enter passphrase for key") || text_lower.contains("passphrase for")
        {
            let message = Self::extract_prompt_message(text);
            return Some((SshPromptType::Passphrase, message));
        }

        // Password authentication patterns
        // Be careful to only match actual password prompts, not just any line containing "password"
        let password_patterns = ["password:", "password for", "'s password:"];

        for pattern in &password_patterns {
            if text_lower.contains(pattern) {
                // Make sure this looks like an actual prompt (ends with : or is at end of text)
                if text.trim().ends_with(':') || text.trim().ends_with("? ") {
                    let message = Self::extract_prompt_message(text);
                    return Some((SshPromptType::Password, message));
                }
            }
        }

        None
    }

    /// Extract a clean prompt message from raw terminal output
    fn extract_prompt_message(text: &str) -> String {
        // Clean up ANSI codes and extra whitespace
        let cleaned = strip_ansi_codes(text);

        // Take the last few lines that contain the actual prompt
        let lines: Vec<&str> = cleaned.lines().filter(|l| !l.trim().is_empty()).collect();

        if lines.len() <= 5 {
            lines.join("\n")
        } else {
            // Take last 5 lines for long messages
            lines[lines.len() - 5..].join("\n")
        }
    }
}

/// Strip ANSI escape codes from text
fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ANSI sequence
            if chars.peek() == Some(&'[') {
                chars.next();
                // Skip until we hit a letter
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence - skip until BEL or ST
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_prompt_overlay_new() {
        let overlay = SshPromptOverlay::new();
        assert!(!overlay.is_active());
        assert!(overlay.prompt_message.is_empty());
        assert!(overlay.input_buffer.is_empty());
    }

    #[test]
    fn test_ssh_prompt_overlay_show() {
        let mut overlay = SshPromptOverlay::new();
        overlay.show(SshPromptType::Password, "Enter password:".to_string());
        assert!(overlay.is_active());
        assert_eq!(overlay.prompt_message, "Enter password:");
        assert!(overlay.input_buffer.is_empty());
    }

    #[test]
    fn test_ssh_prompt_overlay_hide() {
        let mut overlay = SshPromptOverlay::new();
        overlay.show(SshPromptType::Password, "Enter password:".to_string());
        overlay.hide();
        assert!(!overlay.is_active());
        assert!(overlay.prompt_message.is_empty());
    }

    #[test]
    fn test_detect_host_key_verification() {
        let output = "The authenticity of host 'example.com (1.2.3.4)' can't be established.\nRSA key fingerprint is SHA256:abc123.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
        let result = SshPromptOverlay::detect_ssh_prompt(output);
        assert!(result.is_some());
        let (prompt_type, _) = result.unwrap();
        assert_eq!(prompt_type, SshPromptType::HostKeyVerification);
    }

    #[test]
    fn test_detect_passphrase() {
        let output = "Enter passphrase for key '/home/user/.ssh/id_rsa': ";
        let result = SshPromptOverlay::detect_ssh_prompt(output);
        assert!(result.is_some());
        let (prompt_type, _) = result.unwrap();
        assert_eq!(prompt_type, SshPromptType::Passphrase);
    }

    #[test]
    fn test_detect_password() {
        let output = "user@example.com's password: ";
        let result = SshPromptOverlay::detect_ssh_prompt(output);
        assert!(result.is_some());
        let (prompt_type, _) = result.unwrap();
        assert_eq!(prompt_type, SshPromptType::Password);
    }

    #[test]
    fn test_detect_password_with_colon() {
        let output = "Password: ";
        let result = SshPromptOverlay::detect_ssh_prompt(output);
        assert!(result.is_some());
        let (prompt_type, _) = result.unwrap();
        assert_eq!(prompt_type, SshPromptType::Password);
    }

    #[test]
    fn test_no_false_positive() {
        // This should NOT be detected as an SSH prompt
        let output = "The password was changed successfully.";
        let result = SshPromptOverlay::detect_ssh_prompt(output);
        assert!(result.is_none());
    }

    #[test]
    fn test_prompt_type_is_secret() {
        assert!(!SshPromptType::HostKeyVerification.is_secret());
        assert!(SshPromptType::Passphrase.is_secret());
        assert!(SshPromptType::Password.is_secret());
        assert!(!SshPromptType::Generic.is_secret());
    }

    #[test]
    fn test_strip_ansi_codes() {
        let input = "\x1b[32mGreen text\x1b[0m normal";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Green text normal");
    }
}
