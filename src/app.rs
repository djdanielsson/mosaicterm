//! Main application structure and state management

use eframe::egui;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use futures::executor;
use mosaicterm::pty::PtyManager;
use mosaicterm::terminal::{Terminal, TerminalFactory};
use mosaicterm::execution::DirectExecutor;
use mosaicterm::models::{TerminalSession, ShellType as ModelShellType};
use mosaicterm::ui::{InputPrompt, ScrollableHistory};
use mosaicterm::models::{CommandBlock, ExecutionStatus};
use mosaicterm::config::RuntimeConfig;
use mosaicterm::error::Result;

/// Main MosaicTerm application
pub struct MosaicTermApp {
    /// Terminal emulator instance
    terminal: Option<Terminal>,
    /// PTY manager for process management
    pty_manager: Arc<Mutex<PtyManager>>,
    /// Terminal factory for creating terminals
    terminal_factory: TerminalFactory,
    /// UI components
    input_prompt: InputPrompt,
    scrollable_history: ScrollableHistory,
    /// Command history
    command_history: Vec<CommandBlock>,
    /// Application state
    state: AppState,
    /// Runtime configuration
    runtime_config: RuntimeConfig,
    /// Accumulates partial output (no newline yet) for the latest command
    /// Last time a command was executed (for timeout detection)
    last_command_time: Option<std::time::Instant>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    /// Whether the terminal is ready for input
    terminal_ready: bool,
    /// Whether terminal initialization has been attempted
    initialization_attempted: bool,
    /// Window title
    title: String,
    /// Status message
    status_message: Option<String>,
}


#[derive(Debug, Clone)]
pub enum AppTheme {
    Auto,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            terminal_ready: false,
            initialization_attempted: false,
            title: "MosaicTerm".to_string(),
            status_message: None,
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
        let input_prompt = InputPrompt::new();
        let scrollable_history = ScrollableHistory::new();

        let runtime_config = RuntimeConfig::new().unwrap_or_else(|e| {
            warn!("Failed to create runtime config: {}, using minimal config", e);
            // For now, panic on config creation failure - this should be handled better
            panic!("Failed to create runtime config: {}", e);
        });

        let mut command_history = Vec::new();

        // Add some demo commands to show the UI functionality
        let demo_commands = vec![
            ("pwd", "Current working directory"),
            ("ls -la", "List all files with details"),
            ("echo 'Hello from MosaicTerm!'", "Print a greeting message"),
        ];

        for (cmd, _description) in demo_commands {
            let mut block = mosaicterm::models::CommandBlock::new(
                cmd.to_string(),
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            );

            // Simulate some output for demo
            if cmd == "pwd" {
                block.add_output_line(mosaicterm::models::OutputLine {
                    text: std::env::current_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                        .to_string_lossy()
                        .to_string(),
                    ansi_codes: vec![],
                    line_number: 0,
                    timestamp: chrono::Utc::now(),
                });
                block.mark_completed(std::time::Duration::from_millis(50));
            } else if cmd == "echo 'Hello from MosaicTerm!'" {
                block.add_output_line(mosaicterm::models::OutputLine {
                    text: "Hello from MosaicTerm!".to_string(),
                    ansi_codes: vec![],
                    line_number: 0,
                    timestamp: chrono::Utc::now(),
                });
                block.mark_completed(std::time::Duration::from_millis(25));
            } else {
                block.mark_running();
            }

            command_history.push(block);
        }

        Self {
            terminal: None,
            pty_manager,
            terminal_factory,
            input_prompt,
            scrollable_history,
            command_history,
            state: AppState::default(),
            runtime_config,
            last_command_time: None,
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
            ModelShellType::Bash => ModelShellType::Bash,
            ModelShellType::Zsh => ModelShellType::Zsh,
            ModelShellType::Fish => ModelShellType::Fish,
            // Map other shell types to supported ones or use Other variant
            ModelShellType::Ksh |
            ModelShellType::Csh |
            ModelShellType::Tcsh |
            ModelShellType::Dash |
            ModelShellType::PowerShell |
            ModelShellType::Cmd => ModelShellType::Bash, // Default to bash
            ModelShellType::Other => ModelShellType::Bash, // Default to bash for unknown shells
        };

        // Create terminal session configuration
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        
        // Create environment with prompt suppression
        let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();
        
