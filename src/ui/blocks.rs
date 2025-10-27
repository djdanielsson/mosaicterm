//! Command block rendering
//!
//! This module handles the rendering of individual command blocks
//! in the MosaicTerm interface.

use eframe::egui;
use std::collections::HashMap;
use crate::error::Result;
use crate::models::{CommandBlock, ExecutionStatus};
use crate::terminal::ansi_parser::AnsiParser;

/// Command block renderer
pub struct CommandBlocks {
    /// ANSI parser for formatting
    ansi_parser: AnsiParser,
    /// Block rendering configuration
    config: BlockConfig,
    /// Cached rendered blocks
    rendered_blocks: HashMap<String, RenderedBlock>,
    /// Block interaction state
    interaction_state: InteractionState,
}

#[derive(Debug, Clone)]
pub struct BlockConfig {
    /// Show timestamps
    show_timestamps: bool,
    /// Show execution status
    show_status: bool,
    /// Maximum output lines to display (0 = unlimited)
    max_output_lines: usize,
    /// Block padding
    padding: egui::Vec2,
    /// Block spacing
    spacing: f32,
    /// Font size
    font_size: f32,
}

#[derive(Debug, Clone)]
pub struct RenderedBlock {
    /// Block ID
    id: String,
    /// Command area content
    command_area: RenderArea,
    /// Output area content
    output_area: RenderArea,
    /// Status indicator
    status_indicator: StatusIcon,
    /// Timestamp display
    timestamp_display: String,
    /// Total block dimensions (currently unused but kept for future layout calculations)
    #[allow(dead_code)]
    dimensions: egui::Vec2,
    /// Whether block is expanded
    expanded: bool,
}

#[derive(Debug, Clone)]
pub struct RenderArea {
    /// Text content
    text: String,
    /// Dimensions of the area
    dimensions: egui::Vec2,
}

#[derive(Debug, Clone)]
pub enum StatusIcon {
    /// Success (green checkmark)
    Success,
    /// Error (red X)
    Error,
    /// Running (spinner)
    Running,
    /// Unknown (question mark)
    Unknown,
}

#[derive(Debug, Clone)]
pub enum ContextMenuAction {
    /// Copy the command text to clipboard
    CopyCommand(String),
    /// Copy the output text to clipboard
    CopyOutput(String),
    /// Copy both command and output to clipboard
    CopyCommandAndOutput(String),
    /// Rerun the command
    RerunCommand(String),
}

#[derive(Debug, Clone)]
pub struct InteractionState {
    /// Currently hovered block ID
    pub hovered_block: Option<String>,
    /// Currently selected block ID
    pub selected_block: Option<String>,
    /// Block showing context menu
    pub context_menu_block: Option<String>,
    /// Context menu position
    pub context_menu_pos: Option<egui::Pos2>,
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self {
            show_timestamps: true,
            show_status: true,
            max_output_lines: 0, // 0 = unlimited
            padding: egui::Vec2::new(8.0, 6.0),
            spacing: 4.0,
            font_size: 12.0,
        }
    }
}

impl Default for InteractionState {
    fn default() -> Self {
        Self {
            hovered_block: None,
            selected_block: None,
            context_menu_block: None,
            context_menu_pos: None,
        }
    }
}

