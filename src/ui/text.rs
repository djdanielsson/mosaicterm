//! ANSI-aware text rendering
//!
//! This module handles rendering of text with ANSI escape sequence formatting
//! for the MosaicTerm interface.

use eframe::egui;
use std::collections::HashMap;
use crate::error::Result;
use crate::models::output_line::AnsiCode;
use crate::terminal::AnsiColor;

/// ANSI-aware text renderer
pub struct AnsiTextRenderer {
    /// Font configuration
    font_config: FontConfig,
    /// Color scheme mapping ANSI colors to egui colors
    color_scheme: ColorScheme,
    /// Cached rendered text for performance
    render_cache: HashMap<String, RenderedText>,
    /// Maximum cache size
    max_cache_size: usize,
}

#[derive(Debug, Clone)]
pub struct FontConfig {
    /// Default font family
    pub family: egui::FontFamily,
    /// Default font size
    pub size: f32,
    /// Monospace font for code
    pub monospace_family: egui::FontFamily,
    /// Monospace font size
    pub monospace_size: f32,
    /// Line height multiplier
    pub line_height: f32,
}

#[derive(Debug, Clone)]
pub struct ColorScheme {
    /// Default text color
    pub default_text: egui::Color32,
    /// Default background color
    pub default_background: egui::Color32,
    /// ANSI color mappings
    pub ansi_colors: HashMap<AnsiColor, egui::Color32>,
    /// Custom color mappings
    pub custom_colors: HashMap<String, egui::Color32>,
}

#[derive(Debug, Clone)]
pub struct RenderedText {
    /// The full rendered text layout
    pub layout: egui::epaint::text::LayoutJob,
    /// Total dimensions
    pub dimensions: egui::Vec2,
    /// Whether the text contains formatting
    pub has_formatting: bool,
    /// Number of ANSI codes processed
    pub ansi_code_count: usize,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: egui::FontFamily::Proportional,
            size: 14.0,
            monospace_family: egui::FontFamily::Monospace,
            monospace_size: 12.0,
            line_height: 1.2,
        }
    }
}

impl Default for ColorScheme {
    fn default() -> Self {
        let mut ansi_colors = HashMap::new();

        // Standard ANSI colors
        ansi_colors.insert(AnsiColor::Black, egui::Color32::from_rgb(0, 0, 0));
        ansi_colors.insert(AnsiColor::Red, egui::Color32::from_rgb(205, 49, 49));
        ansi_colors.insert(AnsiColor::Green, egui::Color32::from_rgb(13, 188, 121));
        ansi_colors.insert(AnsiColor::Yellow, egui::Color32::from_rgb(229, 229, 16));
        ansi_colors.insert(AnsiColor::Blue, egui::Color32::from_rgb(36, 114, 200));
        ansi_colors.insert(AnsiColor::Magenta, egui::Color32::from_rgb(188, 63, 188));
        ansi_colors.insert(AnsiColor::Cyan, egui::Color32::from_rgb(17, 168, 205));
        ansi_colors.insert(AnsiColor::White, egui::Color32::from_rgb(229, 229, 229));

        // Bright ANSI colors
        ansi_colors.insert(AnsiColor::BrightBlack, egui::Color32::from_rgb(102, 102, 102));
        ansi_colors.insert(AnsiColor::BrightRed, egui::Color32::from_rgb(241, 76, 76));
        ansi_colors.insert(AnsiColor::BrightGreen, egui::Color32::from_rgb(35, 209, 139));
        ansi_colors.insert(AnsiColor::BrightYellow, egui::Color32::from_rgb(245, 245, 67));
        ansi_colors.insert(AnsiColor::BrightBlue, egui::Color32::from_rgb(59, 142, 234));
        ansi_colors.insert(AnsiColor::BrightMagenta, egui::Color32::from_rgb(214, 112, 214));
        ansi_colors.insert(AnsiColor::BrightCyan, egui::Color32::from_rgb(41, 184, 219));
        ansi_colors.insert(AnsiColor::BrightWhite, egui::Color32::from_rgb(229, 229, 229));

        Self {
            default_text: egui::Color32::from_rgb(229, 229, 229),
            default_background: egui::Color32::TRANSPARENT,
            ansi_colors,
            custom_colors: HashMap::new(),
        }
    }
}