        // Suppress shell prompts by setting PS1 to empty for bash/zsh
        // This prevents prompts from appearing in output at all
        match shell_type {
            ModelShellType::Bash | ModelShellType::Zsh => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string()); // Also suppress continuation prompt
                environment.insert("PS3".to_string(), "".to_string()); // Suppress select prompt
                environment.insert("PS4".to_string(), "".to_string()); // Suppress debug prompt
                // Prevent shells from being 'interactive' which can trigger config loading
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Fish => {
                // For fish shell, we can disable the prompt via function
                environment.insert("fish_prompt".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Ksh => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Csh | ModelShellType::Tcsh => {
                environment.insert("prompt".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Dash => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::PowerShell => {
                // PowerShell doesn't have PS1, but we can set a minimal prompt
                environment.insert("PROMPT".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Cmd => {
                // Windows CMD doesn't have environment-based prompt suppression
                environment.insert("TERM".to_string(), "dumb".to_string());
            },
            ModelShellType::Other => {
                // For unknown shells, try PS1 suppression as fallback
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string());
                environment.insert("PS3".to_string(), "".to_string());
                environment.insert("PS4".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
        }
        
        let session = TerminalSession::with_environment(
            shell_type,
            working_dir,
            environment
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

        // Check if we should use direct execution (faster, cleaner)
        if DirectExecutor::should_use_direct_execution(&command) {
            info!("Using direct execution for command: {}", command);
            
            // For now, fallback to PTY execution to avoid async issues
            // TODO: Implement proper async execution in background thread
            info!("Temporarily falling back to PTY execution");
        }

        info!("Using PTY execution for command: {}", command);
        
        // Create command block and add to history first
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let mut command_block = CommandBlock::new(command.clone(), working_dir.clone());
        command_block.mark_running();
        self.command_history.push(command_block);
        
        // Record command execution time for timeout detection
        self.last_command_time = Some(std::time::Instant::now());

        // UI will be updated automatically on the next frame

        if let Some(_terminal) = &mut self.terminal {
            // Send command directly to PTY with newline so the shell executes it
            if let Some(handle) = _terminal.pty_handle() {
                let mut pty_manager = self.pty_manager.lock().await;
                let cmd = format!("{}\n", command);
                if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                    warn!("Failed to send input to PTY: {}", e);
                }
            }

            // Leave the block in Running; async loop will collect output and we can mark done later
            self.state.status_message = Some(format!("Running: {}", command));
            info!("Command '{}' queued", command);
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


    /// Set status message
    pub fn set_status_message(&mut self, message: Option<String>) {
        self.state.status_message = message;
    }
}

impl eframe::App for MosaicTermApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Only print debug once per second to avoid spam
        static mut LAST_DEBUG_TIME: Option<std::time::Instant> = None;
        unsafe {
            let now = std::time::Instant::now();
            if LAST_DEBUG_TIME.is_none() || now.duration_since(LAST_DEBUG_TIME.unwrap()).as_secs() >= 1 {
                println!("🔄 MosaicTerm UI is rendering...");
                LAST_DEBUG_TIME = Some(now);
            }
        }

        // Initialize terminal on first startup
        if self.terminal.is_none() && !self.state.terminal_ready && !self.state.initialization_attempted {
            self.state.initialization_attempted = true;
            info!("Initializing terminal session...");

            // Initialize terminal asynchronously
            match futures::executor::block_on(self.initialize_terminal()) {
                Ok(()) => {
                    info!("Terminal session initialized successfully");
                }
                Err(e) => {
                    error!("Failed to initialize terminal: {}", e);
                    self.state.status_message = Some(format!("Terminal init failed: {}", e));
                }
            }
        }

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
            // Calculate available space for layout
            let available_height = ui.available_height();
            let input_height = 120.0; // Fixed height for input area
            let history_height = available_height - input_height - 20.0; // Leave some margin

            // Layout from bottom to top: input at bottom, then history above it
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                // FIXED INPUT AREA AT BOTTOM - This stays static
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), input_height),
                    egui::Layout::left_to_right(egui::Align::LEFT),
                    |ui| {
                        self.render_fixed_input_area(ui);
                    }
                );

                // HISTORY AREA ABOVE INPUT - Scrollable, with commands stacking from newest to oldest
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), history_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.render_command_history_area(ui);
                    }
                );
            });
        });

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

    /// Set up visual style for the application
    fn setup_visual_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();

        // Set up a terminal-inspired theme
        style.visuals.dark_mode = true;
        style.visuals.window_fill = egui::Color32::from_rgb(15, 15, 25);
        style.visuals.panel_fill = egui::Color32::from_rgb(20, 20, 35);

        // Customize widget spacing for better terminal-like appearance
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        // Set font size for better readability
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::monospace(12.0)
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(11.0)
        );

        // Apply the style
        ctx.set_style(style);
    }

    /// Handle keyboard shortcuts and navigation
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    fn handle_interrupt_command(&mut self) {
        if let Some(_terminal) = &mut self.terminal {
            // Send interrupt signal to current process
            if let Err(e) = executor::block_on(async {
                let _pty_manager = self.pty_manager.lock().await;
                // In a real implementation, this would send SIGINT
                // For now, just log and update status
                Ok::<(), mosaicterm::error::Error>(())
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
        if let Some(_terminal) = &mut self.terminal {
            let _ = _terminal.process_input("clear");
        }
        info!("Clear screen requested (Ctrl+L)");
        self.set_status_message(Some("Screen cleared".to_string()));
    }

    /// Handle exit (Ctrl+D)
    fn handle_exit(&mut self) {
        // Send EOF to shell (Ctrl+D)
        if let Some(_terminal) = &mut self.terminal {
            let _ = _terminal.process_input("\x04"); // EOF character
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

    /// Render the status bar

    /// Render the fixed input area at the bottom
    fn render_fixed_input_area(&mut self, ui: &mut egui::Ui) {
        // Create a fixed input block with clear visual boundaries
        let input_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(25, 25, 35))
            .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 100, 150)))
            .inner_margin(egui::Margin::symmetric(15.0, 10.0))
            .outer_margin(egui::Margin::symmetric(5.0, 5.0));

        input_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Input prompt label
                ui.label(egui::RichText::new("$")
                    .font(egui::FontId::monospace(16.0))
                    .color(egui::Color32::from_rgb(100, 200, 100))
                    .strong());

                // Get current input for display
                let mut current_input = self.input_prompt.current_input().to_string();

                // Input field - take full width
                let input_response = ui.add(
                    egui::TextEdit::singleline(&mut current_input)
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(f32::INFINITY)
                        .hint_text("Type a command and press Enter...")
                        .margin(egui::Vec2::new(8.0, 6.0))
                );
                // Ensure the input keeps focus so typing works
                if !input_response.has_focus() {
                    input_response.request_focus();
                }

                // Update the input prompt with the current input
                self.input_prompt.set_input(current_input.clone());

                // Handle Enter key to submit command
                if input_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !current_input.trim().is_empty() {
                        let command = current_input.clone();
                        self.input_prompt.add_to_history(command.clone());
                        self.input_prompt.clear_input();

                        // Handle the command
                        let _ = futures::executor::block_on(self.handle_command_input(command));
                    }
                }

                // Handle arrow keys for history navigation
                if input_response.has_focus() {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.input_prompt.navigate_history_previous();
                    } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.input_prompt.navigate_history_next();
                    }
                }
            });
        });
    }

    /// Render the command history area above the input
    fn render_command_history_area(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            // Status bar at the top
            let status_frame = egui::Frame::none()
                .fill(egui::Color32::from_rgb(35, 35, 45))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 100)))
                .inner_margin(egui::Margin::symmetric(10.0, 5.0));

            status_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("MosaicTerm")
                        .font(egui::FontId::proportional(16.0))
                        .color(egui::Color32::from_rgb(200, 200, 255))
                        .strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new("Ready")
                            .font(egui::FontId::proportional(12.0))
                            .color(egui::Color32::from_rgb(150, 255, 150)));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("History: {}", self.command_history.len()))
                            .font(egui::FontId::monospace(12.0))
                            .color(egui::Color32::from_rgb(200, 200, 200)));
                    });
                });
            });

            // Scrollable command history - commands from newest to oldest (bottom to top)
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Commands appear in execution order: oldest at top, newest at bottom
                        for (i, block) in self.command_history.iter().enumerate() {
                            Self::render_single_command_block_static(ui, block, i);
                        }

                        // If no commands, show welcome message
                        if self.command_history.is_empty() {
                            ui.add_space(50.0);
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new("🎉 Welcome to MosaicTerm!")
                                    .font(egui::FontId::proportional(24.0))
                                    .color(egui::Color32::from_rgb(255, 200, 100))
                                    .strong());
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new("Type a command in the input area below")
                                    .font(egui::FontId::proportional(16.0))
                                    .color(egui::Color32::from_rgb(200, 200, 255)));
                            });
                        }
                    });
                });
        });
    }

    /// Render a single command block (static version to avoid borrow checker issues)
    fn render_single_command_block_static(ui: &mut egui::Ui, block: &CommandBlock, _index: usize) {
        let block_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(45, 45, 55))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 100)))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .outer_margin(egui::Margin::symmetric(0.0, 4.0));

        block_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Command header with timestamp and status
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&block.command)
                        .font(egui::FontId::monospace(14.0))
                        .color(egui::Color32::from_rgb(200, 200, 255)));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        let (status_text, status_color) = match block.status {
                            ExecutionStatus::Running => ("Running", egui::Color32::from_rgb(255, 200, 0)),
                            ExecutionStatus::Completed => ("Done", egui::Color32::from_rgb(0, 255, 100)),
                            ExecutionStatus::Failed => ("Failed", egui::Color32::from_rgb(255, 100, 100)),
                            ExecutionStatus::Pending => ("Pending", egui::Color32::from_rgb(150, 150, 150)),
                        };

                        ui.label(egui::RichText::new(status_text)
                            .font(egui::FontId::proportional(12.0))
                            .color(status_color));
                    });
                });

                // Output area if available
                if !block.output.is_empty() {
                    ui.add_space(6.0);
                    let output_frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgb(25, 25, 35))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 80)))
                        .inner_margin(egui::Margin::symmetric(8.0, 6.0));

                    output_frame.show(ui, |ui| {
                        for line in block.output.iter() { // Show all output lines
                            ui.label(egui::RichText::new(&line.text)
                                .font(egui::FontId::monospace(12.0))
                                .color(egui::Color32::from_rgb(180, 180, 200)));
                        }
                    });
                }

                // Timestamp
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&format!("{}", block.timestamp.format("%H:%M:%S")))
                        .font(egui::FontId::monospace(10.0))
                        .color(egui::Color32::from_rgb(120, 120, 140)));
                });
            });
        });
    }


    /// Handle async operations (called from update) - SIMPLIFIED VERSION
    fn handle_async_operations(&mut self, _ctx: &egui::Context) {
        // SIMPLIFIED: Poll PTY output and add to current command (no complex prompt detection)
        if let Some(_terminal) = &mut self.terminal {
            if let Some(handle) = _terminal.pty_handle() {
                if let Ok(mut pty_manager) = self.pty_manager.try_lock() {
                    if let Ok(data) = pty_manager.try_read_output_now(handle) {
                        if !data.is_empty() {
                            // Process output
                            debug!("PTY read {} bytes: {:?}", data.len(), String::from_utf8_lossy(&data));
                            let _ = futures::executor::block_on(async {
                                _terminal.process_output(&data, mosaicterm::terminal::StreamType::Stdout).await
                            });

                            // Add output to current command block
                            let ready_lines = _terminal.take_ready_output_lines();
                            debug!("Got {} ready lines from terminal", ready_lines.len());
                            if let Some(last_block) = self.command_history.last_mut() {
                                let has_ready_lines = !ready_lines.is_empty();
                                for line in ready_lines {
                                    // Only skip exact command echo
                                    if line.text.trim() != last_block.command.trim() {
                                        last_block.add_output_line(line);
                                    }
                                }
                                
                                // Also handle partial data (no newlines) as immediate output
                                if !has_ready_lines {
                                    let text = String::from_utf8_lossy(&data);
                                    debug!("Processing partial data: '{}'", text.trim());
                                    
                                    let trimmed_text = text.trim();
                                    
                                    // Since we've suppressed prompts at the source, we can add any non-empty text
                                    // that's not the command echo
                                    if !trimmed_text.is_empty() && 
                                       trimmed_text != last_block.command.trim() {
                                        // Add partial data as a line immediately
                                        let partial_line = mosaicterm::models::OutputLine {
                                            text: trimmed_text.to_string(),
                                            ansi_codes: vec![],
                                            line_number: 0,
                                            timestamp: chrono::Utc::now(),
                                        };
                                        last_block.add_output_line(partial_line);
                                        debug!("Added partial data as output line: '{}'", trimmed_text);
                                    }
                                }
                                
                                // Simple timeout-based completion (1 second)
                                if let Some(start_time) = self.last_command_time {
                                    if start_time.elapsed().as_millis() > 1000 {
                                        last_block.mark_completed(std::time::Duration::from_millis(100));
                                        self.last_command_time = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
    fn test_app_state() {
        let state = AppState::default();
        assert_eq!(state.title, "MosaicTerm");
        assert!(!state.terminal_ready);
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
