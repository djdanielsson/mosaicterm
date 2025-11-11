//! UI components and rendering
//!
//! This module contains all UI-related functionality for MosaicTerm,
//! including command block rendering, input handling, and layout management.

pub mod blocks;
pub mod completion_popup;
pub mod input;
pub mod metrics;
pub mod scroll;
pub mod text;
pub mod tui_overlay;
pub mod viewport;

// Re-exports for convenience
pub use blocks::{BlockConfig, CommandBlocks, RenderedBlock, StatusIcon};
pub use completion_popup::CompletionPopup;
pub use input::{InputConfig, InputPrompt};
pub use metrics::MetricsPanel;
pub use scroll::{ScrollState, ScrollableHistory, ScrollbarConfig};
pub use text::{AnsiTextRenderer, ColorScheme, FontConfig};
pub use tui_overlay::TuiOverlay;
pub use viewport::{TerminalViewport, ViewportConfig};

use eframe::egui;

/// Layout manager for responsive UI adaptation
pub struct LayoutManager {
    /// Current window size
    window_size: egui::Vec2,
    /// Minimum window size
    min_window_size: egui::Vec2,
    /// Maximum window size
    max_window_size: Option<egui::Vec2>,
    /// Layout breakpoints for responsive design
    breakpoints: LayoutBreakpoints,
    /// Current layout mode
    current_mode: LayoutMode,
}

#[derive(Debug, Clone)]
pub struct LayoutBreakpoints {
    /// Mobile/small window breakpoint
    pub mobile: f32,
    /// Tablet/medium window breakpoint
    pub tablet: f32,
    /// Desktop/large window breakpoint
    pub desktop: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    /// Mobile layout (single column, stacked)
    Mobile,
    /// Tablet layout (compact but readable)
    Tablet,
    /// Desktop layout (full featured)
    Desktop,
}

impl Default for LayoutBreakpoints {
    fn default() -> Self {
        Self {
            mobile: 600.0,   // Below 600px width
            tablet: 900.0,   // Below 900px width
            desktop: 1200.0, // Above 1200px width
        }
    }
}

