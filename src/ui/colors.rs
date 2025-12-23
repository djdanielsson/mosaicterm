//! Color utilities for UI rendering
//!
//! This module provides utilities for converting config colors to egui colors
//! and managing the application color theme.

use crate::models::config::{AnsiColors, BlockColors, Color, InputColors, StatusBarColors, Theme};
use eframe::egui;

/// Extension trait to convert config Color to egui::Color32
pub trait ToEguiColor {
    /// Convert to egui::Color32
    fn to_egui(&self) -> egui::Color32;

    /// Convert to egui::Color32 with custom alpha
    fn to_egui_with_alpha(&self, alpha: u8) -> egui::Color32;
}

impl ToEguiColor for Color {
    fn to_egui(&self) -> egui::Color32 {
        let (r, g, b, a) = self.to_rgba8();
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    fn to_egui_with_alpha(&self, alpha: u8) -> egui::Color32 {
        let (r, g, b, _) = self.to_rgba8();
        egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
    }
}

/// UI color provider that caches egui colors from the theme
#[derive(Debug, Clone)]
pub struct UiColors {
    /// Main theme colors
    pub background: egui::Color32,
    pub foreground: egui::Color32,
    pub accent: egui::Color32,
    pub success: egui::Color32,
    pub error: egui::Color32,
    pub warning: egui::Color32,
    pub selection: egui::Color32,

    /// ANSI terminal colors
    pub ansi: AnsiEguiColors,

    /// Command block colors
    pub blocks: BlockEguiColors,

    /// Input field colors
    pub input: InputEguiColors,

    /// Status bar colors
    pub status_bar: StatusBarEguiColors,
}

/// ANSI colors converted to egui
#[derive(Debug, Clone)]
pub struct AnsiEguiColors {
    pub black: egui::Color32,
    pub red: egui::Color32,
    pub green: egui::Color32,
    pub yellow: egui::Color32,
    pub blue: egui::Color32,
    pub magenta: egui::Color32,
    pub cyan: egui::Color32,
    pub white: egui::Color32,
    pub bright_black: egui::Color32,
    pub bright_red: egui::Color32,
    pub bright_green: egui::Color32,
    pub bright_yellow: egui::Color32,
    pub bright_blue: egui::Color32,
    pub bright_magenta: egui::Color32,
    pub bright_cyan: egui::Color32,
    pub bright_white: egui::Color32,
}

/// Block colors converted to egui
#[derive(Debug, Clone)]
pub struct BlockEguiColors {
    pub background: egui::Color32,
    pub border: egui::Color32,
    pub header_background: egui::Color32,
    pub command_text: egui::Color32,
    pub output_text: egui::Color32,
    pub timestamp: egui::Color32,
    pub prompt: egui::Color32,
    pub status_running: egui::Color32,
    pub status_completed: egui::Color32,
    pub status_failed: egui::Color32,
    pub status_cancelled: egui::Color32,
    pub status_pending: egui::Color32,
    pub status_tui: egui::Color32,
    pub hover_border: egui::Color32,
    pub selected_border: egui::Color32,
}

/// Input colors converted to egui
#[derive(Debug, Clone)]
pub struct InputEguiColors {
    pub background: egui::Color32,
    pub text: egui::Color32,
    pub placeholder: egui::Color32,
    pub cursor: egui::Color32,
    pub border: egui::Color32,
    pub focused_border: egui::Color32,
    pub prompt: egui::Color32,
}

/// Status bar colors converted to egui
#[derive(Debug, Clone)]
pub struct StatusBarEguiColors {
    pub background: egui::Color32,
    pub text: egui::Color32,
    pub path: egui::Color32,
    pub branch: egui::Color32,
    pub environment: egui::Color32,
    pub ssh_indicator: egui::Color32,
    pub border: egui::Color32,
}

impl UiColors {
    /// Create UiColors from a Theme configuration
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            background: theme.background.to_egui(),
            foreground: theme.foreground.to_egui(),
            accent: theme.accent.to_egui(),
            success: theme.success.to_egui(),
            error: theme.error.to_egui(),
            warning: theme.warning.to_egui(),
            selection: theme.selection.to_egui(),
            ansi: AnsiEguiColors::from_config(&theme.ansi),
            blocks: BlockEguiColors::from_config(&theme.blocks),
            input: InputEguiColors::from_config(&theme.input),
            status_bar: StatusBarEguiColors::from_config(&theme.status_bar),
        }
    }
}

