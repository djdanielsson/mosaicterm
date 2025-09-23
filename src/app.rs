//! Main application structure and state management

use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use futures::executor;
use crate::pty::PtyManager;
use crate::terminal::{Terminal, TerminalFactory, ShellType as SessionShellType};
use crate::models::{TerminalSession, ShellType as ModelShellType};
use crate::ui::{CommandBlocks, InputPrompt, ScrollableHistory};
use crate::config::theme::ThemeManager;
use crate::models::{CommandBlock, ExecutionStatus};
use crate::config::RuntimeConfig;
use crate::error::Result;

/// Main MosaicTerm application
pub struct MosaicTermApp {
    /// Terminal emulator instance
    terminal: Option<Terminal>,
    /// PTY manager for process management
    pty_manager: Arc<Mutex<PtyManager>>,
    /// Terminal factory for creating terminals
    terminal_factory: TerminalFactory,
    /// UI components
    command_blocks: CommandBlocks,
    input_prompt: InputPrompt,
    scrollable_history: ScrollableHistory,
    /// Command history
    command_history: Vec<CommandBlock>,
    /// Application state
    state: AppState,
    /// Runtime configuration
    runtime_config: RuntimeConfig,
    /// Theme manager for UI styling
    theme_manager: ThemeManager,
    /// Pending context menu info (block_id, position)
    pending_context_menu: Option<(String, egui::Pos2)>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    /// Whether the terminal is ready for input
    terminal_ready: bool,
    /// Current theme
    theme: AppTheme,
    /// Window title
    title: String,
    /// Status message
    status_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Default shell type
    default_shell: SessionShellType,
    /// Initial terminal dimensions
    initial_dimensions: (usize, usize),
    /// Maximum scrollback lines
    max_scrollback: usize,
    /// Font size
    font_size: f32,
}

#[derive(Debug, Clone)]
pub enum AppTheme {
    Dark,
    Light,
    Auto,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            terminal_ready: false,
            theme: AppTheme::Auto,
            title: "MosaicTerm".to_string(),
            status_message: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_shell: SessionShellType::Bash,
            initial_dimensions: (120, 30),
            max_scrollback: 1000,
            font_size: 12.0,
        }
    }
}

impl Default for MosaicTermApp {
    fn default() -> Self {
        Self::new()
    }
}

impl MosaicTermApp {
    /// Create a new MosaicTerm application instance
    pub fn new() -> Self {
        info!("Initializing MosaicTerm application");

        // Create PTY manager
        let pty_manager = Arc::new(Mutex::new(PtyManager::new()));

        // Create terminal factory
        let terminal_factory = TerminalFactory::new(pty_manager.clone());

        // Create UI components
        let command_blocks = CommandBlocks::new();
        let input_prompt = InputPrompt::new();
        let scrollable_history = ScrollableHistory::new();

        Self {
            terminal: None,
            pty_manager,
            terminal_factory,
            command_blocks,
            input_prompt,
            scrollable_history,
            command_history: Vec::new(),
            state: AppState::default(),
            runtime_config: RuntimeConfig::new().unwrap_or_else(|e| {
                warn!("Failed to create runtime config: {}, using minimal config", e);
                // For now, panic on config creation failure - this should be handled better
                panic!("Failed to create runtime config: {}", e);
            }),
            theme_manager: ThemeManager::new(),
            pending_context_menu: None,
        }
    }

    /// Create application with runtime configuration
    pub fn with_config(runtime_config: RuntimeConfig) -> Self {
        let mut app = Self::new();
        app.runtime_config = runtime_config;
        app
    }

