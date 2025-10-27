//! Terminal viewport component
//!
//! This module provides a simple terminal viewport for displaying
//! terminal output and managing the viewing area.

use eframe::egui;

/// Terminal viewport component
pub struct TerminalViewport {
    /// Viewport configuration
    config: ViewportConfig,
    /// Current scroll position
    scroll_position: f32,
    /// Viewport dimensions
    dimensions: egui::Vec2,
}

#[derive(Debug, Clone)]
pub struct ViewportConfig {
    /// Background color
    pub background_color: egui::Color32,
    /// Border color
    pub border_color: egui::Color32,
    /// Border width
    pub border_width: f32,
    /// Padding around content
    pub padding: egui::Vec2,
    /// Enable smooth scrolling
    pub smooth_scrolling: bool,
    /// Maximum scroll speed
    pub max_scroll_speed: f32,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        Self {
            background_color: egui::Color32::from_rgb(15, 15, 25),
            border_color: egui::Color32::from_rgb(45, 45, 65),
            border_width: 1.0,
            padding: egui::Vec2::new(8.0, 8.0),
            smooth_scrolling: true,
            max_scroll_speed: 50.0,
        }
    }
}

impl TerminalViewport {
    /// Create a new terminal viewport
    pub fn new() -> Self {
        Self {
            config: ViewportConfig::default(),
            scroll_position: 0.0,
            dimensions: egui::Vec2::ZERO,
        }
    }
}

impl Default for TerminalViewport {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalViewport {
    /// Create a viewport with custom configuration
    pub fn with_config(config: ViewportConfig) -> Self {
        Self {
            config,
            scroll_position: 0.0,
            dimensions: egui::Vec2::ZERO,
        }
    }

    /// Render the viewport
    pub fn render(&mut self, ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
        // Store current dimensions
        self.dimensions = ui.available_size();

        // Create the viewport frame
        egui::Frame::none()
            .fill(self.config.background_color)
            .stroke(egui::Stroke::new(
                self.config.border_width,
                self.config.border_color,
            ))
            .inner_margin(self.config.padding)
            .show(ui, |ui| {
                // Create scrollable area for terminal content
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, content);
            });
    }

    /// Get current scroll position
    pub fn scroll_position(&self) -> f32 {
        self.scroll_position
    }

    /// Set scroll position
    pub fn set_scroll_position(&mut self, position: f32) {
        self.scroll_position = position.max(0.0);
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_position = 0.0;
    }

    /// Get viewport dimensions
    pub fn dimensions(&self) -> egui::Vec2 {
        self.dimensions
    }

    /// Check if viewport is visible
    pub fn is_visible(&self, ui: &egui::Ui) -> bool {
        let rect = ui.max_rect();
        rect.width() > 0.0 && rect.height() > 0.0
    }

    /// Get configuration
    pub fn config(&self) -> &ViewportConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut ViewportConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_creation() {
        let viewport = TerminalViewport::new();
        assert_eq!(viewport.scroll_position(), 0.0);
        assert_eq!(viewport.dimensions(), egui::Vec2::ZERO);
    }

    #[test]
    fn test_viewport_config() {
        let config = ViewportConfig::default();
        assert_eq!(config.background_color, egui::Color32::from_rgb(15, 15, 25));
        assert_eq!(config.border_color, egui::Color32::from_rgb(45, 45, 65));
        assert_eq!(config.border_width, 1.0);
    }

    #[test]
    fn test_scroll_position() {
        let mut viewport = TerminalViewport::new();

        viewport.set_scroll_position(100.0);
        assert_eq!(viewport.scroll_position(), 100.0);

        viewport.set_scroll_position(-10.0); // Should clamp to 0
        assert_eq!(viewport.scroll_position(), 0.0);

        viewport.scroll_to_bottom();
        assert_eq!(viewport.scroll_position(), 0.0);
    }
}
