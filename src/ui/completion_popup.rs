//! Completion Popup UI Component
//!
//! Displays auto-completion suggestions in a popup window below the input field.

use crate::completion::{CompletionItem, CompletionResult};
use eframe::egui;

/// Completion popup state and rendering
#[derive(Debug)]
pub struct CompletionPopup {
    /// Whether the popup is visible
    visible: bool,
    /// Current completion result
    completion_result: Option<CompletionResult>,
    /// Selected completion index
    selected_index: usize,
    /// Position to display the popup
    position: Option<egui::Pos2>,
}

impl CompletionPopup {
    /// Create a new completion popup
    pub fn new() -> Self {
        Self {
            visible: false,
            completion_result: None,
            selected_index: 0,
            position: None,
        }
    }

    /// Show the popup with completion results
    pub fn show(&mut self, result: CompletionResult, position: egui::Pos2) {
        if !result.is_empty() {
            self.completion_result = Some(result);
            self.selected_index = 0;
            self.position = Some(position);
            self.visible = true;
        }
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.completion_result = None;
        self.selected_index = 0;
        self.position = None;
    }

    /// Check if popup is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get selected completion item
    pub fn get_selected_item(&self) -> Option<&CompletionItem> {
        self.completion_result
            .as_ref()
            .and_then(|result| result.suggestions.get(self.selected_index))
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if let Some(result) = &self.completion_result {
            self.selected_index = result.suggestions.len().saturating_sub(1);
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if let Some(result) = &self.completion_result {
            self.selected_index = (self.selected_index + 1) % result.suggestions.len();
        }
    }

    /// Get the completion result
    pub fn completion_result(&self) -> Option<&CompletionResult> {
        self.completion_result.as_ref()
    }

    /// Render the completion popup
    pub fn render(&mut self, ctx: &egui::Context, input_rect: egui::Rect) -> Option<String> {
        if !self.visible {
            return None;
        }

        let Some(result) = &self.completion_result else {
            return None;
        };

        if result.is_empty() {
            self.hide();
            return None;
        }

        let mut selected_completion = None;

        // Calculate popup position below the input field
        let popup_pos = egui::pos2(input_rect.left(), input_rect.bottom() + 5.0);

        // Create popup window
        let _window_response = egui::Window::new("Completions")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_pos(popup_pos)
            .frame(egui::Frame {
                inner_margin: egui::Margin::same(8.0),
                outer_margin: egui::Margin::ZERO,
                rounding: egui::Rounding::same(6.0),
                shadow: egui::epaint::Shadow {
                    extrusion: 16.0,
                    color: egui::Color32::from_black_alpha(80),
                },
                fill: egui::Color32::from_rgb(30, 30, 45),
                stroke: egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 100, 150)),
            })
            .show(ctx, |ui| {
                // Header with count
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("📋 {} suggestions", result.len()))
                            .font(egui::FontId::proportional(12.0))
                            .color(egui::Color32::from_rgb(160, 160, 200)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Tab: select  ↑↓: navigate  Esc: close")
                                .font(egui::FontId::proportional(10.0))
                                .color(egui::Color32::from_rgb(120, 120, 140)),
                        );
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // Calculate dynamic height based on number of items
                let item_height = 40.0; // Approximate height per item
                let max_visible_items = 10;
                let num_items = result.suggestions.len().min(max_visible_items);
                let dynamic_height = (num_items as f32 * item_height).min(300.0).max(item_height);

                // Scrollable list of completions
                let scroll_area = egui::ScrollArea::vertical()
                    .max_height(dynamic_height)
                    .auto_shrink([false; 2])
                    .id_source("completion_scroll");

                scroll_area.show(ui, |ui| {
                    // Show all suggestions, not just first 10
                    for (idx, item) in result.suggestions.iter().enumerate() {
                        let is_selected = idx == self.selected_index;

                        // Create completion item frame
                        let item_frame = if is_selected {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(60, 80, 120))
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    egui::Color32::from_rgb(100, 150, 255),
                                ))
                                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                .rounding(egui::Rounding::same(4.0))
                        } else {
                            egui::Frame::none()
                                .fill(egui::Color32::from_rgb(40, 40, 55))
                                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                .rounding(egui::Rounding::same(4.0))
                        };

                        let response = item_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Icon
                                ui.label(
                                    egui::RichText::new(item.get_icon())
                                        .font(egui::FontId::proportional(14.0)),
                                );

                                ui.add_space(6.0);

                                // Completion text
                                let text_color = if is_selected {
                                    egui::Color32::from_rgb(255, 255, 255)
                                } else {
                                    egui::Color32::from_rgb(200, 200, 220)
                                };

                                ui.label(
                                    egui::RichText::new(&item.label)
                                        .font(egui::FontId::monospace(13.0))
                                        .color(text_color),
                                );

                                // Description (if any)
                                if let Some(desc) = &item.description {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(desc)
                                                    .font(egui::FontId::proportional(11.0))
                                                    .color(egui::Color32::from_rgb(140, 140, 160)),
                                            );
                                        },
                                    );
                                }
                            });
                        });

                        // Scroll to selected item
                        if is_selected {
                            ui.scroll_to_rect(response.response.rect, Some(egui::Align::Center));
                        }

                        // Handle mouse clicks
                        if response.response.clicked() {
                            selected_completion = Some(item.text.clone());
                        }

                        // Handle hover to update selection
                        if response.response.hovered() && !is_selected {
                            self.selected_index = idx;
                        }

                        ui.add_space(2.0);
                    }
                });
            });

        // Handle mouse clicks within the popup
        // Keyboard is handled in app.rs to prevent flickering

        selected_completion
    }
}