impl AnsiTextRenderer {
    /// Create a new ANSI text renderer
    pub fn new() -> Self {
        Self {
            font_config: FontConfig::default(),
            color_scheme: ColorScheme::default(),
            render_cache: HashMap::new(),
            max_cache_size: 100,
        }
    }
}

impl Default for AnsiTextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl AnsiTextRenderer {
    /// Create with custom configuration
    pub fn with_config(font_config: FontConfig, color_scheme: ColorScheme) -> Self {
        Self {
            font_config,
            color_scheme,
            render_cache: HashMap::new(),
            max_cache_size: 100,
        }
    }

    /// Render text with ANSI codes
    pub fn render_ansi_text(&mut self, ui: &mut egui::Ui, text: &str, ansi_codes: &[AnsiCode]) -> Result<()> {
        let cache_key = self.generate_cache_key(text, ansi_codes);

        // Check cache first
        if let Some(rendered) = self.render_cache.get(&cache_key) {
            self.render_cached_text(ui, rendered);
            return Ok(());
        }

        // Render new text
        let rendered = self.render_text_with_ansi(text, ansi_codes)?;
        self.render_cached_text(ui, &rendered);

        // Cache the result
        if self.render_cache.len() < self.max_cache_size {
            self.render_cache.insert(cache_key, rendered);
        }

        Ok(())
    }

    /// Render plain text without ANSI codes
    pub fn render_plain_text(&mut self, ui: &mut egui::Ui, text: &str) {
        ui.label(egui::RichText::new(text)
            .font(egui::FontId::new(self.font_config.size, self.font_config.family.clone()))
            .color(self.color_scheme.default_text));
    }

    /// Render text with ANSI formatting
    fn render_text_with_ansi(&self, text: &str, ansi_codes: &[AnsiCode]) -> Result<RenderedText> {
        let mut layout = egui::epaint::text::LayoutJob::default();
        let mut current_color = self.color_scheme.default_text;
        let mut current_bg_color = self.color_scheme.default_background;
        let mut current_font = egui::FontId::new(self.font_config.size, self.font_config.family.clone());
        let mut ansi_code_count = 0;

        // Sort ANSI codes by position
        let mut sorted_codes = ansi_codes.to_vec();
        sorted_codes.sort_by_key(|code| code.position);

        let mut last_pos = 0;

        for code in &sorted_codes {
            // Add text before this ANSI code
            if code.position > last_pos {
                let text_before = &text[last_pos..code.position];
                self.add_text_section(&mut layout, text_before, &current_font, current_color, current_bg_color);
            }

            // Apply ANSI formatting
            self.apply_ansi_formatting(code, &mut current_color, &mut current_bg_color, &mut current_font);
            ansi_code_count += 1;
            last_pos = code.position;
        }

        // Add remaining text
        if last_pos < text.len() {
            let remaining_text = &text[last_pos..];
            self.add_text_section(&mut layout, remaining_text, &current_font, current_color, current_bg_color);
        }

        // Calculate approximate dimensions (simplified)
        let dimensions = egui::Vec2::new(
            text.len() as f32 * self.font_config.size * 0.6,
            self.font_config.size * self.font_config.line_height
        );

        Ok(RenderedText {
            layout,
            dimensions,
            has_formatting: !ansi_codes.is_empty(),
            ansi_code_count,
        })
    }

    /// Add a text section with specific formatting
    fn add_text_section(
        &self,
        layout: &mut egui::epaint::text::LayoutJob,
        text: &str,
        font: &egui::FontId,
        color: egui::Color32,
        bg_color: egui::Color32,
    ) {
        use egui::epaint::text::TextFormat;

        layout.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color,
                background: bg_color,
                italics: false,
                underline: egui::Stroke::NONE,
                strikethrough: egui::Stroke::NONE,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
    }

