//! Theme and Styling Configuration
//!
//! Manages visual themes, colors, and styling for the MosaicTerm interface.

use crate::error::{Error, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Theme manager for MosaicTerm
#[derive(Debug, Clone)]
pub struct ThemeManager {
    /// Available themes
    themes: HashMap<String, Theme>,
    /// Current active theme
    current_theme: String,
    /// System theme detection
    system_theme: SystemTheme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Theme description
    pub description: String,
    /// Theme author
    pub author: Option<String>,
    /// Theme version
    pub version: String,

    /// Color palette
    pub colors: ColorPalette,
    /// Typography settings
    pub typography: Typography,
    /// UI element styles
    pub styles: UiStyles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    /// Background colors
    pub background: BackgroundColors,
    /// Text colors
    pub text: TextColors,
    /// Accent colors
    pub accent: AccentColors,
    /// Status colors
    pub status: StatusColors,
    /// ANSI color mappings
    pub ansi_colors: AnsiColorPalette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundColors {
    /// Main background color
    pub primary: Color,
    /// Secondary background (e.g., for panels)
    pub secondary: Color,
    /// Tertiary background (e.g., for inputs)
    pub tertiary: Color,
    /// Hover background
    pub hover: Color,
    /// Selected background
    pub selected: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColors {
    /// Primary text color
    pub primary: Color,
    /// Secondary text color
    pub secondary: Color,
    /// Tertiary text color
    pub tertiary: Color,
    /// Muted text color
    pub muted: Color,
    /// Error text color
    pub error: Color,
    /// Success text color
    pub success: Color,
    /// Warning text color
    pub warning: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccentColors {
    /// Primary accent color
    pub primary: Color,
    /// Secondary accent color
    pub secondary: Color,
    /// Tertiary accent color
    pub tertiary: Color,
    /// Link color
    pub link: Color,
    /// Border color
    pub border: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusColors {
    /// Success status color
    pub success: Color,
    /// Error status color
    pub error: Color,
    /// Warning status color
    pub warning: Color,
    /// Info status color
    pub info: Color,
    /// Running status color
    pub running: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnsiColorPalette {
    /// ANSI black
    pub black: Color,
    /// ANSI red
    pub red: Color,
    /// ANSI green
    pub green: Color,
    /// ANSI yellow
    pub yellow: Color,
    /// ANSI blue
    pub blue: Color,
    /// ANSI magenta
    pub magenta: Color,
    /// ANSI cyan
    pub cyan: Color,
    /// ANSI white
    pub white: Color,
    /// ANSI bright black
    pub bright_black: Color,
    /// ANSI bright red
    pub bright_red: Color,
    /// ANSI bright green
    pub bright_green: Color,
    /// ANSI bright yellow
    pub bright_yellow: Color,
    /// ANSI bright blue
    pub bright_blue: Color,
    /// ANSI bright magenta
    pub bright_magenta: Color,
    /// ANSI bright cyan
    pub bright_cyan: Color,
    /// ANSI bright white
    pub bright_white: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typography {
    /// Font family for terminal text
    pub terminal_font: FontFamily,
    /// Font family for UI text
    pub ui_font: FontFamily,
    /// Font size for terminal text
    pub terminal_size: f32,
    /// Font size for UI text
    pub ui_size: f32,
    /// Font size for headings
    pub heading_size: f32,
    /// Line height multiplier
    pub line_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontFamily {
    /// Font family name
    pub name: String,
    /// Font weight
    pub weight: FontWeight,
    /// Font style
    pub style: FontStyle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FontWeight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Normal = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiStyles {
    /// Border radius for UI elements
    pub border_radius: f32,
    /// Border width
    pub border_width: f32,
    /// Padding for UI elements
    pub padding: Padding,
    /// Spacing between UI elements
    pub spacing: f32,
    /// Shadow settings
    pub shadow: Option<Shadow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    pub fn to_egui(&self) -> egui::Color32 {
        egui::Color32::from_rgba_premultiplied(
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }

    pub fn hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemTheme {
    Light,
    Dark,
    Auto,
}

/// Colors for specific UI components
#[derive(Debug, Clone)]
pub struct ComponentColors {
    pub background: egui::Color32,
    pub border: egui::Color32,
    pub text: egui::Color32,
    pub accent: egui::Color32,
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new() -> Self {
        let mut manager = Self {
            themes: HashMap::new(),
            current_theme: "default-dark".to_string(),
            system_theme: SystemTheme::Auto,
        };

        // Load built-in themes
        manager.load_builtin_themes();
        manager
    }

    /// Load built-in themes
    fn load_builtin_themes(&mut self) {
        // Default Dark Theme
        let dark_theme = Self::create_dark_theme();
        self.themes.insert("default-dark".to_string(), dark_theme);

        // Default Light Theme
        let light_theme = Self::create_light_theme();
        self.themes.insert("default-light".to_string(), light_theme);

        // High Contrast Theme
        let high_contrast_theme = Self::create_high_contrast_theme();
        self.themes
            .insert("high-contrast".to_string(), high_contrast_theme);
    }

    /// Create the default dark theme
    fn create_dark_theme() -> Theme {
        Theme {
            name: "Default Dark".to_string(),
            description: "A modern dark theme optimized for long coding sessions".to_string(),
            author: Some("MosaicTerm".to_string()),
            version: "1.0.0".to_string(),
            colors: ColorPalette {
                background: BackgroundColors {
                    primary: Color::from_rgb(24, 24, 37),
                    secondary: Color::from_rgb(32, 32, 46),
                    tertiary: Color::from_rgb(40, 40, 54),
                    hover: Color::from_rgb(48, 48, 62),
                    selected: Color::from_rgb(64, 64, 82),
                },
                text: TextColors {
                    primary: Color::from_rgb(229, 229, 229),
                    secondary: Color::from_rgb(178, 178, 178),
                    tertiary: Color::from_rgb(127, 127, 127),
                    muted: Color::from_rgb(102, 102, 102),
                    error: Color::from_rgb(241, 76, 76),
                    success: Color::from_rgb(46, 204, 113),
                    warning: Color::from_rgb(241, 196, 15),
                },
                accent: AccentColors {
                    primary: Color::from_rgb(100, 150, 255),
                    secondary: Color::from_rgb(138, 173, 244),
                    tertiary: Color::from_rgb(176, 196, 222),
                    link: Color::from_rgb(138, 173, 244),
                    border: Color::from_rgb(64, 64, 82),
                },
                status: StatusColors {
                    success: Color::from_rgb(46, 204, 113),
                    error: Color::from_rgb(241, 76, 76),
                    warning: Color::from_rgb(241, 196, 15),
                    info: Color::from_rgb(52, 152, 219),
                    running: Color::from_rgb(155, 89, 182),
                },
                ansi_colors: AnsiColorPalette {
                    black: Color::from_rgb(0, 0, 0),
                    red: Color::from_rgb(205, 49, 49),
                    green: Color::from_rgb(13, 188, 121),
                    yellow: Color::from_rgb(229, 229, 16),
                    blue: Color::from_rgb(36, 114, 200),
                    magenta: Color::from_rgb(188, 63, 188),
                    cyan: Color::from_rgb(17, 168, 205),
                    white: Color::from_rgb(229, 229, 229),
                    bright_black: Color::from_rgb(102, 102, 102),
                    bright_red: Color::from_rgb(241, 76, 76),
                    bright_green: Color::from_rgb(35, 209, 139),
                    bright_yellow: Color::from_rgb(245, 245, 67),
                    bright_blue: Color::from_rgb(59, 142, 234),
                    bright_magenta: Color::from_rgb(214, 112, 214),
                    bright_cyan: Color::from_rgb(41, 184, 219),
                    bright_white: Color::from_rgb(229, 229, 229),
                },
            },
            typography: Typography {
                terminal_font: FontFamily {
                    name: "JetBrains Mono".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                ui_font: FontFamily {
                    name: "Inter".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                terminal_size: 12.0,
                ui_size: 14.0,
                heading_size: 18.0,
                line_height: 1.4,
            },
            styles: UiStyles {
                border_radius: 6.0,
                border_width: 1.0,
                padding: Padding {
                    top: 8.0,
                    right: 12.0,
                    bottom: 8.0,
                    left: 12.0,
                },
                spacing: 8.0,
                shadow: Some(Shadow {
                    color: Color::from_rgba(0, 0, 0, 64),
                    offset_x: 0.0,
                    offset_y: 2.0,
                    blur: 8.0,
                    spread: 0.0,
                }),
            },
        }
    }

    /// Create the default light theme
    fn create_light_theme() -> Theme {
        Theme {
            name: "Default Light".to_string(),
            description: "A clean light theme for daytime use".to_string(),
            author: Some("MosaicTerm".to_string()),
            version: "1.0.0".to_string(),
            colors: ColorPalette {
                background: BackgroundColors {
                    primary: Color::from_rgb(248, 248, 248),
                    secondary: Color::from_rgb(240, 240, 240),
                    tertiary: Color::from_rgb(232, 232, 232),
                    hover: Color::from_rgb(224, 224, 224),
                    selected: Color::from_rgb(200, 200, 200),
                },
                text: TextColors {
                    primary: Color::from_rgb(36, 36, 36),
                    secondary: Color::from_rgb(77, 77, 77),
                    tertiary: Color::from_rgb(118, 118, 118),
                    muted: Color::from_rgb(153, 153, 153),
                    error: Color::from_rgb(220, 53, 69),
                    success: Color::from_rgb(40, 167, 69),
                    warning: Color::from_rgb(255, 193, 7),
                },
                accent: AccentColors {
                    primary: Color::from_rgb(0, 123, 255),
                    secondary: Color::from_rgb(108, 117, 125),
                    tertiary: Color::from_rgb(52, 58, 64),
                    link: Color::from_rgb(0, 123, 255),
                    border: Color::from_rgb(200, 200, 200),
                },
                status: StatusColors {
                    success: Color::from_rgb(40, 167, 69),
                    error: Color::from_rgb(220, 53, 69),
                    warning: Color::from_rgb(255, 193, 7),
                    info: Color::from_rgb(23, 162, 184),
                    running: Color::from_rgb(111, 66, 193),
                },
                ansi_colors: AnsiColorPalette {
                    black: Color::from_rgb(0, 0, 0),
                    red: Color::from_rgb(195, 39, 43),
                    green: Color::from_rgb(40, 174, 96),
                    yellow: Color::from_rgb(224, 147, 0),
                    blue: Color::from_rgb(66, 113, 174),
                    magenta: Color::from_rgb(170, 60, 135),
                    cyan: Color::from_rgb(0, 163, 181),
                    white: Color::from_rgb(36, 36, 36),
                    bright_black: Color::from_rgb(102, 102, 102),
                    bright_red: Color::from_rgb(237, 85, 59),
                    bright_green: Color::from_rgb(0, 188, 120),
                    bright_yellow: Color::from_rgb(244, 191, 117),
                    bright_blue: Color::from_rgb(59, 142, 234),
                    bright_magenta: Color::from_rgb(214, 112, 214),
                    bright_cyan: Color::from_rgb(41, 184, 219),
                    bright_white: Color::from_rgb(0, 0, 0),
                },
            },
            typography: Typography {
                terminal_font: FontFamily {
                    name: "JetBrains Mono".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                ui_font: FontFamily {
                    name: "Inter".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                terminal_size: 12.0,
                ui_size: 14.0,
                heading_size: 18.0,
                line_height: 1.4,
            },
            styles: UiStyles {
                border_radius: 6.0,
                border_width: 1.0,
                padding: Padding {
                    top: 8.0,
                    right: 12.0,
                    bottom: 8.0,
                    left: 12.0,
                },
                spacing: 8.0,
                shadow: Some(Shadow {
                    color: Color::from_rgba(0, 0, 0, 32),
                    offset_x: 0.0,
                    offset_y: 1.0,
                    blur: 4.0,
                    spread: 0.0,
                }),
            },
        }
    }

    /// Create the high contrast theme
    fn create_high_contrast_theme() -> Theme {
        Theme {
            name: "High Contrast".to_string(),
            description: "High contrast theme for accessibility".to_string(),
            author: Some("MosaicTerm".to_string()),
            version: "1.0.0".to_string(),
            colors: ColorPalette {
                background: BackgroundColors {
                    primary: Color::from_rgb(0, 0, 0),
                    secondary: Color::from_rgb(32, 32, 32),
                    tertiary: Color::from_rgb(64, 64, 64),
                    hover: Color::from_rgb(96, 96, 96),
                    selected: Color::from_rgb(128, 128, 128),
                },
                text: TextColors {
                    primary: Color::from_rgb(255, 255, 255),
                    secondary: Color::from_rgb(200, 200, 200),
                    tertiary: Color::from_rgb(150, 150, 150),
                    muted: Color::from_rgb(100, 100, 100),
                    error: Color::from_rgb(255, 100, 100),
                    success: Color::from_rgb(100, 255, 100),
                    warning: Color::from_rgb(255, 255, 100),
                },
                accent: AccentColors {
                    primary: Color::from_rgb(255, 255, 255),
                    secondary: Color::from_rgb(200, 200, 200),
                    tertiary: Color::from_rgb(150, 150, 150),
                    link: Color::from_rgb(100, 200, 255),
                    border: Color::from_rgb(255, 255, 255),
                },
                status: StatusColors {
                    success: Color::from_rgb(0, 255, 0),
                    error: Color::from_rgb(255, 0, 0),
                    warning: Color::from_rgb(255, 255, 0),
                    info: Color::from_rgb(0, 255, 255),
                    running: Color::from_rgb(255, 0, 255),
                },
                ansi_colors: AnsiColorPalette {
                    black: Color::from_rgb(0, 0, 0),
                    red: Color::from_rgb(255, 0, 0),
                    green: Color::from_rgb(0, 255, 0),
                    yellow: Color::from_rgb(255, 255, 0),
                    blue: Color::from_rgb(0, 0, 255),
                    magenta: Color::from_rgb(255, 0, 255),
                    cyan: Color::from_rgb(0, 255, 255),
                    white: Color::from_rgb(255, 255, 255),
                    bright_black: Color::from_rgb(128, 128, 128),
                    bright_red: Color::from_rgb(255, 128, 128),
                    bright_green: Color::from_rgb(128, 255, 128),
                    bright_yellow: Color::from_rgb(255, 255, 128),
                    bright_blue: Color::from_rgb(128, 128, 255),
                    bright_magenta: Color::from_rgb(255, 128, 255),
                    bright_cyan: Color::from_rgb(128, 255, 255),
                    bright_white: Color::from_rgb(255, 255, 255),
                },
            },
            typography: Typography {
                terminal_font: FontFamily {
                    name: "JetBrains Mono".to_string(),
                    weight: FontWeight::Bold,
                    style: FontStyle::Normal,
                },
                ui_font: FontFamily {
                    name: "Inter".to_string(),
                    weight: FontWeight::Bold,
                    style: FontStyle::Normal,
                },
                terminal_size: 14.0,
                ui_size: 16.0,
                heading_size: 20.0,
                line_height: 1.5,
            },
            styles: UiStyles {
                border_radius: 2.0,
                border_width: 2.0,
                padding: Padding {
                    top: 10.0,
                    right: 14.0,
                    bottom: 10.0,
                    left: 14.0,
                },
                spacing: 12.0,
                shadow: Some(Shadow {
                    color: Color::from_rgb(255, 255, 255),
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur: 0.0,
                    spread: 2.0,
                }),
            },
        }
    }

    /// Get the current theme
    pub fn current_theme(&self) -> Result<&Theme> {
        self.themes
            .get(&self.current_theme)
            .ok_or_else(|| Error::ThemeNotFound {
                theme_name: self.current_theme.clone(),
            })
    }

    /// Get a mutable reference to the current theme
    pub fn current_theme_mut(&mut self) -> Result<&mut Theme> {
        self.themes
            .get_mut(&self.current_theme)
            .ok_or_else(|| Error::ThemeNotFound {
                theme_name: self.current_theme.clone(),
            })
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme_name: &str) -> Result<()> {
        if self.themes.contains_key(theme_name) {
            self.current_theme = theme_name.to_string();
            Ok(())
        } else {
            Err(Error::ThemeNotFound {
                theme_name: theme_name.to_string(),
            })
        }
    }

    /// Add a custom theme
    pub fn add_theme(&mut self, theme: Theme) -> Result<()> {
        if self.themes.contains_key(&theme.name) {
            return Err(Error::ThemeAlreadyExists {
                theme_name: theme.name.clone(),
            });
        }
        self.themes.insert(theme.name.clone(), theme);
        Ok(())
    }

    /// Remove a theme
    pub fn remove_theme(&mut self, theme_name: &str) -> Result<()> {
        if theme_name.starts_with("default-") {
            return Err(Error::CannotRemoveBuiltInTheme {
                theme_name: theme_name.to_string(),
            });
        }

        if self.themes.remove(theme_name).is_some() {
            // If we removed the current theme, switch to default
            if self.current_theme == theme_name {
                self.current_theme = "default-dark".to_string();
            }
            Ok(())
        } else {
            Err(Error::ThemeNotFound {
                theme_name: theme_name.to_string(),
            })
        }
    }

    /// List all available themes
    pub fn list_themes(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }

    /// Get the system theme
    pub fn system_theme(&self) -> SystemTheme {
        self.system_theme
    }

    /// Set the system theme preference
    pub fn set_system_theme(&mut self, theme: SystemTheme) {
        self.system_theme = theme;
    }

    /// Auto-detect and apply appropriate theme based on system settings
    pub fn apply_system_theme(&mut self) {
        match self.system_theme {
            SystemTheme::Light => {
                self.set_theme("default-light").unwrap_or(());
            }
            SystemTheme::Dark => {
                self.set_theme("default-dark").unwrap_or(());
            }
            SystemTheme::Auto => {
                // For now, default to dark theme
                // In a real implementation, this would detect system theme
                self.set_theme("default-dark").unwrap_or(());
            }
        }
    }

    /// Export theme to JSON string
    pub fn export_theme(&self, theme_name: &str) -> Result<String> {
        if let Some(theme) = self.themes.get(theme_name) {
            serde_json::to_string_pretty(theme).map_err(|e| Error::ThemeExportFailed {
                theme_name: theme_name.to_string(),
                reason: e.to_string(),
            })
        } else {
            Err(Error::ThemeNotFound {
                theme_name: theme_name.to_string(),
            })
        }
    }

    /// Import theme from JSON string
    pub fn import_theme(&mut self, json: &str) -> Result<String> {
        let theme: Theme = serde_json::from_str(json).map_err(|e| Error::ThemeImportFailed {
            reason: e.to_string(),
        })?;

        let theme_name = theme.name.clone();
        self.add_theme(theme)?;
        Ok(theme_name)
    }

    /// Apply the current theme to an egui context
    pub fn apply_to_egui(&self, ctx: &egui::Context) -> Result<()> {
        let theme = self.current_theme()?;

        // Apply visual style
        let mut style = (*ctx.style()).clone();

        // Set overall theme mode
        style.visuals.dark_mode = matches!(theme.name.as_str(), "default-dark" | "high-contrast");

        // Apply background colors
        style.visuals.window_fill = theme.colors.background.primary.to_egui();
        style.visuals.panel_fill = theme.colors.background.secondary.to_egui();

        // Apply text colors
        style.visuals.override_text_color = Some(theme.colors.text.primary.to_egui());

        // Apply widget styling
        style.visuals.widgets.noninteractive.bg_fill = theme.colors.background.secondary.to_egui();
        style.visuals.widgets.noninteractive.fg_stroke.color = theme.colors.text.primary.to_egui();

        style.visuals.widgets.inactive.bg_fill = theme.colors.background.secondary.to_egui();
        style.visuals.widgets.inactive.fg_stroke.color = theme.colors.text.primary.to_egui();

        style.visuals.widgets.hovered.bg_fill = theme.colors.background.hover.to_egui();
        style.visuals.widgets.hovered.fg_stroke.color = theme.colors.text.primary.to_egui();

        style.visuals.widgets.active.bg_fill = theme.colors.background.selected.to_egui();
        style.visuals.widgets.active.fg_stroke.color = theme.colors.text.primary.to_egui();

        style.visuals.widgets.open.bg_fill = theme.colors.background.selected.to_egui();
        style.visuals.widgets.open.fg_stroke.color = theme.colors.text.primary.to_egui();

        // Apply selection colors
        style.visuals.selection.bg_fill = theme.colors.accent.primary.to_egui();
        style.visuals.selection.stroke.color = theme.colors.accent.secondary.to_egui();

        // Apply hyperlink colors
        style.visuals.hyperlink_color = theme.colors.accent.primary.to_egui();

        // Apply window shadow
        style.visuals.window_shadow = egui::epaint::Shadow {
            extrusion: 8.0,
            color: egui::Color32::from_rgba_premultiplied(0, 0, 0, 40),
        };

        // Apply popup shadow
        style.visuals.popup_shadow = style.visuals.window_shadow;

        // Apply scrollbar styling
        // Scrollbar colors are handled differently in newer egui versions
        // These fields may not be available or may have different names

        // Apply spacing
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.menu_margin = egui::Margin::same(6.0);
        style.spacing.indent = 18.0;

        // Apply font settings
        let mono_font = egui::FontId::monospace(theme.typography.terminal_size);
        let prop_font = egui::FontId::proportional(theme.typography.ui_size);

        style
            .text_styles
            .insert(egui::TextStyle::Monospace, mono_font);
        style
            .text_styles
            .insert(egui::TextStyle::Body, prop_font.clone());
        style
            .text_styles
            .insert(egui::TextStyle::Button, prop_font.clone());
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(theme.typography.heading_size),
        );

        ctx.set_style(style);

        Ok(())
    }

    /// Create a color scheme for terminal output
    pub fn create_terminal_color_scheme(&self) -> Result<crate::ui::text::ColorScheme> {
        let theme = self.current_theme()?;

        let mut ansi_colors = HashMap::new();

        // Basic ANSI colors
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Black,
            theme.colors.ansi_colors.black.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Red,
            theme.colors.ansi_colors.red.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Green,
            theme.colors.ansi_colors.green.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Yellow,
            theme.colors.ansi_colors.yellow.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Blue,
            theme.colors.ansi_colors.blue.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Magenta,
            theme.colors.ansi_colors.magenta.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::Cyan,
            theme.colors.ansi_colors.cyan.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::White,
            theme.colors.ansi_colors.white.to_egui(),
        );

        // Bright ANSI colors
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightBlack,
            theme.colors.ansi_colors.bright_black.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightRed,
            theme.colors.ansi_colors.bright_red.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightGreen,
            theme.colors.ansi_colors.bright_green.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightYellow,
            theme.colors.ansi_colors.bright_yellow.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightBlue,
            theme.colors.ansi_colors.bright_blue.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightMagenta,
            theme.colors.ansi_colors.bright_magenta.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightCyan,
            theme.colors.ansi_colors.bright_cyan.to_egui(),
        );
        ansi_colors.insert(
            crate::terminal::ansi_parser::AnsiColor::BrightWhite,
            theme.colors.ansi_colors.bright_white.to_egui(),
        );

        Ok(crate::ui::text::ColorScheme {
            default_text: theme.colors.text.primary.to_egui(),
            default_background: theme.colors.background.primary.to_egui(),
            ansi_colors,
            custom_colors: HashMap::new(),
        })
    }

    /// Get theme colors for UI components
    pub fn get_component_colors(&self, component: &str) -> Result<ComponentColors> {
        let theme = self.current_theme()?;

        match component {
            "command_block" => Ok(ComponentColors {
                background: theme.colors.background.secondary.to_egui(),
                border: theme.colors.background.tertiary.to_egui(),
                text: theme.colors.text.primary.to_egui(),
                accent: theme.colors.accent.primary.to_egui(),
            }),
            "input_prompt" => Ok(ComponentColors {
                background: theme.colors.background.secondary.to_egui(),
                border: theme.colors.accent.primary.to_egui(),
                text: theme.colors.text.primary.to_egui(),
                accent: theme.colors.accent.secondary.to_egui(),
            }),
            "status_bar" => Ok(ComponentColors {
                background: theme.colors.background.tertiary.to_egui(),
                border: theme.colors.background.hover.to_egui(),
                text: theme.colors.text.secondary.to_egui(),
                accent: theme.colors.status.success.to_egui(),
            }),
            _ => Err(Error::UnknownComponent {
                component: component.to_string(),
            }),
        }
    }

    /// Apply a specific color scheme preset
    pub fn apply_color_scheme(&mut self, scheme_name: &str) -> Result<()> {
        match scheme_name {
            "monokai" => self.apply_monokai_scheme(),
            "solarized_dark" => self.apply_solarized_dark_scheme(),
            "solarized_light" => self.apply_solarized_light_scheme(),
            "dracula" => self.apply_dracula_scheme(),
            "nord" => self.apply_nord_scheme(),
            _ => Err(Error::UnknownColorScheme {
                scheme: scheme_name.to_string(),
            }),
        }
    }

    /// Apply Monokai color scheme
    fn apply_monokai_scheme(&mut self) -> Result<()> {
        let theme = self.current_theme_mut()?;
        theme.colors.ansi_colors = AnsiColorPalette {
            black: Color::from_rgb(39, 40, 34),
            red: Color::from_rgb(249, 38, 114),
            green: Color::from_rgb(166, 226, 46),
            yellow: Color::from_rgb(253, 151, 31),
            blue: Color::from_rgb(102, 217, 239),
            magenta: Color::from_rgb(174, 129, 255),
            cyan: Color::from_rgb(23, 198, 163),
            white: Color::from_rgb(248, 248, 242),
            bright_black: Color::from_rgb(101, 123, 131),
            bright_red: Color::from_rgb(253, 151, 31),
            bright_green: Color::from_rgb(166, 226, 46),
            bright_yellow: Color::from_rgb(249, 38, 114),
            bright_blue: Color::from_rgb(102, 217, 239),
            bright_magenta: Color::from_rgb(174, 129, 255),
            bright_cyan: Color::from_rgb(23, 198, 163),
            bright_white: Color::from_rgb(253, 253, 253),
        };
        Ok(())
    }

    /// Apply Solarized Dark color scheme
    fn apply_solarized_dark_scheme(&mut self) -> Result<()> {
        let theme = self.current_theme_mut()?;
        theme.colors.ansi_colors = AnsiColorPalette {
            black: Color::from_rgb(0, 43, 54),
            red: Color::from_rgb(220, 50, 47),
            green: Color::from_rgb(133, 153, 0),
            yellow: Color::from_rgb(181, 137, 0),
            blue: Color::from_rgb(38, 139, 210),
            magenta: Color::from_rgb(211, 54, 130),
            cyan: Color::from_rgb(42, 161, 152),
            white: Color::from_rgb(238, 232, 213),
            bright_black: Color::from_rgb(7, 54, 66),
            bright_red: Color::from_rgb(203, 75, 22),
            bright_green: Color::from_rgb(88, 110, 117),
            bright_yellow: Color::from_rgb(101, 123, 131),
            bright_blue: Color::from_rgb(131, 148, 150),
            bright_magenta: Color::from_rgb(108, 113, 196),
            bright_cyan: Color::from_rgb(147, 161, 161),
            bright_white: Color::from_rgb(253, 246, 227),
        };
        Ok(())
    }

    /// Apply Solarized Light color scheme
    fn apply_solarized_light_scheme(&mut self) -> Result<()> {
        let theme = self.current_theme_mut()?;
        theme.colors.ansi_colors = AnsiColorPalette {
            black: Color::from_rgb(101, 123, 131),
            red: Color::from_rgb(220, 50, 47),
            green: Color::from_rgb(133, 153, 0),
            yellow: Color::from_rgb(181, 137, 0),
            blue: Color::from_rgb(38, 139, 210),
            magenta: Color::from_rgb(211, 54, 130),
            cyan: Color::from_rgb(42, 161, 152),
            white: Color::from_rgb(253, 246, 227),
            bright_black: Color::from_rgb(88, 110, 117),
            bright_red: Color::from_rgb(203, 75, 22),
            bright_green: Color::from_rgb(7, 54, 66),
            bright_yellow: Color::from_rgb(0, 43, 54),
            bright_blue: Color::from_rgb(131, 148, 150),
            bright_magenta: Color::from_rgb(108, 113, 196),
            bright_cyan: Color::from_rgb(147, 161, 161),
            bright_white: Color::from_rgb(238, 232, 213),
        };
        Ok(())
    }

    /// Apply Dracula color scheme
    fn apply_dracula_scheme(&mut self) -> Result<()> {
        let theme = self.current_theme_mut()?;
        theme.colors.ansi_colors = AnsiColorPalette {
            black: Color::from_rgb(40, 42, 54),
            red: Color::from_rgb(255, 85, 85),
            green: Color::from_rgb(80, 250, 123),
            yellow: Color::from_rgb(241, 250, 140),
            blue: Color::from_rgb(189, 147, 249),
            magenta: Color::from_rgb(255, 121, 198),
            cyan: Color::from_rgb(139, 233, 253),
            white: Color::from_rgb(248, 248, 242),
            bright_black: Color::from_rgb(68, 71, 90),
            bright_red: Color::from_rgb(255, 85, 85),
            bright_green: Color::from_rgb(80, 250, 123),
            bright_yellow: Color::from_rgb(241, 250, 140),
            bright_blue: Color::from_rgb(189, 147, 249),
            bright_magenta: Color::from_rgb(255, 121, 198),
            bright_cyan: Color::from_rgb(139, 233, 253),
            bright_white: Color::from_rgb(255, 255, 255),
        };
        Ok(())
    }

    /// Apply Nord color scheme
    fn apply_nord_scheme(&mut self) -> Result<()> {
        let theme = self.current_theme_mut()?;
        theme.colors.ansi_colors = AnsiColorPalette {
            black: Color::from_rgb(46, 52, 64),
            red: Color::from_rgb(191, 97, 106),
            green: Color::from_rgb(163, 190, 140),
            yellow: Color::from_rgb(235, 203, 139),
            blue: Color::from_rgb(129, 161, 193),
            magenta: Color::from_rgb(180, 142, 173),
            cyan: Color::from_rgb(136, 192, 208),
            white: Color::from_rgb(236, 239, 244),
            bright_black: Color::from_rgb(59, 66, 82),
            bright_red: Color::from_rgb(191, 97, 106),
            bright_green: Color::from_rgb(163, 190, 140),
            bright_yellow: Color::from_rgb(235, 203, 139),
            bright_blue: Color::from_rgb(129, 161, 193),
            bright_magenta: Color::from_rgb(180, 142, 173),
            bright_cyan: Color::from_rgb(136, 192, 208),
            bright_white: Color::from_rgb(255, 255, 255),
        };
        Ok(())
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Theme utilities
pub mod utils {
    use super::*;

    /// Calculate contrast ratio between two colors
    pub fn contrast_ratio(color1: &Color, color2: &Color) -> f32 {
        // Simplified contrast ratio calculation
        // In practice, you'd want to convert to luminance first
        let r_diff = (color1.r - color2.r).abs();
        let g_diff = (color1.g - color2.g).abs();
        let b_diff = (color1.b - color2.b).abs();

        (r_diff + g_diff + b_diff) / 3.0
    }

    /// Check if a color is light
    pub fn is_light(color: &Color) -> bool {
        // Calculate perceived brightness
        let brightness = (color.r * 299.0 + color.g * 587.0 + color.b * 114.0) / 1000.0;
        brightness > 0.5
    }

    /// Generate a complementary color
    pub fn complementary_color(color: &Color) -> Color {
        Color::new(1.0 - color.r, 1.0 - color.g, 1.0 - color.b, color.a)
    }

    /// Create a color with adjusted brightness
    pub fn adjust_brightness(color: &Color, factor: f32) -> Color {
        let factor = factor.clamp(0.0, 2.0);
        Color::new(
            (color.r * factor).min(1.0),
            (color.g * factor).min(1.0),
            (color.b * factor).min(1.0),
            color.a,
        )
    }

    /// Blend two colors
    pub fn blend_colors(color1: &Color, color2: &Color, ratio: f32) -> Color {
        let ratio = ratio.clamp(0.0, 1.0);
        let inv_ratio = 1.0 - ratio;

        Color::new(
            color1.r * ratio + color2.r * inv_ratio,
            color1.g * ratio + color2.g * inv_ratio,
            color1.b * ratio + color2.b * inv_ratio,
            color1.a * ratio + color2.a * inv_ratio,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_creation() {
        let color = Color::new(1.0, 0.5, 0.0, 1.0);
        assert_eq!(color.r, 1.0);
        assert_eq!(color.g, 0.5);
        assert_eq!(color.b, 0.0);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_color_from_rgb() {
        let color = Color::from_rgb(255, 128, 0);
        assert_eq!(color.r, 1.0);
        assert_eq!(color.g, 128.0 / 255.0);
        assert_eq!(color.b, 0.0);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_color_hex() {
        let color = Color::from_rgb(255, 128, 64);
        let hex = color.hex();
        assert_eq!(hex, "#ff8040ff");
    }

    #[test]
    fn test_theme_manager_creation() {
        let manager = ThemeManager::new();
        assert!(manager.themes.contains_key("default-dark"));
        assert!(manager.themes.contains_key("default-light"));
        assert!(manager.themes.contains_key("high-contrast"));
        assert_eq!(manager.current_theme, "default-dark");
    }

    #[test]
    fn test_theme_switching() {
        let mut manager = ThemeManager::new();

        assert!(manager.set_theme("default-light").is_ok());
        assert_eq!(manager.current_theme, "default-light");

        assert!(manager.set_theme("nonexistent").is_err());
    }

    #[test]
    fn test_custom_theme() {
        let mut manager = ThemeManager::new();

        let custom_theme = Theme {
            name: "custom".to_string(),
            description: "Custom theme".to_string(),
            author: Some("Test".to_string()),
            version: "1.0.0".to_string(),
            colors: ColorPalette {
                background: BackgroundColors {
                    primary: Color::from_rgb(255, 255, 255),
                    secondary: Color::from_rgb(240, 240, 240),
                    tertiary: Color::from_rgb(220, 220, 220),
                    hover: Color::from_rgb(200, 200, 200),
                    selected: Color::from_rgb(180, 180, 180),
                },
                text: TextColors {
                    primary: Color::from_rgb(0, 0, 0),
                    secondary: Color::from_rgb(64, 64, 64),
                    tertiary: Color::from_rgb(128, 128, 128),
                    muted: Color::from_rgb(160, 160, 160),
                    error: Color::from_rgb(220, 53, 69),
                    success: Color::from_rgb(40, 167, 69),
                    warning: Color::from_rgb(255, 193, 7),
                },
                accent: AccentColors {
                    primary: Color::from_rgb(0, 123, 255),
                    secondary: Color::from_rgb(108, 117, 125),
                    tertiary: Color::from_rgb(52, 58, 64),
                    link: Color::from_rgb(0, 123, 255),
                    border: Color::from_rgb(200, 200, 200),
                },
                status: StatusColors {
                    success: Color::from_rgb(40, 167, 69),
                    error: Color::from_rgb(220, 53, 69),
                    warning: Color::from_rgb(255, 193, 7),
                    info: Color::from_rgb(23, 162, 184),
                    running: Color::from_rgb(111, 66, 193),
                },
                ansi_colors: AnsiColorPalette {
                    black: Color::from_rgb(0, 0, 0),
                    red: Color::from_rgb(195, 39, 43),
                    green: Color::from_rgb(40, 174, 96),
                    yellow: Color::from_rgb(224, 147, 0),
                    blue: Color::from_rgb(66, 113, 174),
                    magenta: Color::from_rgb(170, 60, 135),
                    cyan: Color::from_rgb(0, 163, 181),
                    white: Color::from_rgb(36, 36, 36),
                    bright_black: Color::from_rgb(102, 102, 102),
                    bright_red: Color::from_rgb(237, 85, 59),
                    bright_green: Color::from_rgb(0, 188, 120),
                    bright_yellow: Color::from_rgb(244, 191, 117),
                    bright_blue: Color::from_rgb(59, 142, 234),
                    bright_magenta: Color::from_rgb(214, 112, 214),
                    bright_cyan: Color::from_rgb(41, 184, 219),
                    bright_white: Color::from_rgb(0, 0, 0),
                },
            },
            typography: Typography {
                terminal_font: FontFamily {
                    name: "JetBrains Mono".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                ui_font: FontFamily {
                    name: "Inter".to_string(),
                    weight: FontWeight::Normal,
                    style: FontStyle::Normal,
                },
                terminal_size: 12.0,
                ui_size: 14.0,
                heading_size: 18.0,
                line_height: 1.4,
            },
            styles: UiStyles {
                border_radius: 6.0,
                border_width: 1.0,
                padding: Padding {
                    top: 8.0,
                    right: 12.0,
                    bottom: 8.0,
                    left: 12.0,
                },
                spacing: 8.0,
                shadow: None,
            },
        };

        assert!(manager.add_theme(custom_theme).is_ok());
        assert!(manager.themes.contains_key("custom"));
        assert!(manager.set_theme("custom").is_ok());
    }

    #[test]
    fn test_theme_export_import() {
        let manager = ThemeManager::new();

        // Export a theme
        let exported = manager.export_theme("default-dark").unwrap();

        // Should be valid JSON
        assert!(exported.contains("Default Dark"));

        // Import the theme
        let mut new_manager = ThemeManager::new();
        let theme_name = new_manager.import_theme(&exported).unwrap();

        assert_eq!(theme_name, "Default Dark");
    }

    #[test]
    fn test_utils_contrast_ratio() {
        let color1 = Color::from_rgb(255, 255, 255);
        let color2 = Color::from_rgb(0, 0, 0);

        let ratio = utils::contrast_ratio(&color1, &color2);
        assert!(ratio > 0.0);
    }

    #[test]
    fn test_utils_is_light() {
        let light_color = Color::from_rgb(200, 200, 200);
        let dark_color = Color::from_rgb(50, 50, 50);

        assert!(utils::is_light(&light_color));
        assert!(!utils::is_light(&dark_color));
    }

    #[test]
    fn test_utils_adjust_brightness() {
        let color = Color::from_rgb(100, 100, 100);
        let brighter = utils::adjust_brightness(&color, 1.5);

        assert!(brighter.r > color.r);
        assert!(brighter.g > color.g);
        assert!(brighter.b > color.b);
    }

    #[test]
    fn test_font_weight_values() {
        assert_eq!(FontWeight::Normal as u32, 400);
        assert_eq!(FontWeight::Bold as u32, 700);
        assert_eq!(FontWeight::Thin as u32, 100);
        assert_eq!(FontWeight::Black as u32, 900);
    }
}