impl CommandBlocks {
    /// Create a new command blocks renderer
    pub fn new() -> Self {
        Self {
            ansi_parser: AnsiParser::new(),
            config: BlockConfig::default(),
            rendered_blocks: HashMap::new(),
            interaction_state: InteractionState::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: BlockConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Render command blocks in the UI
    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // In a real implementation, we would get blocks from the terminal
            // For now, show a placeholder
            self.render_placeholder(ui);
        });
    }

    /// Render command history with enhanced block-based UI
    pub fn render_command_history(&mut self, ui: &mut egui::Ui, command_blocks: &[CommandBlock]) {
        ui.vertical(|ui| {
            if command_blocks.is_empty() {
                self.render_placeholder(ui);
                return;
            }

            // Render each command block with enhanced styling
            for (index, block) in command_blocks.iter().enumerate() {
                self.render_enhanced_block(ui, block, index);

                // Add spacing between blocks
                if index < command_blocks.len() - 1 {
                    ui.add_space(self.config.spacing);
                }
            }
        });
    }

    /// Render a single command block with enhanced styling
    fn render_enhanced_block(&mut self, ui: &mut egui::Ui, block: &CommandBlock, index: usize) {
        // Create a frame for the block with subtle background
        let block_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgba_premultiplied(25, 25, 35, 180))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 65)))
            .inner_margin(self.config.padding)
            .outer_margin(egui::Margin::symmetric(0.0, 2.0));

        block_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Command header with timestamp and status
                self.render_command_header(ui, block, index);

                ui.add_space(4.0);

                // Command text
                self.render_command_text(ui, block);

                // Output area (if any)
                if !block.output.is_empty() {
                    ui.add_space(6.0);
                    self.render_output_area(ui, block);
                }
            });
        });
    }

    /// Render command header with metadata
    fn render_command_header(&self, ui: &mut egui::Ui, block: &CommandBlock, index: usize) {
        ui.horizontal(|ui| {
            // Command number
            ui.label(egui::RichText::new(format!("#{}", index + 1))
                .font(egui::FontId::monospace(10.0))
                .color(egui::Color32::from_rgb(150, 150, 170)));

            ui.separator();

            // Timestamp
            let timestamp = format!("{}", block.timestamp.format("%H:%M:%S"));

            ui.label(egui::RichText::new(&timestamp)
                .font(egui::FontId::monospace(10.0))
                .color(egui::Color32::from_rgb(120, 120, 140)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Status indicator
                self.render_status_indicator(ui, block);
            });
        });
    }

    /// Render status indicator
    fn render_status_indicator(&self, ui: &mut egui::Ui, block: &CommandBlock) {
        match block.status {
            ExecutionStatus::Running => {
                ui.colored_label(egui::Color32::from_rgb(255, 200, 0), "● Running");
            }
            ExecutionStatus::Completed => {
                if let Some(exit_code) = block.exit_code {
                    if exit_code == 0 {
                        ui.colored_label(egui::Color32::from_rgb(0, 255, 100), "● Success");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), format!("● Failed ({})", exit_code));
                    }
                } else {
                    ui.colored_label(egui::Color32::from_rgb(0, 255, 100), "● Completed");
                }
            }
            ExecutionStatus::Failed => {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "● Failed");
            }
            ExecutionStatus::Cancelled => {
                ui.colored_label(egui::Color32::from_rgb(255, 165, 0), "● Cancelled");
            }
            ExecutionStatus::Pending => {
                ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "● Pending");
            }
        }
    }

    /// Render command text with syntax highlighting
    fn render_command_text(&mut self, ui: &mut egui::Ui, block: &CommandBlock) {
        // Parse and render command with ANSI codes
        if let Ok(rendered) = self.render_command_block(block) {
            self.render_rendered_block(ui, &rendered);
        } else {
            // Fallback to plain text
            ui.label(egui::RichText::new(&block.command)
                .font(egui::FontId::monospace(12.0))
                .color(egui::Color32::from_rgb(200, 200, 255)));
        }
    }

    /// Render output area
    fn render_output_area(&mut self, ui: &mut egui::Ui, block: &CommandBlock) {
        // Create a subtle frame for output
        let output_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgba_premultiplied(15, 15, 25, 200))
            .stroke(egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 80)))
            .inner_margin(egui::Margin::symmetric(8.0, 6.0));

        output_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Display output lines (unlimited if max_output_lines is 0)
                let display_limit = if self.config.max_output_lines == 0 {
                    block.output.len()
                } else {
                    self.config.max_output_lines
                };

                let display_lines = block.output.iter()
                    .take(display_limit)
                    .enumerate();

                for (i, line) in display_lines {
                    // Render each output line
                    ui.label(egui::RichText::new(&line.text)
                        .font(egui::FontId::monospace(11.0))
                        .color(egui::Color32::from_rgb(180, 180, 200)));

                    // Add subtle line spacing
                    if i < block.output.len().min(display_limit) - 1 {
                        ui.add_space(1.0);
                    }
                }

                // Show truncation indicator if needed
                if self.config.max_output_lines > 0 && block.output.len() > self.config.max_output_lines {
                    let remaining = block.output.len() - self.config.max_output_lines;
                    ui.label(egui::RichText::new(format!("... and {} more lines", remaining))
                        .font(egui::FontId::monospace(9.0))
                        .color(egui::Color32::from_rgb(120, 120, 140)));
                }
            });
        });
    }

    /// Render a single command block
    pub fn render_block(&mut self, ui: &mut egui::Ui, block: &CommandBlock) -> Result<()> {
        let block_id = block.id.clone();

        // Check if we have a cached rendered block
        if !self.rendered_blocks.contains_key(&block_id) {
            let rendered = self.render_command_block(block)?;
            self.rendered_blocks.insert(block_id.clone(), rendered);
        }

        // Get the rendered block and render it
        if let Some(rendered) = self.rendered_blocks.get(&block_id).cloned() {
            self.render_rendered_block(ui, &rendered);
        }

        Ok(())
    }

    /// Render a command block from data
    fn render_command_block(&mut self, block: &CommandBlock) -> Result<RenderedBlock> {
        // Parse ANSI codes in command
        let command_parsed = self.ansi_parser.parse(&block.command)?;

        // Create command area
        let command_area = RenderArea {
            text: command_parsed.clean_text,
            dimensions: egui::Vec2::new(400.0, 20.0), // Placeholder dimensions
        };

        // Create output area (simplified)
        let output_limit = if self.config.max_output_lines == 0 {
            block.output.len()
        } else {
            self.config.max_output_lines
        };

        let output_text = block.output.iter()
            .take(output_limit)
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
            .join("\n");

        let output_parsed = self.ansi_parser.parse(&output_text)?;
        let output_area = RenderArea {
            text: output_parsed.clean_text,
            dimensions: egui::Vec2::new(400.0, 100.0), // Placeholder dimensions
        };

        // Determine status
        let status_indicator = match block.status {
            ExecutionStatus::Completed => StatusIcon::Success,
            ExecutionStatus::Running => StatusIcon::Running,
            ExecutionStatus::Pending => StatusIcon::Unknown,
            ExecutionStatus::Cancelled => StatusIcon::Error, // Orange-ish icon
            _ => StatusIcon::Error,
        };

        // Format timestamp
        let timestamp_display = if self.config.show_timestamps {
            block.timestamp.format("%H:%M:%S").to_string()
        } else {
            String::new()
        };

        // Calculate total dimensions
        let dimensions = egui::Vec2::new(
            command_area.dimensions.x.max(output_area.dimensions.x),
            command_area.dimensions.y + output_area.dimensions.y + self.config.spacing,
        );

        Ok(RenderedBlock {
            id: block.id.clone(),
            command_area,
            output_area,
            status_indicator,
            timestamp_display,
            dimensions,
            expanded: true, // Default to expanded
        })
    }

    /// Render a pre-computed rendered block
    fn render_rendered_block(&mut self, ui: &mut egui::Ui, rendered: &RenderedBlock) {
        let block_id = rendered.id.clone();

        // Block container with padding
        let block_rect = ui.available_rect_before_wrap();
        let block_response = ui.allocate_rect(block_rect, egui::Sense::click_and_drag());

        // Handle hover and selection
        let is_hovered = block_response.hovered();
        let is_selected = self.interaction_state.selected_block.as_ref() == Some(&block_id);

        // Update interaction state
        if is_hovered {
            self.interaction_state.hovered_block = Some(block_id.clone());
        }

        // Block background
        let bg_color = if is_selected {
            egui::Color32::from_rgb(60, 60, 80)
        } else if is_hovered {
            egui::Color32::from_rgb(45, 45, 55)
        } else {
            egui::Color32::from_rgb(35, 35, 45)
        };

        ui.painter().rect_filled(block_rect, 4.0, bg_color);

        // Block border
        let border_color = if is_selected {
            egui::Color32::from_rgb(100, 150, 255)
        } else {
            egui::Color32::from_rgb(80, 80, 100)
        };

        ui.painter().rect_stroke(block_rect, 4.0, egui::Stroke::new(1.0, border_color));

        // Block content
        ui.allocate_ui_at_rect(block_rect.shrink(self.config.padding.x), |ui| {
            ui.vertical(|ui| {
                // Header with command and status
                self.render_block_header(ui, rendered);

                ui.add_space(self.config.spacing);

                // Output area
                if rendered.expanded {
                    self.render_block_output(ui, rendered);
                }
            });
        });

        // Handle left click for selection
        if block_response.clicked_by(egui::PointerButton::Primary) {
            if is_selected {
                self.interaction_state.selected_block = None;
            } else {
                self.interaction_state.selected_block = Some(block_id.clone());
            }
            // Close context menu when left clicking elsewhere
            if self.interaction_state.context_menu_block.is_some() {
                self.interaction_state.context_menu_block = None;
                self.interaction_state.context_menu_pos = None;
            }
        }

        // Handle right click for context menu
        if block_response.clicked_by(egui::PointerButton::Secondary) {
            // Select the block first
            self.interaction_state.selected_block = Some(block_id.clone());

            // Show context menu at cursor position
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                self.interaction_state.context_menu_block = Some(block_id);
                self.interaction_state.context_menu_pos = Some(pos);
            }
        }
    }

    /// Render block header (command + status)
    fn render_block_header(&mut self, ui: &mut egui::Ui, rendered: &RenderedBlock) {
        ui.horizontal(|ui| {
            // Status icon
            if self.config.show_status {
                let (icon, color) = match rendered.status_indicator {
                    StatusIcon::Success => ("✓", egui::Color32::GREEN),
                    StatusIcon::Error => ("✗", egui::Color32::RED),
                    StatusIcon::Running => ("⟳", egui::Color32::YELLOW),
                    StatusIcon::Unknown => ("?", egui::Color32::GRAY),
                };

                ui.colored_label(color, icon);
                ui.add_space(self.config.spacing);
            }

            // Command text
            ui.label(egui::RichText::new(&rendered.command_area.text)
                .font(egui::FontId::monospace(self.config.font_size))
                .color(egui::Color32::WHITE));

            // Timestamp (right-aligned)
            if self.config.show_timestamps && !rendered.timestamp_display.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&rendered.timestamp_display)
                        .font(egui::FontId::proportional(self.config.font_size * 0.8))
                        .color(egui::Color32::LIGHT_GRAY));
                });
            }
        });
    }

    /// Render block output area
    fn render_block_output(&mut self, ui: &mut egui::Ui, rendered: &RenderedBlock) {
        // Output background (slightly different from main background)
        let output_bg = egui::Color32::from_rgb(25, 25, 35);
        let output_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(output_rect, 2.0, output_bg);

        ui.add_space(2.0);

        // Output text
        ui.label(egui::RichText::new(&rendered.output_area.text)
            .font(egui::FontId::monospace(self.config.font_size))
            .color(egui::Color32::LIGHT_GRAY));
    }

    /// Render placeholder when no blocks are available
    pub fn render_placeholder(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("🖥️")
                    .font(egui::FontId::proportional(48.0))
                    .color(egui::Color32::LIGHT_GRAY));
                ui.add_space(10.0);
                ui.heading("No commands yet");
                ui.label("Execute a command to see it here");
                ui.add_space(20.0);
            });
        });
    }

    /// Clear cached rendered blocks
    pub fn clear_cache(&mut self) {
        self.rendered_blocks.clear();
    }

    /// Get interaction state
    pub fn interaction_state(&self) -> &InteractionState {
        &self.interaction_state
    }

    /// Get mutable interaction state
    pub fn interaction_state_mut(&mut self) -> &mut InteractionState {
        &mut self.interaction_state
    }

    /// Set configuration
    pub fn set_config(&mut self, config: BlockConfig) {
        self.config = config;
        self.clear_cache(); // Clear cache when config changes
    }

    /// Check if a context menu should be shown for a block (doesn't render it)
    pub fn should_show_context_menu(&self, command_block: &CommandBlock) -> bool {
        if let Some(menu_block_id) = &self.interaction_state.context_menu_block {
            menu_block_id == &command_block.id
        } else {
            false
        }
    }

    /// Get the current context menu position
    pub fn context_menu_pos(&self) -> Option<egui::Pos2> {
        self.interaction_state.context_menu_pos
    }

    /// Render context menu at a specific position for a specific block
    pub fn render_context_menu_at(&mut self, ui: &mut egui::Ui, command_block: &CommandBlock, position: egui::Pos2) -> Option<ContextMenuAction> {
        let mut action = None;

        // Render context menu at the specified position
        let response = egui::Area::new(format!("block_context_menu_{}", command_block.id))
            .fixed_pos(position)
            .show(ui.ctx(), |ui| {
                self.render_context_menu_ui(ui, command_block, &mut action);
            });

        // Close menu if clicked outside or if the response was clicked
        if ui.input(|i| i.pointer.any_click()) {
            if !response.response.rect.contains(position) || response.response.clicked() {
                self.interaction_state.context_menu_block = None;
                self.interaction_state.context_menu_pos = None;
            }
        }

        action
    }

    /// Close any open context menu
    pub fn close_context_menu(&mut self) {
        self.interaction_state.context_menu_block = None;
        self.interaction_state.context_menu_pos = None;
    }

    /// Render the context menu UI
    fn render_context_menu_ui(&self, ui: &mut egui::Ui, command_block: &CommandBlock, action: &mut Option<ContextMenuAction>) {
        egui::Frame::popup(ui.style())
            .shadow(egui::epaint::Shadow::small_dark())
            .show(ui, |ui| {
                ui.set_min_width(180.0);

                // Copy Command
                if ui.selectable_label(false, "📋 Copy Command").clicked() {
                    *action = Some(ContextMenuAction::CopyCommand(command_block.command.clone()));
                }

                // Copy Output (if any)
                if !command_block.output.is_empty() {
                    if ui.selectable_label(false, "📄 Copy Output").clicked() {
                        let output_text = command_block.output.iter()
                            .map(|line| line.text.clone())
                            .collect::<Vec<_>>()
                            .join("\n");
                        *action = Some(ContextMenuAction::CopyOutput(output_text));
                    }
                }

                ui.separator();

                // Rerun Command
                if ui.selectable_label(false, "🔄 Rerun Command").clicked() {
                    *action = Some(ContextMenuAction::RerunCommand(command_block.command.clone()));
                }

                // Copy Command + Output (if output exists)
                if !command_block.output.is_empty() {
                    ui.separator();
                    if ui.selectable_label(false, "📋 Copy All").clicked() {
                        let output_text = command_block.output.iter()
                            .map(|line| line.text.clone())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let all_text = format!("{}\n{}", command_block.command, output_text);
                        *action = Some(ContextMenuAction::CopyCommandAndOutput(all_text));
                    }
                }
            });
    }
}