    /// Apply ANSI formatting to current style
    fn apply_ansi_formatting(
        &self,
        ansi_code: &AnsiCode,
        current_color: &mut egui::Color32,
        current_bg_color: &mut egui::Color32,
        current_font: &mut egui::FontId,
    ) {
        // Parse ANSI code to determine formatting
        // This is a simplified implementation - would need proper ANSI parsing
        if ansi_code.code.contains("[0") {
            // Reset
            *current_color = self.color_scheme.default_text;
            *current_bg_color = self.color_scheme.default_background;
            *current_font = egui::FontId::new(self.font_config.size, self.font_config.family.clone());
        } else if ansi_code.code.contains("[1") {
            // Bold - make brighter
            let r = (current_color.r() as f32 * 1.2).min(255.0) as u8;
            let g = (current_color.g() as f32 * 1.2).min(255.0) as u8;
            let b = (current_color.b() as f32 * 1.2).min(255.0) as u8;
            *current_color = egui::Color32::from_rgb(r, g, b);
        } else if ansi_code.code.contains("[3") {
            // Foreground color - simplified parsing
            if let Some(&color) = self.color_scheme.ansi_colors.get(&AnsiColor::Red) {
                *current_color = color;
            }
        }
    }

    /// Render cached text
    fn render_cached_text(&self, ui: &mut egui::Ui, rendered: &RenderedText) {
        ui.label(rendered.layout.clone());
    }

    /// Generate cache key for text and ANSI codes
    fn generate_cache_key(&self, text: &str, ansi_codes: &[AnsiCode]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        ansi_codes.len().hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }

    /// Clear render cache
    pub fn clear_cache(&mut self) {
        self.render_cache.clear();
    }

    /// Set font configuration
    pub fn set_font_config(&mut self, config: FontConfig) {
        self.font_config = config;
        self.clear_cache();
    }