    /// Initialize the terminal session
    pub async fn initialize_terminal(&mut self) -> Result<()> {
        info!("Initializing terminal session");

        // Convert config shell type to model shell type
        let shell_type = match self.runtime_config.config().terminal.shell_type {
            crate::config::shell::ShellType::Bash => ModelShellType::Bash,
            crate::config::shell::ShellType::Zsh => ModelShellType::Zsh,
            crate::config::shell::ShellType::Fish => ModelShellType::Fish,
            // Map other shell types to supported ones or use Other variant
            crate::config::shell::ShellType::Ksh |
            crate::config::shell::ShellType::Csh |
            crate::config::shell::ShellType::Tcsh |
            crate::config::shell::ShellType::Dash |
            crate::config::shell::ShellType::PowerShell |
            crate::config::shell::ShellType::Cmd => ModelShellType::Bash, // Default to bash
            crate::config::shell::ShellType::Other => ModelShellType::Bash, // Default to bash for unknown shells
        };

        // Create terminal session configuration
        let session = TerminalSession::new(
            shell_type,
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
        );

        // Create and initialize terminal
        let terminal = self.terminal_factory.create_and_initialize(session).await?;
        self.terminal = Some(terminal);
        self.state.terminal_ready = true;

        info!("Terminal session initialized successfully");
        Ok(())
    }

    /// Handle command input from the UI
    pub async fn handle_command_input(&mut self, command: String) -> Result<()> {
        if command.trim().is_empty() {
            return Ok(());
        }

        info!("Processing command: {}", command);

        if let Some(terminal) = &mut self.terminal {
            // Send command to terminal
            let _ = terminal.process_input(&command).await;

            // Create command block and add to history
            let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            let mut command_block = CommandBlock::new(command.clone(), working_dir);
            command_block.mark_running();
            self.command_history.push(command_block);

            // Update UI components
            self.update_ui_components();

            self.state.status_message = Some(format!("Executed: {}", command));
        } else {
            warn!("Terminal not initialized, cannot execute command");
            self.state.status_message = Some("Terminal not ready".to_string());
        }

        Ok(())
    }

    /// Update application state
    pub fn update_state(&mut self) {
        self.state.terminal_ready = self.terminal.is_some();

        // Update UI components if needed
        self.update_ui_components();
    }

    /// Update UI components with latest data
    fn update_ui_components(&mut self) {
        // Update command blocks with current history
        // This would be called when command history changes
        debug!("UI components updated");
    }

    /// Get current application state
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get current runtime configuration
    pub fn runtime_config(&self) -> &RuntimeConfig {
        &self.runtime_config
    }

    /// Get terminal instance (for testing)
    pub fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Get terminal instance mutably (for testing)
    pub fn terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.terminal.as_mut()
    }

    /// Set status message
    pub fn set_status_message(&mut self, message: Option<String>) {
        self.state.status_message = message;
    }
}

impl eframe::App for MosaicTermApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Update application state
        self.update_state();

        // Update window title with application state
        self.update_window_title(frame);

        // Set up visual style
        self.setup_visual_style(ctx);

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx, frame);

        // Main layout with scrollable history and pinned input
        egui::CentralPanel::default().show(ctx, |ui| {
            // Set up the main vertical layout with proper spacing
            ui.vertical(|ui| {
                // Status bar at the top
                self.render_status_bar(ui);

                // Separator
                ui.separator();

                // Scrollable history area (takes most space)
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    // Use the enhanced scrollable history component with command blocks
                    let context_menu_info = self.scrollable_history.render_command_blocks(ui, &self.command_history, &mut self.command_blocks);

                    // Store context menu info for rendering at top level
                    self.pending_context_menu = context_menu_info;
                });

                // Separator before input area
                ui.separator();

                // Pinned input prompt at the bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    self.render_input_area(ui);
                });
            });
        });

        // Render context menu at top level (outside scroll areas)
        if let Some((block_id, position)) = &self.pending_context_menu {
            if let Some(block) = self.command_history.iter().find(|b| b.id == *block_id) {
                if let Some(action) = self.command_blocks.render_context_menu_at(ctx, block, *position) {
                    self.handle_context_menu_action(action);
                }
            }
            // Clear the pending menu after rendering
            self.pending_context_menu = None;
        }

        // Handle async operations
        self.handle_async_operations(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("MosaicTerm application shutting down");

        // Cleanup terminal session
        if self.terminal.is_some() {
            // Terminal cleanup will happen automatically when dropped
        }
    }
}

