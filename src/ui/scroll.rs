//! Scrollable history area
//!
//! This module manages the scrollable history region that displays
//! past command blocks in the MosaicTerm interface.

use eframe::egui;
use std::collections::HashMap;
use crate::models::CommandBlock;

/// Scrollable history component
pub struct ScrollableHistory {
    /// Current scroll position
    scroll_position: f32,
    /// Target scroll position for animation
    target_scroll_position: f32,
    /// Total content height
    total_height: f32,
    /// Viewport height
    viewport_height: f32,
    /// Scroll velocity for smooth scrolling
    scroll_velocity: f32,
    /// Scroll momentum
    scroll_momentum: f32,
    /// Whether smooth scrolling is enabled
    smooth_scrolling: bool,
    /// Whether we're currently animating
    is_animating: bool,
    /// Animation duration
    animation_duration: f32,
    /// Current animation time
    animation_time: f32,
    /// Scroll bar configuration
    scrollbar_config: ScrollbarConfig,
    /// Cached block heights for performance
    block_heights: HashMap<String, f32>,
}

#[derive(Debug, Clone)]
pub struct ScrollbarConfig {
    /// Width of the scrollbar
    pub width: f32,
    /// Background color
    pub background_color: egui::Color32,
    /// Handle color
    pub handle_color: egui::Color32,
    /// Handle hover color
    pub handle_hover_color: egui::Color32,
    /// Show scrollbar only when needed
    pub auto_hide: bool,
}

#[derive(Debug, Clone)]
pub struct ScrollState {
    /// Current scroll position (0.0 = top, 1.0 = bottom)
    pub position: f32,
    /// Whether the content is being scrolled
    pub scrolling: bool,
    /// Whether the scrollbar is visible
    pub scrollbar_visible: bool,
    /// Content offset in pixels
    pub content_offset: f32,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            width: 12.0,
            background_color: egui::Color32::from_rgb(40, 40, 50),
            handle_color: egui::Color32::from_rgb(80, 80, 100),
            handle_hover_color: egui::Color32::from_rgb(100, 100, 120),
            auto_hide: true,
        }
    }
}

impl ScrollableHistory {
    /// Create a new scrollable history
    pub fn new() -> Self {
        Self {
            scroll_position: 0.0,
            target_scroll_position: 0.0,
            total_height: 0.0,
            viewport_height: 0.0,
            scroll_velocity: 0.0,
            scroll_momentum: 0.0,
            smooth_scrolling: true,
            is_animating: false,
            animation_duration: 0.3, // 300ms animation
            animation_time: 0.0,
            scrollbar_config: ScrollbarConfig::default(),
            block_heights: HashMap::new(),
        }
    }
}

impl Default for ScrollableHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollableHistory {
    /// Create with custom configuration
    pub fn with_config(scrollbar_config: ScrollbarConfig) -> Self {
        Self {
            scrollbar_config,
            ..Self::new()
        }
    }