impl Default for CompletionPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::{CompletionItemType, CompletionType};

    #[test]
    fn test_completion_popup_creation() {
        let popup = CompletionPopup::new();
        assert!(!popup.is_visible());
        assert!(popup.completion_result.is_none());
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_show_hide() {
        let mut popup = CompletionPopup::new();

        let result = CompletionResult {
            suggestions: vec![CompletionItem {
                text: "test".to_string(),
                label: "test".to_string(),
                item_type: CompletionItemType::File,
                description: None,
            }],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result, egui::pos2(0.0, 0.0));
        assert!(popup.is_visible());

        popup.hide();
        assert!(!popup.is_visible());
    }

    #[test]
    fn test_navigation() {
        let mut popup = CompletionPopup::new();

        let result = CompletionResult {
            suggestions: vec![
                CompletionItem {
                    text: "test1".to_string(),
                    label: "test1".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
                CompletionItem {
                    text: "test2".to_string(),
                    label: "test2".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
                CompletionItem {
                    text: "test3".to_string(),
                    label: "test3".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
            ],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result, egui::pos2(0.0, 0.0));

        assert_eq!(popup.selected_index, 0);

        popup.select_next();
        assert_eq!(popup.selected_index, 1);

        popup.select_next();
        assert_eq!(popup.selected_index, 2);

        popup.select_previous();
        assert_eq!(popup.selected_index, 1);

        // Wrap around
        popup.select_next();
        popup.select_next();
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_get_selected_item() {
        let mut popup = CompletionPopup::new();
        let result = CompletionResult {
            suggestions: vec![
                CompletionItem {
                    text: "test1".to_string(),
                    label: "test1".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
                CompletionItem {
                    text: "test2".to_string(),
                    label: "test2".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
            ],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result, egui::pos2(0.0, 0.0));
        assert_eq!(popup.get_selected_item().unwrap().text, "test1");

        popup.select_next();
        assert_eq!(popup.get_selected_item().unwrap().text, "test2");
    }

    #[test]
    fn test_get_selected_item_empty() {
        let popup = CompletionPopup::new();
        assert!(popup.get_selected_item().is_none());
    }

    #[test]
    fn test_select_previous_wraps() {
        let mut popup = CompletionPopup::new();
        let result = CompletionResult {
            suggestions: vec![
                CompletionItem {
                    text: "test1".to_string(),
                    label: "test1".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
                CompletionItem {
                    text: "test2".to_string(),
                    label: "test2".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
            ],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result, egui::pos2(0.0, 0.0));
        assert_eq!(popup.selected_index, 0);

        popup.select_previous(); // Should wrap to last item
        assert_eq!(popup.selected_index, 1);
    }

    #[test]
    fn test_show_with_empty_result() {
        let mut popup = CompletionPopup::new();
        let empty_result = CompletionResult {
            suggestions: vec![],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(empty_result, egui::pos2(0.0, 0.0));
        // Should not show if empty
        assert!(!popup.is_visible());
    }

    #[test]
    fn test_completion_result_accessor() {
        let mut popup = CompletionPopup::new();
        let result = CompletionResult {
            suggestions: vec![CompletionItem {
                text: "test".to_string(),
                label: "test".to_string(),
                item_type: CompletionItemType::File,
                description: None,
            }],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result.clone(), egui::pos2(0.0, 0.0));
        assert!(popup.completion_result().is_some());
        assert_eq!(popup.completion_result().unwrap().len(), 1);

        popup.hide();
        assert!(popup.completion_result().is_none());
    }

    #[test]
    fn test_default_implementation() {
        let popup = CompletionPopup::default();
        assert!(!popup.is_visible());
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_select_next_empty() {
        let mut popup = CompletionPopup::new();
        // Should not panic when no completion result
        popup.select_next();
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_select_previous_empty() {
        let mut popup = CompletionPopup::new();
        // Should not panic when no completion result
        popup.select_previous();
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_hide_resets_state() {
        let mut popup = CompletionPopup::new();
        let result = CompletionResult {
            suggestions: vec![CompletionItem {
                text: "test".to_string(),
                label: "test".to_string(),
                item_type: CompletionItemType::File,
                description: None,
            }],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        popup.show(result, egui::pos2(10.0, 20.0));
        popup.select_next();
        assert_eq!(popup.selected_index, 0); // Wraps around

        popup.hide();
        assert!(!popup.is_visible());
        assert_eq!(popup.selected_index, 0);
        assert!(popup.completion_result().is_none());
        assert!(popup.position.is_none());
    }
}