impl MosaicTermApp {
    /// Update window title with application state
    fn update_window_title(&self, _frame: &mut eframe::Frame) {
        // Window title management is handled by the framework
        // This method is a placeholder for future window title customization
        let _title = if self.state.terminal_ready {
            format!("{} - Ready", self.state.title)
        } else {
            format!("{} - Initializing...", self.state.title)
        };
        // TODO: Implement window title updates when eframe supports it
    }

    /// Set up visual style for the application using the theme system
    fn setup_visual_style(&self, ctx: &egui::Context) {
        // Apply the current theme to the egui context
        if let Err(e) = self.theme_manager.apply_to_egui(ctx) {
            warn!("Failed to apply theme to egui: {}, using default styling", e);
            // Fallback to basic styling if theme application fails
            let mut style = (*ctx.style()).clone();
            style.visuals.dark_mode = true;
            ctx.set_style(style);
        }
    }

    /// Handle keyboard shortcuts and navigation
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Only handle shortcuts when no text input is focused
        if ctx.memory(|mem| mem.focus().is_none()) {
            // Application shortcuts
            if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.ctrl) {
                std::process::exit(0); // Ctrl+Q to quit
            }

            // Terminal shortcuts
            if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl) {
                // Ctrl+C to interrupt current command
                self.handle_interrupt_command();
            }

            if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
                // Ctrl+L to clear screen
                self.handle_clear_screen();
            }

            if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
                // Ctrl+D to exit (EOF)
                self.handle_exit();
            }
        }

        // Enhanced navigation shortcuts (work even when input is focused)
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

        // Arrow key navigation for scrolling
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.ctrl) {
            // Ctrl+Up to scroll up by line
            self.handle_scroll_up_line();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.ctrl) {
            // Ctrl+Down to scroll down by line
            self.handle_scroll_down_line();
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

        // Theme switching shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.shift) {
            // Ctrl+Shift+T to cycle themes
            self.handle_cycle_theme();
        }

        // Command history shortcuts (when not in input)
        if ctx.memory(|mem| mem.focus().is_none()) {
            if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
                // Ctrl+R to search command history (reverse search)
                self.handle_command_search();
            }

            if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.ctrl) {
                // Ctrl+G to clear current search
                self.handle_clear_search();
            }
        }
    }

    /// Handle command interruption (Ctrl+C)
    fn handle_interrupt_command(&mut self) {
        if let Some(terminal) = &mut self.terminal {
            // Send interrupt signal to current process
            if let Err(e) = executor::block_on(async {
                let pty_manager = self.pty_manager.lock().await;
                // In a real implementation, this would send SIGINT
                // For now, just log and update status
                Ok::<(), crate::error::Error>(())
            }) {
                error!("Failed to interrupt command: {}", e);
            }
        }
        info!("Interrupt command requested (Ctrl+C)");
        self.set_status_message(Some("Command interrupted".to_string()));
    }

    /// Handle screen clearing (Ctrl+L)
    fn handle_clear_screen(&mut self) {
        // Clear command history
        self.command_history.clear();
        // Clear terminal screen (would send clear command to shell)
        if let Some(terminal) = &mut self.terminal {
            let _ = terminal.process_input("clear");
        }
        info!("Clear screen requested (Ctrl+L)");
        self.set_status_message(Some("Screen cleared".to_string()));
    }

    /// Handle exit (Ctrl+D)
    fn handle_exit(&mut self) {
        // Send EOF to shell (Ctrl+D)
        if let Some(terminal) = &mut self.terminal {
            let _ = terminal.process_input("\x04"); // EOF character
        }
        info!("Exit requested (Ctrl+D)");
        self.set_status_message(Some("EOF sent".to_string()));
    }

    /// Handle scroll up (Page Up)
    fn handle_scroll_up(&mut self) {
        // Scroll history up by one page
        self.scrollable_history.scroll_by(-20.0); // Scroll up by 20 units
        info!("Scroll up requested (Page Up)");
    }

    /// Handle scroll down (Page Down)
    fn handle_scroll_down(&mut self) {
        // Scroll history down by one page
        self.scrollable_history.scroll_by(20.0); // Scroll down by 20 units
        info!("Scroll down requested (Page Down)");
    }

    /// Handle scroll to top (Ctrl+Home)
    fn handle_scroll_to_top(&mut self) {
        // Scroll to top of history
        self.scrollable_history.scroll_to_top();
        info!("Scroll to top requested (Ctrl+Home)");
    }

    /// Handle scroll to bottom (Ctrl+End)
    fn handle_scroll_to_bottom(&mut self) {
        // Scroll to bottom of history
        self.scrollable_history.scroll_to_bottom();
        info!("Scroll to bottom requested (Ctrl+End)");
    }

    /// Handle focus next (Ctrl+Tab)
    fn handle_focus_next(&mut self) {
        // Cycle focus to next UI element
        // This would cycle between input field, command history, etc.
        info!("Focus next requested (Ctrl+Tab)");
        self.set_status_message(Some("Focus cycled to next element".to_string()));
    }

    /// Handle focus previous (Ctrl+Shift+Tab)
    fn handle_focus_previous(&mut self) {
        // Cycle focus to previous UI element
        info!("Focus previous requested (Ctrl+Shift+Tab)");
        self.set_status_message(Some("Focus cycled to previous element".to_string()));
    }

    /// Handle scroll up by line (Ctrl+Up)
    fn handle_scroll_up_line(&mut self) {
        // Scroll history up by one line
        self.scrollable_history.scroll_by(-16.0); // Scroll up by 16 units (approx one line)
        info!("Scroll up by line requested (Ctrl+Up)");
    }

    /// Handle scroll down by line (Ctrl+Down)
    fn handle_scroll_down_line(&mut self) {
        // Scroll history down by one line
        self.scrollable_history.scroll_by(16.0); // Scroll down by 16 units (approx one line)
        info!("Scroll down by line requested (Ctrl+Down)");
    }

    /// Handle theme cycling (Ctrl+Shift+T)
    fn handle_cycle_theme(&mut self) {
        let themes = self.theme_manager.list_themes();
        let current = self.theme_manager.current_theme.clone();

        // Find current theme index and cycle to next
        if let Some(current_idx) = themes.iter().position(|t| *t == current) {
            let next_idx = (current_idx + 1) % themes.len();
            let next_theme = themes[next_idx];

            if self.theme_manager.set_theme(next_theme).is_ok() {
                info!("Switched to theme: {}", next_theme);
                self.set_status_message(Some(format!("Theme: {}", next_theme)));
            } else {
                warn!("Failed to switch to theme: {}", next_theme);
            }
        }
    }

    /// Handle command history search (Ctrl+R)
    fn handle_command_search(&mut self) {
        // Start reverse search through command history
        // This would typically open a search interface
        info!("Command history search requested (Ctrl+R)");
        self.set_status_message(Some("Command search: Type to search history...".to_string()));
    }

    /// Handle clear search (Ctrl+G)
    fn handle_clear_search(&mut self) {
        // Clear any active search
        info!("Clear search requested (Ctrl+G)");
        self.set_status_message(Some("Search cleared".to_string()));
    }

    /// Handle context menu actions from command blocks
    fn handle_context_menu_action(&mut self, action: crate::ui::ContextMenuAction) {
        match action {
            crate::ui::ContextMenuAction::CopyCommand(command) => {
                self.copy_to_clipboard(&command);
                self.set_status_message(Some("Command copied to clipboard".to_string()));
            }
            crate::ui::ContextMenuAction::CopyOutput(output) => {
                self.copy_to_clipboard(&output);
                self.set_status_message(Some("Output copied to clipboard".to_string()));
            }
            crate::ui::ContextMenuAction::CopyCommandAndOutput(text) => {
                self.copy_to_clipboard(&text);
                self.set_status_message(Some("Command and output copied to clipboard".to_string()));
            }
            crate::ui::ContextMenuAction::RerunCommand(command) => {
                info!("Rerunning command from context menu: {}", command);
                let _ = executor::block_on(self.handle_command_input(command));
            }
        }
    }

    /// Copy text to clipboard
    fn copy_to_clipboard(&self, text: &str) {
        // Use eframe's clipboard functionality
        if let Some(clipboard) = eframe::get_clipboard() {
            let _ = clipboard.set_text(text);
        } else {
            // Fallback: try to use system clipboard via command
            let _ = std::process::Command::new("pbcopy")
                .arg(text)
                .status();
        }
    }

    /// Render the status bar
    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🖥️ MosaicTerm");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Terminal status
                if self.state.terminal_ready {
                    ui.colored_label(egui::Color32::GREEN, "● Ready");
                } else {
                    ui.colored_label(egui::Color32::YELLOW, "● Initializing...");
                }

                // Status message
                if let Some(message) = &self.state.status_message {
                    ui.separator();
                    ui.label(message);
                }
            });
        });
        ui.separator();
    }


    /// Render the input area
    fn render_input_area(&mut self, ui: &mut egui::Ui) {
        // Render input prompt and handle input
        if let Some(command) = self.input_prompt.render(ui) {
            // For now, handle synchronously. In a real implementation,
            // this would be handled asynchronously with proper error handling
            if let Err(e) = executor::block_on(self.handle_command_input(command)) {
                error!("Failed to handle command input: {}", e);
                self.state.status_message = Some(format!("Error: {}", e));
            }
        }
    }

    /// Handle async operations (called from update)
    fn handle_async_operations(&mut self, _ctx: &egui::Context) {
        // Handle async operations like:
        // - Processing pending terminal output
        // - Updating UI with new command results
        // - Handling PTY events

        // TODO: Implement proper terminal output reading
        // Check for terminal output and update command blocks
        // This requires implementing read_output method on Terminal
        /*
        if let Some(terminal) = &mut self.terminal {
            // Process any pending output
            if let Ok(output) = terminal.read_output() {
                if !output.is_empty() {
                    // Update the last command block with output
                    if let Some(last_block) = self.command_history.last_mut() {
                        for line in output {
                            last_block.add_output_line(line);
                        }
                    }
                }
            }
        }
        */

        // Update UI components if needed
        self.update_ui_components();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = MosaicTermApp::new();
        assert!(app.terminal().is_some());
        assert!(!app.state().terminal_ready);
    }

    #[test]
    fn test_app_config() {
        let config = AppConfig::default();
        assert_eq!(config.initial_dimensions, (120, 30));
        assert_eq!(config.font_size, 12.0);
    }

    #[test]
    fn test_app_state() {
        let state = AppState::default();
        assert_eq!(state.title, "MosaicTerm");
        assert!(!state.terminal_ready);
    }

    #[test]
    fn test_app_with_config() {
        let config = AppConfig {
            default_shell: "zsh".to_string(),
            initial_dimensions: (80, 25),
            max_scrollback: 500,
            font_size: 14.0,
        };

        let app = MosaicTermApp::with_config(config);
        assert_eq!(app.config().initial_dimensions, (80, 25));
        assert_eq!(app.config().font_size, 14.0);
    }

    #[test]
    fn test_status_message() {
        let mut app = MosaicTermApp::new();
        app.set_status_message(Some("Test message".to_string()));
        assert_eq!(app.state().status_message, Some("Test message".to_string()));

        app.set_status_message(None);
        assert!(app.state().status_message.is_none());
    }

    #[test]
    fn test_theme_enum() {
        assert_eq!(format!("{:?}", AppTheme::Dark), "Dark");
        assert_eq!(format!("{:?}", AppTheme::Light), "Light");
        assert_eq!(format!("{:?}", AppTheme::Auto), "Auto");
    }
}
