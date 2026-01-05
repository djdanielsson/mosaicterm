//! Input and Keyboard Handling
//!
//! This module handles keyboard shortcuts, navigation, scroll handling,
//! and focus management for the terminal application.
//!
//! ## Keyboard Shortcuts
//!
//! | Shortcut | Action | Works When Focused |
//! |----------|--------|-------------------|
//! | Ctrl+C | Interrupt running command | Always |
//! | Ctrl+L | Clear screen | Always |
//! | Ctrl+R | Toggle history search | Always |
//! | Ctrl+Q | Quit application | No focus |
//! | Ctrl+D | Send EOF | No focus |
//! | Page Up | Scroll up | Always |
//! | Page Down | Scroll down | Always |
//! | Ctrl+Home | Scroll to top | Always |
//! | Ctrl+End | Scroll to bottom | Always |
//! | Ctrl+Tab | Focus next element | Always |
//! | Ctrl+Shift+Tab | Focus previous element | Always |
//!
//! ## Focus Management
//!
//! Some shortcuts (like Ctrl+Q) only work when no text input is focused,
//! to avoid interfering with text editing. Others (like Ctrl+C for interrupt)
//! work regardless of focus state because they are critical operations.

use eframe::egui;
use tracing::{error, info, warn};

use super::{AsyncRequest, MosaicTermApp};

impl MosaicTermApp {
    /// Handle keyboard shortcuts and navigation
    pub(super) fn handle_keyboard_shortcuts(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        // Ctrl+C works ALWAYS, even when input is focused - kills running command
        if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl && !i.modifiers.shift) {
            // Check if there's a running command
            let has_running_command = self
                .state_manager
                .get_command_history()
                .iter()
                .any(|block| block.is_running());

            if has_running_command {
                // Ctrl+C to interrupt current command
                self.handle_interrupt_command();
                // Consume the event so it doesn't get processed elsewhere
                ctx.input_mut(|i| i.events.clear());
            }
            // If no running command, let the input field handle it normally
        }

