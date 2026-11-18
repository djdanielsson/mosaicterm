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
use tracing::info;

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
            let has_running_command = self.pane.pending_block.is_some();

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
        if ctx.memory(|mem| mem.focused().is_none()) {
            // Application shortcuts
            if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.ctrl) {
                self.handle_exit();
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

        if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl && i.modifiers.shift) {
            self.handle_focus_previous();
        } else if ctx.input(|i| i.key_pressed(egui::Key::Tab) && i.modifiers.ctrl) {
            self.handle_focus_next();
        }

        // Pane management shortcuts
        if ctx.input(|i| {
            i.key_pressed(egui::Key::D) && i.modifiers.ctrl && i.modifiers.shift && !i.modifiers.alt
        }) {
            self.handle_split_right();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.ctrl && i.modifiers.shift) {
            self.handle_split_down();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::W) && i.modifiers.ctrl && i.modifiers.shift) {
            self.handle_close_pane();
        }

        if ctx.input(|i| {
            i.key_pressed(egui::Key::ArrowRight) && i.modifiers.ctrl && i.modifiers.shift
        }) {
            self.handle_navigate_pane(super::pane_tree::Direction::Right);
        }
        if ctx
            .input(|i| i.key_pressed(egui::Key::ArrowLeft) && i.modifiers.ctrl && i.modifiers.shift)
        {
            self.handle_navigate_pane(super::pane_tree::Direction::Left);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl && i.modifiers.shift)
        {
            self.handle_navigate_pane(super::pane_tree::Direction::Up);
        }
        if ctx
            .input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl && i.modifiers.shift)
        {
            self.handle_navigate_pane(super::pane_tree::Direction::Down);
        }
    }

    pub(super) fn handle_interrupt_command(&mut self) {
        let Some(block_id) = self.pane.pending_block.clone() else {
            return;
        };
        self.handle_interrupt_specific_command(block_id);
    }

    pub(super) fn handle_interrupt_specific_command(&mut self, block_id: String) {
        if self.pane.pending_block.as_deref() != Some(&block_id) {
            return;
        }
        let Some(handle) = self
            .pane
            .terminal
            .as_ref()
            .and_then(|terminal| terminal.pty_handle())
            .cloned()
        else {
            return;
        };
        if self.interrupt_pty(handle) {
            self.pane.interrupted = true;
            self.set_status_message(Some("Interrupting command…".to_string()));
        }
    }

    pub(super) fn interrupt_pty(&self, handle: mosaicterm::pty::PtyHandle) -> bool {
        let pane_id = self.pane_tree.active_id().to_string();
        self.async_tx
            .send(AsyncRequest::SendInterrupt { pane_id, handle })
            .is_ok()
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
        if let Some(_terminal) = &mut self.pane.terminal {
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
        info!("Focus previous requested (Ctrl+Shift+Tab)");
        self.set_status_message(Some("Focus cycled to previous element".to_string()));
    }

    /// Split active pane to the right
    pub(super) fn handle_split_right(&mut self) {
        self.split_active_pane(super::pane_tree::SplitAxis::Horizontal);
    }

    /// Split active pane downward
    pub(super) fn handle_split_down(&mut self) {
        self.split_active_pane(super::pane_tree::SplitAxis::Vertical);
    }

    fn split_active_pane(&mut self, axis: super::pane_tree::SplitAxis) {
        let previous_id = self.pane_tree.active_id().to_string();
        let config = &self.runtime_config.config().terminal;
        let (_, shell_type) = config.shell();
        let working_dir = self
            .pane
            .terminal
            .as_ref()
            .map(|terminal| terminal.get_working_directory())
            .filter(|dir| dir.is_dir())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| Self::initial_working_dir(config));
        let session_id = self
            .state_manager
            .create_session(working_dir.clone(), shell_type);
        if self
            .pane_tree
            .split(&previous_id, axis, session_id.clone())
            .is_some()
        {
            self.switch_pane_session(&previous_id);
            self.request_shell(working_dir);
        } else {
            self.state_manager.remove_session(&session_id);
            self.switch_pane_session(&previous_id);
        }
    }

    pub(super) fn handle_close_pane(&mut self) {
        let id = self.pane_tree.active_id().to_string();
        if self.pane_tree.pane_count() <= 1 {
            return;
        }
        let session_id = self
            .pane_tree
            .active_pane()
            .expect("active pane")
            .session_id
            .clone();
        if self.pane_tree.close(&id) {
            if let Some(terminal) = self.pane.terminal.take() {
                self.close_terminal(terminal);
            }
            self.state_manager.remove_session(&session_id);
            self.pane = super::PaneSession::default();
            self.switch_pane_session(&id);
        }
    }

    pub(super) fn handle_navigate_pane(&mut self, direction: super::pane_tree::Direction) {
        let previous = self.pane_tree.active_id().to_string();
        self.pane_tree.navigate(direction);
        if previous != self.pane_tree.active_id() {
            self.switch_pane_session(&previous);
        }
    }
}
