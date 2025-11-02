//! Performance metrics UI panel
//!
//! Displays real-time performance statistics including:
//! - CPU usage, memory usage
//! - Command counts, output line counts
//! - Uptime, active PTY processes
//! - Recent performance history

use egui::{Color32, RichText, Ui};
use std::time::{Duration, Instant};

use crate::state_manager::AppStatistics;

/// Performance metrics panel widget
pub struct MetricsPanel {
    /// Whether the panel is visible
    visible: bool,
    /// Last update time
    last_update: Instant,
    /// Update interval
    update_interval: Duration,
}

impl Default for MetricsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsPanel {
    /// Create a new metrics panel
    pub fn new() -> Self {
        Self {
            visible: false,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
        }
    }

    /// Toggle panel visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Check if panel is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set panel visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Render the metrics panel
    pub fn render(&mut self, ui: &mut Ui, stats: &AppStatistics, pty_count: usize) {
        if !self.visible {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_update) < self.update_interval {
            return;
        }
        self.last_update = now;

        egui::Window::new("⚡ Performance Metrics")
            .collapsible(true)
            .resizable(true)
            .default_width(350.0)
            .show(ui.ctx(), |ui| {
                ui.set_min_width(300.0);

                // System section
                ui.heading(RichText::new("📊 System").size(14.0).strong());
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Uptime:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let uptime = format_duration(stats.uptime());
                        ui.label(RichText::new(uptime).color(Color32::GREEN));
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Memory (current):");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mem = format_bytes(stats.current_memory_bytes);
                        ui.label(
                            RichText::new(mem).color(color_for_memory(stats.current_memory_bytes)),
                        );
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Memory (peak):");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mem = format_bytes(stats.peak_memory_bytes);
                        ui.label(RichText::new(mem).color(Color32::YELLOW));
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Active PTY processes:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", pty_count)).color(Color32::LIGHT_BLUE),
                        );
                    });
                });

                ui.add_space(8.0);

                // Commands section
                ui.heading(RichText::new("⌨️  Commands").size(14.0).strong());
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Total commands:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", stats.total_commands))
                                .color(Color32::LIGHT_BLUE),
                        );
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Successful:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", stats.successful_commands))
                                .color(Color32::GREEN),
                        );
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Failed:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", stats.failed_commands)).color(Color32::RED),
                        );
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Cancelled:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", stats.cancelled_commands))
                                .color(Color32::YELLOW),
                        );
                    });
                });

                if stats.total_commands > 0 {
                    let success_rate =
                        (stats.successful_commands as f32 / stats.total_commands as f32) * 100.0;
                    ui.horizontal(|ui| {
                        ui.label("Success rate:");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:.1}%", success_rate))
                                    .color(color_for_success_rate(success_rate)),
                            );
                        });
                    });
                }

                ui.add_space(8.0);

                // Output section
                ui.heading(RichText::new("📝 Output").size(14.0).strong());
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Total lines:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", stats.total_output_lines))
                                .color(Color32::LIGHT_BLUE),
                        );
                    });
                });

                if stats.total_commands > 0 {
                    let avg_lines = stats.total_output_lines as f32 / stats.total_commands as f32;
                    ui.horizontal(|ui| {
                        ui.label("Avg lines per command:");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:.1}", avg_lines)).color(Color32::WHITE),
                            );
                        });
                    });
                }
            });
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

/// Get color based on memory usage
fn color_for_memory(bytes: usize) -> Color32 {
    const MB_100: usize = 100 * 1024 * 1024;
    const MB_500: usize = 500 * 1024 * 1024;

    if bytes < MB_100 {
        Color32::GREEN
    } else if bytes < MB_500 {
        Color32::YELLOW
    } else {
        Color32::RED
    }
}

/// Get color based on success rate
fn color_for_success_rate(rate: f32) -> Color32 {
    if rate >= 90.0 {
        Color32::GREEN
    } else if rate >= 70.0 {
        Color32::YELLOW
    } else {
        Color32::RED
    }
}