impl LayoutManager {
    /// Create a new layout manager
    pub fn new() -> Self {
        Self {
            window_size: egui::vec2(800.0, 600.0),
            min_window_size: egui::vec2(400.0, 300.0),
            max_window_size: None,
            breakpoints: LayoutBreakpoints::default(),
            current_mode: LayoutMode::Desktop,
        }
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutManager {
    /// Create layout manager with custom initial window size
    pub fn with_initial_size(width: f32, height: f32) -> Self {
        let mut manager = Self::new();
        manager.window_size = egui::vec2(width, height);
        manager.update_layout_mode();
        manager
    }

    /// Update window size and recalculate layout mode
    pub fn update_window_size(&mut self, new_size: egui::Vec2) {
        self.window_size = new_size.max(self.min_window_size);
        if let Some(max_size) = self.max_window_size {
            self.window_size = self.window_size.min(max_size);
        }
        self.update_layout_mode();
    }

    /// Update the current layout mode based on window size
    fn update_layout_mode(&mut self) {
        let width = self.window_size.x;

        self.current_mode = if width < self.breakpoints.mobile {
            LayoutMode::Mobile
        } else if width < self.breakpoints.tablet {
            LayoutMode::Tablet
        } else {
            LayoutMode::Desktop
        };
    }

    /// Get current layout mode
    pub fn current_mode(&self) -> LayoutMode {
        self.current_mode
    }

    /// Get responsive spacing based on layout mode
    pub fn responsive_spacing(&self) -> egui::Vec2 {
        match self.current_mode {
            LayoutMode::Mobile => egui::vec2(4.0, 4.0),
            LayoutMode::Tablet => egui::vec2(6.0, 6.0),
            LayoutMode::Desktop => egui::vec2(8.0, 8.0),
        }
    }

    /// Get responsive font size for body text
    pub fn responsive_font_size(&self) -> f32 {
        match self.current_mode {
            LayoutMode::Mobile => 11.0,
            LayoutMode::Tablet => 12.0,
            LayoutMode::Desktop => 13.0,
        }
    }

    /// Get responsive font size for monospace text
    pub fn responsive_mono_font_size(&self) -> f32 {
        match self.current_mode {
            LayoutMode::Mobile => 10.0,
            LayoutMode::Tablet => 11.0,
            LayoutMode::Desktop => 12.0,
        }
    }

    /// Calculate responsive column count
    pub fn responsive_columns(&self) -> usize {
        match self.current_mode {
            LayoutMode::Mobile => 1,
            LayoutMode::Tablet => 1,
            LayoutMode::Desktop => 2,
        }
    }

    /// Check if layout is mobile
    pub fn is_mobile(&self) -> bool {
        matches!(self.current_mode, LayoutMode::Mobile)
    }

    /// Check if layout is tablet
    pub fn is_tablet(&self) -> bool {
        matches!(self.current_mode, LayoutMode::Tablet)
    }

    /// Check if layout is desktop
    pub fn is_desktop(&self) -> bool {
        matches!(self.current_mode, LayoutMode::Desktop)
    }

    /// Get available content area size
    pub fn content_area_size(&self) -> egui::Vec2 {
        self.window_size
    }

    /// Get recommended layout proportions for different areas
    pub fn layout_proportions(&self) -> LayoutProportions {
        match self.current_mode {
            LayoutMode::Mobile => LayoutProportions {
                status_bar_height: 28.0,
                input_area_height: 80.0,
                sidebar_width: 0.0, // No sidebar on mobile
                content_padding: 8.0,
            },
            LayoutMode::Tablet => LayoutProportions {
                status_bar_height: 32.0,
                input_area_height: 90.0,
                sidebar_width: 200.0,
                content_padding: 12.0,
            },
            LayoutMode::Desktop => LayoutProportions {
                status_bar_height: 36.0,
                input_area_height: 100.0,
                sidebar_width: 250.0,
                content_padding: 16.0,
            },
        }
    }

    /// Calculate adaptive font sizes based on window size
    pub fn adaptive_font_sizes(&self) -> AdaptiveFontSizes {
        let scale_factor = (self.window_size.x / 1920.0)
            .min(self.window_size.y / 1080.0)
            .max(0.5);

        AdaptiveFontSizes {
            terminal: (12.0 * scale_factor).clamp(9.0, 18.0),
            ui_body: (14.0 * scale_factor).clamp(11.0, 20.0),
            ui_heading: (18.0 * scale_factor).clamp(14.0, 24.0),
            input: (14.0 * scale_factor).clamp(12.0, 18.0),
        }
    }

    /// Get optimal grid layout for content
    pub fn optimal_grid_layout(&self) -> GridLayout {
        match self.current_mode {
            LayoutMode::Mobile => GridLayout {
                columns: 1,
                item_width: self.window_size.x - 32.0,
                spacing: egui::vec2(8.0, 8.0),
            },
            LayoutMode::Tablet => GridLayout {
                columns: 2,
                item_width: (self.window_size.x - 48.0) / 2.0,
                spacing: egui::vec2(12.0, 12.0),
            },
            LayoutMode::Desktop => GridLayout {
                columns: 3,
                item_width: (self.window_size.x - 64.0) / 3.0,
                spacing: egui::vec2(16.0, 16.0),
            },
        }
    }

    /// Check if window size has changed significantly
    pub fn has_size_changed(&self, new_size: egui::Vec2, threshold: f32) -> bool {
        let diff = (self.window_size - new_size).abs();
        diff.x > threshold || diff.y > threshold
    }

    /// Get smooth transition duration for layout changes
    pub fn transition_duration(&self, old_mode: LayoutMode, new_mode: LayoutMode) -> f32 {
        if old_mode != new_mode {
            0.3 // 300ms for mode changes
        } else {
            0.1 // 100ms for size adjustments
        }
    }

    /// Apply layout constraints to UI elements
    pub fn constrain_ui_element(
        &self,
        available_space: egui::Vec2,
        element_type: UiElementType,
    ) -> egui::Vec2 {
        let proportions = self.layout_proportions();

        match element_type {
            UiElementType::StatusBar => {
                egui::vec2(available_space.x, proportions.status_bar_height)
            }
            UiElementType::InputArea => {
                egui::vec2(available_space.x, proportions.input_area_height)
            }
            UiElementType::Sidebar => egui::vec2(proportions.sidebar_width, available_space.y),
            UiElementType::Content => {
                let content_width = available_space.x - proportions.sidebar_width;
                let content_height = available_space.y
                    - proportions.status_bar_height
                    - proportions.input_area_height;
                egui::vec2(content_width.max(200.0), content_height.max(100.0))
            }
        }
    }

    /// Get recommended panel sizes for different layout modes
    pub fn recommended_panel_sizes(&self) -> PanelSizes {
        match self.current_mode {
            LayoutMode::Mobile => PanelSizes {
                status_bar_height: 24.0,
                input_area_height: 60.0,
                scrollbar_width: 8.0,
                padding: 4.0,
            },
            LayoutMode::Tablet => PanelSizes {
                status_bar_height: 28.0,
                input_area_height: 70.0,
                scrollbar_width: 10.0,
                padding: 6.0,
            },
            LayoutMode::Desktop => PanelSizes {
                status_bar_height: 32.0,
                input_area_height: 80.0,
                scrollbar_width: 12.0,
                padding: 8.0,
            },
        }
    }

    /// Adapt UI style based on current layout mode
    pub fn adapt_style(&self, style: &mut egui::Style) {
        style.spacing.item_spacing = self.responsive_spacing();
        style.spacing.button_padding = egui::vec2(
            self.responsive_spacing().x * 0.75,
            self.responsive_spacing().y * 0.5,
        );
        style.spacing.menu_margin = egui::Margin::same(self.responsive_spacing().x * 0.5);

        // Update font sizes
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(self.responsive_font_size()),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(self.responsive_mono_font_size()),
        );
    }
}

/// Recommended panel sizes for different layout modes
#[derive(Debug, Clone)]
pub struct PanelSizes {
    pub status_bar_height: f32,
    pub input_area_height: f32,
    pub scrollbar_width: f32,
    pub padding: f32,
}

/// Layout proportions for different screen areas
#[derive(Debug, Clone)]
pub struct LayoutProportions {
    pub status_bar_height: f32,
    pub input_area_height: f32,
    pub sidebar_width: f32,
    pub content_padding: f32,
}

/// Adaptive font sizes based on window size
#[derive(Debug, Clone)]
pub struct AdaptiveFontSizes {
    pub terminal: f32,
    pub ui_body: f32,
    pub ui_heading: f32,
    pub input: f32,
}

/// Grid layout configuration
#[derive(Debug, Clone)]
pub struct GridLayout {
    pub columns: usize,
    pub item_width: f32,
    pub spacing: egui::Vec2,
}

/// UI element types for layout constraints
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiElementType {
    StatusBar,
    InputArea,
    Sidebar,
    Content,
}

/// Type alias for grid items to reduce complexity
pub type GridItems = Vec<Box<dyn FnOnce(&mut egui::Ui)>>;

/// Responsive grid layout helper
pub struct ResponsiveGrid {
    columns: usize,
    spacing: egui::Vec2,
}

impl ResponsiveGrid {
    pub fn new(layout_manager: &LayoutManager) -> Self {
        Self {
            columns: layout_manager.responsive_columns(),
            spacing: layout_manager.responsive_spacing(),
        }
    }

    pub fn show<F>(&self, ui: &mut egui::Ui, items: GridItems, item_width: Option<f32>)
    where
        F: FnOnce(&mut egui::Ui),
    {
        ui.horizontal_wrapped(|ui| {
            for (index, item) in items.into_iter().enumerate() {
                let width = item_width.unwrap_or_else(|| {
                    let available_width = ui.available_width();
                    (available_width - self.spacing.x * (self.columns - 1) as f32)
                        / self.columns as f32
                });

                ui.allocate_ui(egui::vec2(width, ui.available_height()), |ui| {
                    item(ui);
                });

                if (index + 1) % self.columns != 0 {
                    ui.add_space(self.spacing.x);
                }
            }
        });
    }
}