impl Default for UiColors {
    fn default() -> Self {
        Self::from_theme(&Theme::default())
    }
}

impl AnsiEguiColors {
    pub fn from_config(config: &AnsiColors) -> Self {
        Self {
            black: config.black.to_egui(),
            red: config.red.to_egui(),
            green: config.green.to_egui(),
            yellow: config.yellow.to_egui(),
            blue: config.blue.to_egui(),
            magenta: config.magenta.to_egui(),
            cyan: config.cyan.to_egui(),
            white: config.white.to_egui(),
            bright_black: config.bright_black.to_egui(),
            bright_red: config.bright_red.to_egui(),
            bright_green: config.bright_green.to_egui(),
            bright_yellow: config.bright_yellow.to_egui(),
            bright_blue: config.bright_blue.to_egui(),
            bright_magenta: config.bright_magenta.to_egui(),
            bright_cyan: config.bright_cyan.to_egui(),
            bright_white: config.bright_white.to_egui(),
        }
    }
}

impl BlockEguiColors {
    pub fn from_config(config: &BlockColors) -> Self {
        Self {
            background: config.background.to_egui(),
            border: config.border.to_egui(),
            header_background: config.header_background.to_egui(),
            command_text: config.command_text.to_egui(),
            output_text: config.output_text.to_egui(),
            timestamp: config.timestamp.to_egui(),
            prompt: config.prompt.to_egui(),
            status_running: config.status_running.to_egui(),
            status_completed: config.status_completed.to_egui(),
            status_failed: config.status_failed.to_egui(),
            status_cancelled: config.status_cancelled.to_egui(),
            status_pending: config.status_pending.to_egui(),
            status_tui: config.status_tui.to_egui(),
            hover_border: config.hover_border.to_egui(),
            selected_border: config.selected_border.to_egui(),
        }
    }
}

impl InputEguiColors {
    pub fn from_config(config: &InputColors) -> Self {
        Self {
            background: config.background.to_egui(),
            text: config.text.to_egui(),
            placeholder: config.placeholder.to_egui(),
            cursor: config.cursor.to_egui(),
            border: config.border.to_egui(),
            focused_border: config.focused_border.to_egui(),
            prompt: config.prompt.to_egui(),
        }
    }
}

impl StatusBarEguiColors {
    pub fn from_config(config: &StatusBarColors) -> Self {
        Self {
            background: config.background.to_egui(),
            text: config.text.to_egui(),
            path: config.path.to_egui(),
            branch: config.branch.to_egui(),
            environment: config.environment.to_egui(),
            ssh_indicator: config.ssh_indicator.to_egui(),
            border: config.border.to_egui(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_egui() {
        let color = Color::from_rgb8(255, 128, 64);
        let egui_color = color.to_egui();
        assert_eq!(egui_color.r(), 255);
        assert_eq!(egui_color.g(), 128);
        assert_eq!(egui_color.b(), 64);
        assert_eq!(egui_color.a(), 255);
    }

    #[test]
    fn test_color_to_egui_with_alpha() {
        let color = Color::from_rgb8(255, 128, 64);
        let egui_color = color.to_egui_with_alpha(128);
        // egui stores colors with premultiplied alpha internally,
        // so we check that alpha is correctly set
        assert_eq!(egui_color.a(), 128);
        // The RGB values may be adjusted due to premultiplication
        // Just verify the color was created successfully
        assert!(egui_color.r() > 0);
        assert!(egui_color.g() > 0);
        assert!(egui_color.b() > 0);
    }

    #[test]
    fn test_ui_colors_from_theme() {
        let theme = Theme::default();
        let colors = UiColors::from_theme(&theme);

        // Check that colors are properly converted
        assert_eq!(
            colors.blocks.status_running,
            egui::Color32::from_rgb(255, 200, 0)
        );
        assert_eq!(
            colors.blocks.status_completed,
            egui::Color32::from_rgb(0, 255, 100)
        );
    }

    #[test]
    fn test_ansi_colors_from_config() {
        let config = AnsiColors::default();
        let colors = AnsiEguiColors::from_config(&config);

        assert_eq!(colors.black, egui::Color32::from_rgb(0, 0, 0));
        assert_eq!(colors.red, egui::Color32::from_rgb(205, 49, 49));
    }

    #[test]
    fn test_default_ui_colors() {
        let colors = UiColors::default();
        // Just verify it doesn't panic
        assert_ne!(colors.background, egui::Color32::TRANSPARENT);
    }
}