/// Block rendering utilities
pub mod utils {
    use super::*;

    /// Create a mock command block for testing
    pub fn create_mock_block(command: &str, _output: &str, _success: bool) -> CommandBlock {
        CommandBlock::new(
            command.to_string(),
            std::path::PathBuf::from("/tmp"),
        )
    }

    /// Calculate block height based on content
    pub fn calculate_block_height(block: &CommandBlock, config: &BlockConfig) -> f32 {
        let command_lines = (block.command.len() as f32 / 80.0).ceil().max(1.0);
        let output_lines = block.output.len() as f32 / 80.0;
        let total_lines = command_lines + output_lines;

        total_lines * config.font_size * 1.2 + config.padding.y * 2.0 + config.spacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_blocks_creation() {
        let blocks = CommandBlocks::new();
        assert!(blocks.rendered_blocks.is_empty());
    }

    #[test]
    fn test_block_config_defaults() {
        let config = BlockConfig::default();
        assert!(config.show_timestamps);
        assert!(config.show_status);
        assert_eq!(config.max_output_lines, 0); // 0 = unlimited
        assert_eq!(config.font_size, 12.0);
    }

    #[test]
    fn test_status_icons() {
        assert_eq!(format!("{:?}", StatusIcon::Success), "Success");
        assert_eq!(format!("{:?}", StatusIcon::Error), "Error");
        assert_eq!(format!("{:?}", StatusIcon::Running), "Running");
        assert_eq!(format!("{:?}", StatusIcon::Unknown), "Unknown");
    }

    #[test]
    fn test_interaction_state() {
        let state = InteractionState::default();
        assert!(state.hovered_block.is_none());
        assert!(state.selected_block.is_none());
        // InteractionState doesn't have scroll_position field
        assert!(state.hovered_block.is_none());
    }

    #[test]
    fn test_render_area() {
        let area = RenderArea {
            text: "test".to_string(),
            dimensions: egui::Vec2::new(100.0, 20.0),
        };

        assert_eq!(area.text, "test");
    }

    #[test]
    fn test_rendered_block() {
        let block = RenderedBlock {
            id: "test".to_string(),
            command_area: RenderArea {
                text: "echo hello".to_string(),
                dimensions: egui::Vec2::new(100.0, 20.0),
            },
            output_area: RenderArea {
                text: "hello".to_string(),
                dimensions: egui::Vec2::new(100.0, 20.0),
            },
            status_indicator: StatusIcon::Success,
            timestamp_display: "12:34:56".to_string(),
            dimensions: egui::Vec2::new(100.0, 40.0),
            expanded: true,
        };

        assert_eq!(block.id, "test");
        assert_eq!(block.command_area.text, "echo hello");
        assert_eq!(block.output_area.text, "hello");
        assert!(block.expanded);
    }

    #[test]
    fn test_clear_cache() {
        let mut blocks = CommandBlocks::new();
        blocks.rendered_blocks.insert("test".to_string(), RenderedBlock {
            id: "test".to_string(),
            command_area: RenderArea {
                text: "test".to_string(),
                dimensions: egui::Vec2::new(50.0, 20.0),
            },
            output_area: RenderArea {
                text: "output".to_string(),
                dimensions: egui::Vec2::new(50.0, 20.0),
            },
            status_indicator: StatusIcon::Success,
            timestamp_display: String::new(),
            dimensions: egui::Vec2::new(50.0, 40.0),
            expanded: true,
        });

        assert_eq!(blocks.rendered_blocks.len(), 1);
        blocks.clear_cache();
        assert_eq!(blocks.rendered_blocks.len(), 0);
    }

    #[test]
    fn test_context_menu_state() {
        let mut blocks = CommandBlocks::new();

        // Create a test block
        let test_block = CommandBlock::new("echo hello".to_string(), std::path::PathBuf::from("/tmp"));
        let test_pos = egui::Pos2::new(100.0, 200.0);

        // Initially no context menu
        assert!(!blocks.should_show_context_menu(&test_block));
        assert!(blocks.context_menu_pos().is_none());

        // Manually set context menu state (simulating right-click)
        blocks.interaction_state.context_menu_block = Some(test_block.id.clone());
        blocks.interaction_state.context_menu_pos = Some(test_pos);

        // Check that context menu should show
        assert!(blocks.should_show_context_menu(&test_block));
        assert_eq!(blocks.context_menu_pos(), Some(test_pos));

        // Close context menu
        blocks.close_context_menu();
        assert!(!blocks.should_show_context_menu(&test_block));
        assert!(blocks.context_menu_pos().is_none());
    }
}