    /// Render the scrollable history area
    pub fn render(&mut self, ui: &mut egui::Ui, blocks: &[String]) {
        // Calculate total content height
        self.calculate_total_height(blocks);

        // Create scroll area
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.render_content(ui, blocks);
            });

        // Render custom scrollbar if needed
        if self.needs_scrollbar() {
            self.render_scrollbar(ui);
        }
    }

    /// Render command blocks with enhanced scrolling
    /// Returns information about context menu state: (block_id, position) if a context menu should be shown
    pub fn render_command_blocks(&mut self, ui: &mut egui::Ui, command_blocks: &[CommandBlock], blocks_renderer: &mut crate::ui::CommandBlocks) -> Option<(String, egui::Pos2)> {
        // Calculate total content height for command blocks
        self.calculate_command_blocks_height(command_blocks);

        // Create enhanced scroll area with momentum and smooth scrolling
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true);

        // Handle mouse wheel events for smooth scrolling
        ui.input(|input| {
            if input.scroll_delta.y != 0.0 {
                self.handle_mouse_wheel(input.scroll_delta);
            }
        });

        let mut context_menu_info = None;


        scroll_area.show(ui, |ui| {
            ui.vertical(|ui| {
                if command_blocks.is_empty() {
                    blocks_renderer.render_placeholder(ui);
                    return;
                }

                // Render each command block with enhanced styling
                for (index, block) in command_blocks.iter().enumerate() {
                    blocks_renderer.render_block(ui, block).ok(); // Ignore render errors for now

                    // Add spacing between blocks
                    if index < command_blocks.len() - 1 {
                        ui.add_space(4.0);
                    }
                }
            });
        });

        // Render custom scrollbar if needed
        if self.needs_scrollbar() {
            self.render_scrollbar(ui);
        }

        // Check if any block should show a context menu
        for block in command_blocks {
            if blocks_renderer.should_show_context_menu(block) {
                if let Some(pos) = blocks_renderer.context_menu_pos() {
                    context_menu_info = Some((block.id.clone(), pos));
                    break;
                }
            }
        }

        // Update scroll animation
        ui.input(|input| {
            self.update_scroll_animation(input.stable_dt);
        });

        context_menu_info
    }

    /// Render the scrollable content
    fn render_content(&mut self, ui: &mut egui::Ui, blocks: &[String]) {
        ui.vertical(|ui| {
            for (index, block) in blocks.iter().enumerate() {
                self.render_block_item(ui, block, index);
            }

            // Add some padding at the bottom
            ui.add_space(20.0);
        });
    }

    /// Render a single block item
    fn render_block_item(&mut self, ui: &mut egui::Ui, block: &str, index: usize) {
        let block_id = format!("block_{}", index);

        // Get or calculate block height
        let height = self.get_block_height(&block_id, block);

        // Block container
        let block_rect = ui.available_rect_before_wrap();
        let block_response = ui.allocate_rect(
            egui::Rect::from_min_size(
                block_rect.min,
                egui::vec2(block_rect.width(), height)
            ),
            egui::Sense::click()
        );

        // Handle hover and selection
        let is_hovered = block_response.hovered();

        // Block background
        let bg_color = if is_hovered {
            egui::Color32::from_rgb(45, 45, 55)
        } else {
            egui::Color32::from_rgb(35, 35, 45)
        };

        ui.painter().rect_filled(block_response.rect, 4.0, bg_color);

        // Block border
        let border_color = if is_hovered {
            egui::Color32::from_rgb(80, 80, 100)
        } else {
            egui::Color32::from_rgb(60, 60, 80)
        };

        ui.painter().rect_stroke(block_response.rect, 4.0, egui::Stroke::new(1.0, border_color));

        // Block content
        ui.allocate_ui_at_rect(block_response.rect.shrink(8.0), |ui| {
            ui.vertical(|ui| {
                // Block header with index
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("[{}]", index + 1))
                        .font(egui::FontId::monospace(10.0))
                        .color(egui::Color32::LIGHT_GRAY));

                    ui.label(egui::RichText::new(block)
                        .font(egui::FontId::monospace(12.0))
                        .color(egui::Color32::WHITE));
                });
            });
        });

        // Handle click for selection
        if block_response.clicked() {
            // Could implement block selection/expansion
        }
    }

    /// Render custom scrollbar
    fn render_scrollbar(&mut self, ui: &mut egui::Ui) {
        let available_rect = ui.available_rect_before_wrap();

        // Scrollbar area
        let scrollbar_rect = egui::Rect::from_min_max(
            egui::pos2(available_rect.max.x - self.scrollbar_config.width, available_rect.min.y),
            available_rect.max
        );

        // Scrollbar background
        ui.painter().rect_filled(
            scrollbar_rect,
            0.0,
            self.scrollbar_config.background_color
        );

        // Calculate handle size and position
        let handle_height = (self.viewport_height / self.total_height).min(1.0) * scrollbar_rect.height();
        let handle_y = self.scroll_position * (scrollbar_rect.height() - handle_height);

        let handle_rect = egui::Rect::from_min_max(
            egui::pos2(scrollbar_rect.min.x, scrollbar_rect.min.y + handle_y),
            egui::pos2(scrollbar_rect.max.x, scrollbar_rect.min.y + handle_y + handle_height)
        );

        // Handle interaction
        let handle_response = ui.allocate_rect(handle_rect, egui::Sense::drag());
        let handle_color = if handle_response.hovered() {
            self.scrollbar_config.handle_hover_color
        } else {
            self.scrollbar_config.handle_color
        };

        // Draw handle
        ui.painter().rect_filled(handle_rect, 4.0, handle_color);

        // Handle drag
        if let Some(drag_delta) = handle_response.interact_pointer_pos() {
            // Simplified drag handling - would need more complex logic for full implementation
            let _scroll_delta = drag_delta.y / (scrollbar_rect.height() - handle_height);
            // self.scroll_position = (self.scroll_position + scroll_delta).clamp(0.0, 1.0);
        }
    }

    /// Calculate total content height
    fn calculate_total_height(&mut self, blocks: &[String]) {
        self.total_height = 0.0;

        for (index, block) in blocks.iter().enumerate() {
            let block_id = format!("block_{}", index);
            let height = self.get_block_height(&block_id, block);
            self.total_height += height;
        }

        // Add spacing between blocks
        if !blocks.is_empty() {
            self.total_height += (blocks.len() - 1) as f32 * 4.0; // 4px spacing
        }
    }

    /// Calculate total content height for command blocks
    fn calculate_command_blocks_height(&mut self, command_blocks: &[CommandBlock]) {
        self.total_height = 0.0;

        for (index, block) in command_blocks.iter().enumerate() {
            let block_id = format!("cmd_block_{}", index);
            let height = self.get_command_block_height(&block_id, block);
            self.total_height += height;
        }

        // Add spacing between blocks
        if !command_blocks.is_empty() {
            self.total_height += (command_blocks.len() - 1) as f32 * 4.0; // 4px spacing
        }
    }

    /// Get cached command block height or calculate it
    fn get_command_block_height(&mut self, block_id: &str, block: &CommandBlock) -> f32 {
        if let Some(&height) = self.block_heights.get(block_id) {
            height
        } else {
            let height = self.calculate_command_block_height(block);
            self.block_heights.insert(block_id.to_string(), height);
            height
        }
    }

    /// Calculate height for a command block
    fn calculate_command_block_height(&self, block: &CommandBlock) -> f32 {
        // Estimate height based on command and output content
        let command_lines = (block.command.len() as f32 / 80.0).ceil().max(1.0);
        let output_lines = block.output.iter()
            .map(|line| (line.text.len() as f32 / 80.0).ceil().max(1.0))
            .sum::<f32>();

        let total_lines = command_lines + output_lines;
        let base_height = 40.0; // Header height
        let line_height = 16.0; // Height per line
        let padding = 24.0; // Top and bottom padding

        base_height + (total_lines * line_height) + padding
    }

    /// Get cached block height or calculate it
    fn get_block_height(&mut self, block_id: &str, block: &str) -> f32 {
        if let Some(&height) = self.block_heights.get(block_id) {
            height
        } else {
            let height = self.calculate_block_height(block);
            self.block_heights.insert(block_id.to_string(), height);
            height
        }
    }

    /// Calculate height for a block based on its content
    fn calculate_block_height(&self, block: &str) -> f32 {
        // Estimate height based on text length
        let lines = block.lines().count().max(1);
        let base_height = 24.0; // Minimum height
        let line_height = 16.0; // Height per line
        let padding = 16.0; // Top and bottom padding

        base_height + (lines as f32 * line_height) + padding
    }

    /// Check if scrollbar is needed
    fn needs_scrollbar(&self) -> bool {
        self.total_height > self.viewport_height
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        if self.smooth_scrolling {
            self.animate_to_position(0.0);
        } else {
            self.scroll_position = 0.0;
            self.scroll_velocity = 0.0;
        }
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        let bottom_pos = 1.0;
        if self.smooth_scrolling {
            self.animate_to_position(bottom_pos);
        } else {
            self.scroll_position = bottom_pos;
            self.scroll_velocity = 0.0;
        }
    }

    /// Scroll to specific position (0.0 = top, 1.0 = bottom)
    pub fn scroll_to(&mut self, position: f32) {
        let clamped_pos = position.clamp(0.0, 1.0);
        if self.smooth_scrolling {
            self.animate_to_position(clamped_pos);
        } else {
            self.scroll_position = clamped_pos;
            self.scroll_velocity = 0.0;
        }
    }

    /// Scroll by a delta amount
    pub fn scroll_by(&mut self, delta: f32) {
        if self.smooth_scrolling {
            let target_pos = (self.scroll_position + delta / self.total_height)
                .clamp(0.0, 1.0);
            self.animate_to_position(target_pos);
        } else {
            let position_delta = delta / self.total_height;
            self.scroll_position = (self.scroll_position + position_delta).clamp(0.0, 1.0);
        }
    }

    /// Animate to target position
    pub fn animate_to_position(&mut self, target_position: f32) {
        self.target_scroll_position = target_position;
        self.is_animating = true;
        self.animation_time = 0.0;
    }

    /// Handle mouse wheel scrolling with momentum
    pub fn handle_mouse_wheel(&mut self, delta: egui::Vec2) {
        if delta.y != 0.0 {
            // Mouse wheel provides momentum
            let scroll_delta = delta.y * 20.0; // Scale wheel delta
            self.scroll_momentum += scroll_delta;

            // Apply momentum immediately
            self.scroll_by(scroll_delta);
        }
    }

    /// Handle mouse drag scrolling
    pub fn handle_mouse_drag(&mut self, _ui: &egui::Ui, drag_delta: egui::Vec2) {
        if drag_delta.y != 0.0 {
            let scroll_delta = drag_delta.y * 0.5; // Scale drag delta
            self.scroll_by(scroll_delta);
        }
    }

    /// Update scroll animation (call this in your main loop)
    pub fn update_scroll_animation(&mut self, delta_time: f32) {
        // Update smooth scrolling animation
        if self.is_animating {
            self.animation_time += delta_time;

            let progress = (self.animation_time / self.animation_duration).min(1.0);

            // Easing function (ease-out cubic)
            let eased_progress = 1.0 - (1.0 - progress).powi(3);

            // Interpolate between current and target position
            let start_pos = self.scroll_position;
            let end_pos = self.target_scroll_position;
            self.scroll_position = start_pos + (end_pos - start_pos) * eased_progress;

            // Check if animation is complete
            if progress >= 1.0 {
                self.is_animating = false;
                self.scroll_position = self.target_scroll_position;
                self.animation_time = 0.0;
            }
        }

        // Update smooth scrolling with velocity and momentum
        if self.smooth_scrolling {
            // Apply velocity
            self.scroll_position += self.scroll_velocity * delta_time;
            self.scroll_position = self.scroll_position.clamp(0.0, 1.0);

            // Apply momentum decay
            if self.scroll_momentum.abs() > 0.1 {
                self.scroll_position += self.scroll_momentum * delta_time * 0.01;
                self.scroll_momentum *= 0.95; // Slower decay for momentum
                self.scroll_position = self.scroll_position.clamp(0.0, 1.0);
            } else {
                self.scroll_momentum = 0.0;
            }

            // Apply friction to velocity
            self.scroll_velocity *= 0.9;

            // Stop if velocity is very small
            if self.scroll_velocity.abs() < 0.001 {
                self.scroll_velocity = 0.0;
            }
        }
    }

    /// Set viewport height
    pub fn set_viewport_height(&mut self, height: f32) {
        self.viewport_height = height;
    }

    /// Get current scroll state
    pub fn scroll_state(&self) -> ScrollState {
        ScrollState {
            position: self.scroll_position,
            scrolling: self.scroll_velocity.abs() > 0.01,
            scrollbar_visible: self.needs_scrollbar(),
            content_offset: self.scroll_position * (self.total_height - self.viewport_height).max(0.0),
        }
    }

    /// Clear cached heights
    pub fn clear_cache(&mut self) {
        self.block_heights.clear();
    }

    /// Set scrollbar configuration
    pub fn set_scrollbar_config(&mut self, config: ScrollbarConfig) {
        self.scrollbar_config = config;
    }

    /// Enable/disable smooth scrolling
    pub fn set_smooth_scrolling(&mut self, enabled: bool) {
        self.smooth_scrolling = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollable_history_creation() {
        let history = ScrollableHistory::new();
        assert_eq!(history.scroll_position, 0.0);
        assert_eq!(history.scroll_velocity, 0.0);
        assert!(history.smooth_scrolling);
    }

    #[test]
    fn test_scrollbar_config_defaults() {
        let config = ScrollbarConfig::default();
        assert_eq!(config.width, 12.0);
        assert!(config.auto_hide);
    }

    #[test]
    fn test_scroll_to_positions() {
        let mut history = ScrollableHistory::new();
        history.smooth_scrolling = false; // Disable smooth scrolling for immediate position update

        history.scroll_to(0.5);
        assert_eq!(history.scroll_position, 0.5);

        history.scroll_to_top();
        assert_eq!(history.scroll_position, 0.0);

        history.scroll_to_bottom();
        assert_eq!(history.scroll_position, 1.0);
    }

    #[test]
    fn test_scroll_by() {
        let mut history = ScrollableHistory::new();
        history.smooth_scrolling = false; // Disable smooth scrolling for immediate position update
        history.total_height = 1000.0;
        history.viewport_height = 100.0;

        history.scroll_by(50.0);
        assert_eq!(history.scroll_position, 0.05);
    }

    #[test]
    fn test_scroll_animation() {
        let mut history = ScrollableHistory::new();
        history.scroll_velocity = 1.0;

        history.update_scroll_animation(0.1);
        assert!(history.scroll_velocity < 1.0);
    }

    #[test]
    fn test_clear_cache() {
        let mut history = ScrollableHistory::new();
        history.block_heights.insert("test".to_string(), 50.0);

        assert_eq!(history.block_heights.len(), 1);
        history.clear_cache();
        assert_eq!(history.block_heights.len(), 0);
    }

    #[test]
    fn test_scroll_state() {
        let mut history = ScrollableHistory::new();
        history.total_height = 1000.0;
        history.viewport_height = 100.0;

        let state = history.scroll_state();
        assert_eq!(state.position, 0.0);
        assert!(!state.scrolling);
        assert!(state.scrollbar_visible);
    }
}