    /// Set color scheme
    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        self.clear_cache();
    }

    /// Add custom color mapping
    pub fn add_custom_color(&mut self, name: &str, color: egui::Color32) {
        self.color_scheme.custom_colors.insert(name.to_string(), color);
    }

    /// Get current font configuration
    pub fn font_config(&self) -> &FontConfig {
        &self.font_config
    }

    /// Get current color scheme
    pub fn color_scheme(&self) -> &ColorScheme {
        &self.color_scheme
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            size: self.render_cache.len(),
            max_size: self.max_cache_size,
            hit_rate: 0.0, // Would need to track hits/misses for this
        }
    }

    /// Render a line of output with ANSI formatting
    pub fn render_output_line(&mut self, ui: &mut egui::Ui, line: &crate::models::OutputLine) -> Result<()> {
        self.render_ansi_text(ui, &line.text, &line.ansi_codes)
    }

    /// Render multiple output lines with proper spacing
    pub fn render_output_lines(&mut self, ui: &mut egui::Ui, lines: &[crate::models::OutputLine]) -> Result<()> {
        for (i, line) in lines.iter().enumerate() {
            self.render_output_line(ui, line)?;

            // Add spacing between lines (except for the last one)
            if i < lines.len() - 1 {
                ui.add_space(2.0);
            }
        }
        Ok(())
    }

    /// Create a simple color scheme with basic ANSI colors
    pub fn create_basic_color_scheme() -> ColorScheme {
        let mut ansi_colors = HashMap::new();

        // Basic ANSI colors
        ansi_colors.insert(AnsiColor::Black, egui::Color32::from_rgb(0, 0, 0));
        ansi_colors.insert(AnsiColor::Red, egui::Color32::from_rgb(194, 54, 33));
        ansi_colors.insert(AnsiColor::Green, egui::Color32::from_rgb(37, 188, 36));
        ansi_colors.insert(AnsiColor::Yellow, egui::Color32::from_rgb(173, 173, 39));
        ansi_colors.insert(AnsiColor::Blue, egui::Color32::from_rgb(73, 46, 225));
        ansi_colors.insert(AnsiColor::Magenta, egui::Color32::from_rgb(211, 56, 211));
        ansi_colors.insert(AnsiColor::Cyan, egui::Color32::from_rgb(51, 187, 200));
        ansi_colors.insert(AnsiColor::White, egui::Color32::from_rgb(203, 204, 205));

        // Bright colors
        ansi_colors.insert(AnsiColor::BrightBlack, egui::Color32::from_rgb(129, 131, 131));
        ansi_colors.insert(AnsiColor::BrightRed, egui::Color32::from_rgb(252, 57, 31));
        ansi_colors.insert(AnsiColor::BrightGreen, egui::Color32::from_rgb(49, 231, 34));
        ansi_colors.insert(AnsiColor::BrightYellow, egui::Color32::from_rgb(234, 236, 35));
        ansi_colors.insert(AnsiColor::BrightBlue, egui::Color32::from_rgb(88, 51, 255));
        ansi_colors.insert(AnsiColor::BrightMagenta, egui::Color32::from_rgb(249, 53, 248));
        ansi_colors.insert(AnsiColor::BrightCyan, egui::Color32::from_rgb(20, 240, 252));
        ansi_colors.insert(AnsiColor::BrightWhite, egui::Color32::from_rgb(233, 235, 235));

        ColorScheme {
            default_text: egui::Color32::from_rgb(203, 204, 205),
            default_background: egui::Color32::from_rgb(15, 15, 25),
            ansi_colors,
            custom_colors: HashMap::new(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Current cache size
    pub size: usize,
    /// Maximum cache size
    pub max_size: usize,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f32,
}

/// Text rendering utilities
pub mod utils {
    use super::*;

    /// Strip ANSI codes from text for plain rendering
    pub fn strip_ansi_codes(text: &str) -> String {
        // Simple ANSI stripping - in practice you'd want more robust parsing
        let ansi_regex = regex::Regex::new(r"\x1b\[[0-9;]*[mG]").unwrap();
        ansi_regex.replace_all(text, "").to_string()
    }

    /// Count ANSI codes in text
    pub fn count_ansi_codes(text: &str) -> usize {
        let ansi_regex = regex::Regex::new(r"\x1b\[[0-9;]*[mG]").unwrap();
        ansi_regex.find_iter(text).count()
    }

    /// Check if text contains ANSI formatting
    pub fn has_ansi_codes(text: &str) -> bool {
        text.contains("\x1b[")
    }

    /// Create a default dark theme color scheme
    pub fn create_dark_theme() -> ColorScheme {
        ColorScheme::default()
    }

    /// Create a light theme color scheme
    pub fn create_light_theme() -> ColorScheme {
        let mut ansi_colors = HashMap::new();

        // Light theme ANSI colors
        ansi_colors.insert(AnsiColor::Black, egui::Color32::from_rgb(0, 0, 0));
        ansi_colors.insert(AnsiColor::Red, egui::Color32::from_rgb(195, 39, 43));
        ansi_colors.insert(AnsiColor::Green, egui::Color32::from_rgb(40, 174, 96));
        ansi_colors.insert(AnsiColor::Yellow, egui::Color32::from_rgb(224, 147, 0));
        ansi_colors.insert(AnsiColor::Blue, egui::Color32::from_rgb(66, 113, 174));
        ansi_colors.insert(AnsiColor::Magenta, egui::Color32::from_rgb(170, 60, 135));
        ansi_colors.insert(AnsiColor::Cyan, egui::Color32::from_rgb(0, 163, 181));
        ansi_colors.insert(AnsiColor::White, egui::Color32::from_rgb(36, 36, 36));

        // Bright colors for light theme
        ansi_colors.insert(AnsiColor::BrightBlack, egui::Color32::from_rgb(102, 102, 102));
        ansi_colors.insert(AnsiColor::BrightRed, egui::Color32::from_rgb(237, 85, 59));
        ansi_colors.insert(AnsiColor::BrightGreen, egui::Color32::from_rgb(0, 188, 120));
        ansi_colors.insert(AnsiColor::BrightYellow, egui::Color32::from_rgb(244, 191, 117));
        ansi_colors.insert(AnsiColor::BrightBlue, egui::Color32::from_rgb(59, 142, 234));
        ansi_colors.insert(AnsiColor::BrightMagenta, egui::Color32::from_rgb(214, 112, 214));
        ansi_colors.insert(AnsiColor::BrightCyan, egui::Color32::from_rgb(41, 184, 219));
        ansi_colors.insert(AnsiColor::BrightWhite, egui::Color32::from_rgb(0, 0, 0));

        ColorScheme {
            default_text: egui::Color32::from_rgb(36, 36, 36),
            default_background: egui::Color32::from_rgb(248, 248, 248),
            ansi_colors,
            custom_colors: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_text_renderer_creation() {
        let renderer = AnsiTextRenderer::new();
        assert_eq!(renderer.render_cache.len(), 0);
        assert_eq!(renderer.max_cache_size, 100);
    }

    #[test]
    fn test_font_config_defaults() {
        let config = FontConfig::default();
        assert_eq!(config.size, 14.0);
        assert_eq!(config.monospace_size, 12.0);
        assert_eq!(config.line_height, 1.2);
    }

    #[test]
    fn test_color_scheme_defaults() {
        let scheme = ColorScheme::default();
        assert_eq!(scheme.ansi_colors.len(), 16); // 8 standard + 8 bright
        assert_ne!(scheme.default_text, scheme.default_background);
    }

    #[test]
    fn test_cache_operations() {
        let mut renderer = AnsiTextRenderer::new();

        // Add something to cache
        let cache_key = "test".to_string();
        renderer.render_cache.insert(cache_key.clone(),
            RenderedText {
                layout: egui::epaint::text::LayoutJob::default(),
                dimensions: egui::Vec2::new(100.0, 20.0),
                has_formatting: false,
                ansi_code_count: 0,
            }
        );

        assert_eq!(renderer.render_cache.len(), 1);
        renderer.clear_cache();
        assert_eq!(renderer.render_cache.len(), 0);
    }

    #[test]
    fn test_utils_strip_ansi_codes() {
        let text_with_ansi = "\x1b[31mRed text\x1b[0m normal";
        let stripped = utils::strip_ansi_codes(text_with_ansi);
        assert_eq!(stripped, "Red text normal");
    }

    #[test]
    fn test_utils_count_ansi_codes() {
        let text = "\x1b[31mRed\x1b[32mGreen\x1b[0m";
        let count = utils::count_ansi_codes(text);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_utils_has_ansi_codes() {
        assert!(utils::has_ansi_codes("\x1b[31mRed\x1b[0m"));
        assert!(!utils::has_ansi_codes("Plain text"));
    }

    #[test]
    fn test_utils_create_light_theme() {
        let light_scheme = utils::create_light_theme();
        assert_ne!(light_scheme.default_text, utils::create_dark_theme().default_text);
    }

    #[test]
    fn test_cache_stats() {
        let renderer = AnsiTextRenderer::new();
        let stats = renderer.cache_stats();
        assert_eq!(stats.size, 0);
        assert_eq!(stats.max_size, 100);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn test_generate_cache_key() {
        let renderer = AnsiTextRenderer::new();
        let key1 = renderer.generate_cache_key("test", &[]);
        let key2 = renderer.generate_cache_key("test", &[]);

        // Same input should generate same key
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_add_custom_color() {
        let mut renderer = AnsiTextRenderer::new();
        let custom_color = egui::Color32::from_rgb(255, 0, 255);

        renderer.add_custom_color("custom", custom_color);

        assert_eq!(renderer.color_scheme.custom_colors.get("custom"), Some(&custom_color));
    }
}