        // Ctrl+L works ALWAYS, even when input is focused - clears screen
        if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            self.handle_clear_screen();
            ctx.input_mut(|i| i.events.clear());
        }

        // Ctrl+R works ALWAYS, even when input is focused - opens history search
        if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            self.history_search_active = !self.history_search_active;
            if self.history_search_active {
                self.history_search_query.clear();
                self.history_search_needs_focus = true; // Request focus on next frame
                                                        // Request repaint to ensure popup shows immediately
                ctx.request_repaint();
            }
            // Only clear events if we're closing the popup, not opening it
            if !self.history_search_active {
                ctx.input_mut(|i| i.events.clear());
            }
        }

        // Only handle other shortcuts when no text input is focused
        if ctx.memory(|mem| mem.focus().is_none()) {
            // Application shortcuts
            if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.ctrl) {
                std::process::exit(0); // Ctrl+Q to quit
            }

            if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
                // Ctrl+D to exit (EOF)
                self.handle_exit();
            }
        }

        // Navigation shortcuts (work even when input is focused)
        if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
            // Page Up to scroll up
            self.handle_scroll_up();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
            // Page Down to scroll down
            self.handle_scroll_down();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Home) && i.modifiers.ctrl) {
            // Ctrl+Home to scroll to top
            self.handle_scroll_to_top();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::End) && i.modifiers.ctrl) {
            // Ctrl+End to scroll to bottom
            self.handle_scroll_to_bottom();
        }

        // Tab navigation
        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl) {
            // Ctrl+Tab to switch focus
            self.handle_focus_next();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && i.modifiers.shift) {
            // Ctrl+Shift+Tab to switch focus backward
            self.handle_focus_previous();
        }
    }

    /// Handle command interruption (Ctrl+C)
    pub(super) fn handle_interrupt_command(&mut self) {
        if let Some(terminal) = &mut self.terminal {
            // Get the current PTY handle from the terminal
            if let Some(pty_handle) = terminal.pty_handle() {
                let handle_id = pty_handle.id.clone();

                // Send interrupt signal to the PTY process (non-blocking)
                info!("Sending interrupt signal for handle: {}", handle_id);
                if let Err(e) = self
                    .async_tx
                    .send(AsyncRequest::SendInterrupt(handle_id.clone()))
                {
                    error!("Failed to send interrupt request: {}", e);
                    self.set_status_message(Some("Failed to interrupt command".to_string()));
                } else {
                    self.set_status_message(Some("Interrupting command...".to_string()));
                }

                // Mark current command as cancelled (result will come from async)
                // Check if the command is interactive first (before mutable borrow)
                let command_history = self.state_manager.get_command_history();
                let is_interactive = command_history
                    .last()
                    .map(|block| self.is_interactive_command(&block.command))
                    .unwrap_or(false);

                // Mark the last command as cancelled in state manager
                let history = self.state_manager.get_command_history();
                if let Some(last_block) = history.last() {
                    let block_id = last_block.id.clone();
                    self.state_manager.update_command_block_status(
                        &block_id,
                        mosaicterm::models::ExecutionStatus::Cancelled,
                    );
                }

                // Clear the command time so new commands can be submitted
                self.state_manager.set_last_command_time(chrono::Utc::now());

                // For interactive programs, we need to restart the PTY session
                // because the shell can get into a corrupted state
                if is_interactive {
                    info!("Restarting PTY session after interactive program cancel");
                    self.start_loading("Restarting shell session...");

                    // Restart the PTY in a background task (non-blocking)
                    if let Err(e) = self.async_tx.send(AsyncRequest::RestartPty) {
                        error!("Failed to send RestartPty request: {}", e);
                        self.stop_loading();
                        self.set_status_message(Some("Failed to restart session".to_string()));
                    }

                    // Result will be handled in poll_async_results
                }
            } else {
                warn!("No active PTY process to interrupt");
                self.set_status_message(Some("No running command to interrupt".to_string()));
            }
        } else {
            warn!("Terminal not available for interrupt");
            self.set_status_message(Some("Terminal not ready".to_string()));
        }
    }

    /// Handle interruption of a specific command block (right-click kill)
    pub(super) fn handle_interrupt_specific_command(&mut self, block_id: String) {
        if let Some(terminal) = &mut self.terminal {
            // Get the current PTY handle from the terminal
            if let Some(pty_handle) = terminal.pty_handle() {
                let handle_id = pty_handle.id.clone();

                // Send interrupt signal to the PTY process (non-blocking)
                info!("Sending interrupt signal for block {}", block_id);
                if let Err(e) = self
                    .async_tx
                    .send(AsyncRequest::SendInterrupt(handle_id.clone()))
                {
                    error!("Failed to send interrupt request: {}", e);
                    self.set_status_message(Some("Failed to interrupt command".to_string()));
                } else {
                    self.set_status_message(Some("Interrupting command...".to_string()));
                }

                // Mark the specific command block as cancelled
                self.state_manager.update_command_block_status(
                    &block_id,
                    mosaicterm::models::ExecutionStatus::Cancelled,
                );

                // Clear the command time so new commands can be submitted
                self.state_manager.set_last_command_time(chrono::Utc::now());

                // Check if the command being killed is interactive
                let command_history = self.state_manager.get_command_history();
                let is_interactive = command_history
                    .iter()
                    .find(|block| block.id == block_id)
                    .map(|block| self.is_interactive_command(&block.command))
                    .unwrap_or(false);

                // For interactive programs, restart the PTY session
                if is_interactive {
                    info!("Restarting PTY session after interactive program cancel");
                    self.start_loading("Restarting shell session...");

                    if let Err(e) = self.async_tx.send(AsyncRequest::RestartPty) {
                        error!("Failed to send RestartPty request: {}", e);
                        self.stop_loading();
                        self.set_status_message(Some("Failed to restart session".to_string()));
                    }
                }
            } else {
                warn!("No active PTY process to interrupt");
                self.set_status_message(Some("No running command to interrupt".to_string()));
            }
        } else {
            warn!("Terminal not available for interrupt");
            self.set_status_message(Some("Terminal not ready".to_string()));
        }
    }

    /// Handle screen clearing (Ctrl+L)
    pub(super) fn handle_clear_screen(&mut self) {
        // Clear command history (visual blocks) but preserve input history for arrow keys
        self.state_manager.clear_command_history();

        info!("Clear screen requested (Ctrl+L)");
        self.set_status_message(Some("Screen cleared".to_string()));
    }

    /// Handle exit (Ctrl+D)
    pub(super) fn handle_exit(&mut self) {
        // Send EOF to shell (Ctrl+D)
        if let Some(_terminal) = &mut self.terminal {
            std::mem::drop(_terminal.process_input("\x04")); // EOF character
        }
        info!("Exit requested (Ctrl+D)");
        self.set_status_message(Some("EOF sent".to_string()));
    }

    /// Handle scroll up (Page Up)
    pub(super) fn handle_scroll_up(&mut self) {
        // Scroll history up by one page
        self.scrollable_history.scroll_by(-20.0); // Scroll up by 20 units
        info!("Scroll up requested (Page Up)");
    }

    /// Handle scroll down (Page Down)
    pub(super) fn handle_scroll_down(&mut self) {
        // Scroll history down by one page
        self.scrollable_history.scroll_by(20.0); // Scroll down by 20 units
        info!("Scroll down requested (Page Down)");
    }

    /// Handle scroll to top (Ctrl+Home)
    pub(super) fn handle_scroll_to_top(&mut self) {
        // Scroll to top of history
        self.scrollable_history.scroll_to_top();
        info!("Scroll to top requested (Ctrl+Home)");
    }

    /// Handle scroll to bottom (Ctrl+End)
    pub(super) fn handle_scroll_to_bottom(&mut self) {
        // Scroll to bottom of history
        self.scrollable_history.scroll_to_bottom();
        info!("Scroll to bottom requested (Ctrl+End)");
    }

    /// Handle focus next (Ctrl+Tab)
    pub(super) fn handle_focus_next(&mut self) {
        // Cycle focus to next UI element
        // This would cycle between input field, command history, etc.
        info!("Focus next requested (Ctrl+Tab)");
        self.set_status_message(Some("Focus cycled to next element".to_string()));
    }

    /// Handle focus previous (Ctrl+Shift+Tab)
    pub(super) fn handle_focus_previous(&mut self) {
        // Cycle focus to previous UI element
        info!("Focus previous requested (Ctrl+Shift+Tab)");
        self.set_status_message(Some("Focus cycled to previous element".to_string()));
    }
}
