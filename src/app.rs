//! Main application structure and state management
//!
//! This module contains the core `MosaicTermApp` struct that implements the `eframe::App` trait,
//! providing the main GUI application logic. It handles:
//!
//! - **UI Rendering:** Command history blocks, input prompt, status bar
//! - **PTY Management:** Creating and managing pseudoterminal processes
//! - **Output Processing:** Reading PTY output, parsing ANSI codes, updating command blocks
//! - **User Input:** Keyboard events, command submission, tab completion
//! - **State Management:** Terminal state, command history, configuration
//!
//! ## Architecture
//!
//! The app runs in a single-threaded event loop managed by `egui`, with background threads
//! for PTY I/O. Communication happens via async channels.
//!
//! ### Main Components
//!
//! - `MosaicTermApp`: Core application state
//! - `handle_async_operations()`: Processes PTY output in the update loop
//! - `render_ui()`: Renders the three-panel UI layout
//!
//! ### UI Layout
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │ Top Panel (Status Bar)                  │
//! ├─────────────────────────────────────────┤
//! │                                         │
//! │ Central Panel (Command History)         │
//! │ - Scrollable command blocks             │
//! │ - ANSI-formatted output                 │
//! │                                         │
//! ├─────────────────────────────────────────┤
//! │ Bottom Panel (Input Prompt)             │
//! │ - Always visible and pinned             │
//! └─────────────────────────────────────────┘
//! ```
//!
//! ## Performance Considerations
//!
//! - **Conditional Repaints:** Only repaints when needed (running command, pending output)
//! - **Output Batching:** Processes multiple lines at once to reduce UI updates
//! - **Size Limits:** Enforces max lines per command (50K) and chars per line (10K)
//! - **Lock Retry:** Retries PTY lock acquisition to prevent dropped output

use arboard::Clipboard;
use eframe::egui;
use futures::executor;
use mosaicterm::completion::CompletionProvider;
use mosaicterm::config::{prompt::PromptFormatter, RuntimeConfig};
use mosaicterm::context::ContextDetector;
use mosaicterm::error::Result;
use mosaicterm::execution::DirectExecutor;
use mosaicterm::models::{CommandBlock, ExecutionStatus};
use mosaicterm::models::{ShellType as ModelShellType, TerminalSession};
use mosaicterm::pty::PtyManager;
use mosaicterm::state_manager::StateManager;
use mosaicterm::terminal::{Terminal, TerminalFactory};
use mosaicterm::ui::{
    CommandBlocks, CompletionPopup, InputPrompt, MetricsPanel, ScrollableHistory,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// Output size limits to prevent memory leaks
const MAX_OUTPUT_LINES_PER_COMMAND: usize = 10_000;
const MAX_LINE_LENGTH: usize = 10_000;

/// Async operation request sent from UI to background task
#[derive(Debug, Clone)]
enum AsyncRequest {
    /// Execute a command
    ExecuteCommand(String),
    /// Initialize terminal
    InitTerminal,
    /// Restart PTY session
    RestartPty,
    /// Send interrupt signal
    SendInterrupt(String), // PTY handle ID
}

/// Async operation result sent from background task to UI
#[derive(Debug, Clone)]
enum AsyncResult {
    /// Command execution started (initial block added to history)
    #[allow(dead_code)]
    CommandStarted(CommandBlock),
    /// Command execution completed (update existing block)
    #[allow(dead_code)]
    CommandCompleted {
        index: usize,
        status: ExecutionStatus,
        exit_code: Option<i32>,
    },
    /// Terminal initialized successfully
    TerminalInitialized,
    /// Terminal initialization failed
    TerminalInitFailed(String),
    /// PTY restarted successfully
    PtyRestarted,
    /// PTY restart failed
    PtyRestartFailed(String),
    /// Interrupt signal sent
    InterruptSent,
    /// Interrupt signal failed
    InterruptFailed(String),
}

/// Main MosaicTerm application
pub struct MosaicTermApp {
    /// Centralized state manager - single source of truth
    state_manager: StateManager,
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
    completion_popup: CompletionPopup,
    metrics_panel: MetricsPanel,
    /// Runtime configuration
    runtime_config: RuntimeConfig,
    /// Completion provider
    completion_provider: CompletionProvider,
    /// History manager for persistent command history
    history_manager: mosaicterm::history::HistoryManager,
    /// Flag to show history search popup
    history_search_active: bool,
    /// Current history search query
    history_search_query: String,
    /// Prompt formatter for custom prompts
    prompt_formatter: PromptFormatter,
    /// Context detector for environment tracking (venv, nvm, conda, etc.)
    context_detector: ContextDetector,
    /// State tracking for environment query across batches
    env_query_in_progress: bool,
    env_query_lines: Vec<String>,
    /// Tokio runtime for async operations
    /// Note: Field is kept alive to prevent runtime shutdown, even though it's not directly accessed
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    /// Channel for sending async requests from UI to background
    async_tx: mpsc::UnboundedSender<AsyncRequest>,
    /// Channel for receiving async results from background to UI
    async_rx: mpsc::UnboundedReceiver<AsyncResult>,
    /// Fullscreen TUI overlay for interactive apps
    tui_overlay: mosaicterm::ui::TuiOverlay,
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
        let scrollable_history = ScrollableHistory::new();
        let completion_popup = CompletionPopup::new();
        let metrics_panel = MetricsPanel::new();

        let runtime_config = RuntimeConfig::new().unwrap_or_else(|e| {
            error!("Failed to create runtime config: {}", e);
            warn!("Using minimal default configuration to continue");

            // Create a minimal working config instead of panicking
            RuntimeConfig::new_minimal()
        });

        // Create prompt formatter from config
        let prompt_format = runtime_config.config().terminal.prompt_format.clone();
        info!("Loading prompt format from config: '{}'", prompt_format);
        let prompt_formatter = PromptFormatter::new(prompt_format);

        // Create input prompt with initial prompt rendering
        let mut input_prompt = InputPrompt::new();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let initial_prompt = prompt_formatter.render(&working_dir);
        info!("Initial prompt rendered as: '{}'", initial_prompt);
        input_prompt.set_prompt(&initial_prompt);

        // Create Tokio runtime for async operations
        // Try multi-threaded first, fallback to single-threaded if that fails
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2) // Minimal threads for our needs
            .thread_name("mosaicterm-async")
            .enable_all()
            .build()
            .or_else(|e| {
                warn!("Failed to create multi-threaded runtime: {}, trying single-threaded", e);
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
            })
            .unwrap_or_else(|e| {
                error!("Failed to create any Tokio runtime: {}", e);
                panic!("Critical: Cannot initialize MosaicTerm without Tokio runtime. This is a system configuration issue.");
            });

        // Create channels for async communication
        let (request_tx, mut request_rx) = mpsc::unbounded_channel();
        let (result_tx, result_rx) = mpsc::unbounded_channel();

        // Clone handles for background task
        let pty_manager_clone = pty_manager.clone();
        let terminal_factory_clone = terminal_factory.clone();

        // Spawn background task to handle async operations
        runtime.spawn(async move {
            Self::async_operation_loop(
                &mut request_rx,
                result_tx,
                pty_manager_clone,
                terminal_factory_clone,
            )
            .await;
        });

        // Initialize StateManager and add demo commands directly
        let mut state_manager = StateManager::new();
        Self::add_demo_commands(&mut state_manager);

        Self {
            state_manager,
            terminal: None,
            pty_manager,
            terminal_factory,
            command_blocks,
            input_prompt,
            scrollable_history,
            completion_popup,
            metrics_panel,
            runtime_config,
            completion_provider: CompletionProvider::new(),
            history_manager: mosaicterm::history::HistoryManager::new().unwrap_or_else(|e| {
                error!("Failed to create history manager: {}", e);
                mosaicterm::history::HistoryManager::default()
            }),
            history_search_active: false,
            history_search_query: String::new(),
            prompt_formatter,
            context_detector: ContextDetector::new(),
            env_query_in_progress: false,
            env_query_lines: Vec::new(),
            runtime,
            async_tx: request_tx,
            async_rx: result_rx,
            tui_overlay: mosaicterm::ui::TuiOverlay::new(),
        }
    }

    /// Create application with runtime configuration
    pub fn with_config(runtime_config: RuntimeConfig) -> Self {
        let mut app = Self::new();
        app.runtime_config = runtime_config;
        app
    }

    /// Add demo commands to state manager for initial UI display
    fn add_demo_commands(state_manager: &mut StateManager) {
        let demo_commands = vec![
            ("pwd", "Current working directory"),
            ("ls -la", "List all files with details"),
            ("echo 'Hello from MosaicTerm!'", "Print a greeting message"),
        ];

        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

        for (cmd, _description) in demo_commands {
            let mut block =
                mosaicterm::models::CommandBlock::new(cmd.to_string(), working_dir.clone());

            // Simulate some output for demo
            if cmd == "pwd" {
                block.add_output_line(mosaicterm::models::OutputLine {
                    text: working_dir.to_string_lossy().to_string(),
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

            // Add directly to state manager (single source of truth)
            state_manager.add_command_block(block);
        }
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
            ModelShellType::Ksh
            | ModelShellType::Csh
            | ModelShellType::Tcsh
            | ModelShellType::Dash
            | ModelShellType::PowerShell
            | ModelShellType::Cmd => ModelShellType::Bash, // Default to bash
            ModelShellType::Other => ModelShellType::Bash, // Default to bash for unknown shells
        };

        // Create terminal session configuration
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));

        // Create environment with prompt suppression
        let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();

        // Disable terminal echo explicitly
        environment.insert("STTY".to_string(), "-echo".to_string());

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
            }
            ModelShellType::Fish => {
                // For fish shell, we can disable the prompt via function
                environment.insert("fish_prompt".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::Ksh => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::Csh | ModelShellType::Tcsh => {
                environment.insert("prompt".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::Dash => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::PowerShell => {
                // PowerShell doesn't have PS1, but we can set a minimal prompt
                environment.insert("PROMPT".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::Cmd => {
                // Windows CMD doesn't have environment-based prompt suppression
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
            ModelShellType::Other => {
                // For unknown shells, try PS1 suppression as fallback
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string());
                environment.insert("PS3".to_string(), "".to_string());
                environment.insert("PS4".to_string(), "".to_string());
                environment.insert("TERM".to_string(), "dumb".to_string());
            }
        }

        let session = TerminalSession::with_environment(shell_type, working_dir, environment);

        // Create and initialize terminal
        let terminal = self.terminal_factory.create_and_initialize(session).await?;
        self.terminal = Some(terminal);

        // Update state manager
        self.state_manager.set_terminal_ready(true);

        // State is now managed through StateManager only

        // Send PS1 override to ensure prompts don't appear in output
        // This is necessary because we now load RC files which may set PS1
        if let Some(terminal) = &self.terminal {
            if let Some(handle) = terminal.pty_handle() {
                let mut pty_manager = self.pty_manager.lock().await;

                // Override PS1 set by rc files with a more robust approach
                // Use PROMPT_COMMAND to ensure PS1 stays empty even if venv/conda modifies it
                let ps1_override = match shell_type {
                    ModelShellType::Bash => {
                        // For bash, use PROMPT_COMMAND to override PS1 before each prompt
                        "export PROMPT_COMMAND='PS1=\"\"; PS2=\"\"; PS3=\"\"; PS4=\"\"'\n"
                    }
                    ModelShellType::Zsh => {
                        // For zsh, use precmd hook to override PS1 before each prompt
                        "precmd() { PS1=''; PS2=''; PS3=''; PS4=''; }\n"
                    }
                    ModelShellType::Fish => "function fish_prompt; end\n",
                    ModelShellType::Ksh => "export PS1=''; export PS2=''\n",
                    ModelShellType::Csh | ModelShellType::Tcsh => "set prompt=''\n",
                    ModelShellType::Dash => "export PS1=''\n",
                    _ => "",
                };

                if !ps1_override.is_empty() {
                    if let Err(e) = pty_manager
                        .send_input(handle, ps1_override.as_bytes())
                        .await
                    {
                        warn!("Failed to override PS1 after initialization: {}", e);
                    } else {
                        info!("Sent PS1 override with persistent hook to suppress shell prompts");
                    }
                    // Note: Shell will process PS1 override asynchronously
                    // The hook ensures PS1 stays empty even if venv/conda tries to modify it
                }
            }
        }

        // Update contexts and prompt after terminal initialization
        self.update_contexts();
        self.update_prompt();

        info!("Terminal session initialized successfully with environment support");
        Ok(())
    }

    /// Handle command input from the UI
    pub async fn handle_command_input(&mut self, command: String) -> Result<()> {
        if command.trim().is_empty() {
            return Ok(());
        }

        info!("Processing command: {}", command);

        // Add command to persistent history
        if let Err(e) = self.history_manager.add(command.clone()) {
            warn!("Failed to add command to history: {}", e);
        }

        // Check if this is a TUI command that should open in fullscreen overlay
        if self.is_tui_command(&command) {
            info!(
                "TUI command detected, opening fullscreen overlay: {}",
                command
            );
            return self.handle_tui_command(command).await;
        }

        // Check if this is an interactive command and warn the user
        if self.is_interactive_command(&command) {
            warn!("Interactive command detected: {}", command);
            self.set_status_message(Some(format!(
                "⚠️  '{}' is an interactive program and may not work correctly in block mode",
                self.get_command_name(&command)
            )));
        }

        // Check if this is a clear command and handle it specially
        let trimmed_command = command.trim();
        if trimmed_command == "clear" || trimmed_command == "clear\n" {
            info!("Clear command detected, clearing screen");
            self.state_manager.clear_command_history();
            self.set_status_message(Some("Screen cleared".to_string()));
            // Still send the command to the shell so it clears its own state
            if let Some(_terminal) = &mut self.terminal {
                if let Some(handle) = _terminal.pty_handle() {
                    let mut pty_manager = self.pty_manager.lock().await;
                    let cmd = format!("{}\n", command);
                    if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                        warn!("Failed to send clear command to PTY: {}", e);
                    }
                }
            }
            return Ok(());
        }

        // Check if this is a cd command and update working directory
        self.update_working_directory_if_cd(&command);

        // Check if we should use direct execution (faster, cleaner)
        if DirectExecutor::should_use_direct_execution(&command) {
            info!("Using direct execution for command: {}", command);

            // NOTE: Direct execution optimization is planned for future release
            // This would execute simple commands (like `ls`, `pwd`) directly without PTY overhead
            // Requires proper async execution in background thread to avoid blocking UI
            // For now, all commands use PTY execution for consistency and full shell integration
            info!("Using PTY execution for consistency (direct execution optimization planned)");
        }

        info!("Using PTY execution for command: {}", command);

        // Create command block and add to history first
        let working_dir = self
            .terminal
            .as_ref()
            .map(|t| t.get_working_directory().to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            });
        // Clone command once since it's used later, but move working_dir
        let command_for_block = command.clone();
        let mut command_block = CommandBlock::new(command_for_block, working_dir);
        command_block.mark_running();

        // Add to state manager (single source of truth) - move instead of clone
        self.state_manager.add_command_block(command_block);

        // DEPRECATED: Also update old field during migration
        // Command block is now managed through StateManager only

        // Record command execution time for timeout detection
        self.state_manager.set_last_command_time(chrono::Utc::now());

        // UI will be updated automatically on the next frame

        if let Some(_terminal) = &mut self.terminal {
            // Send command directly to PTY with newline so the shell executes it
            // Then send a command to echo the exit code with a special marker
            if let Some(handle) = _terminal.pty_handle() {
                let mut pty_manager = self.pty_manager.lock().await;
                let cmd = format!("{}\n", command);
                if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                    warn!("Failed to send input to PTY: {}", e);
                }

                // Send a command to echo the exit code after the command completes
                // Use a unique marker so we can detect it
                let exit_code_cmd = "echo \"MOSAICTERM_EXITCODE:$?\"\n";
                if let Err(e) = pty_manager
                    .send_input(handle, exit_code_cmd.as_bytes())
                    .await
                {
                    warn!("Failed to send exit code check to PTY: {}", e);
                }
            }

            // Leave the block in Running; async loop will collect output and we can mark done later
            self.state_manager
                .set_status_message(Some(format!("Running: {}", command)));

            info!("Command '{}' queued", command);
        } else {
            warn!("Terminal not initialized, cannot execute command");

            self.state_manager
                .set_status_message(Some("Terminal not ready".to_string()));
        }

        Ok(())
    }

    /// Update working directory if command is a cd command
    fn update_working_directory_if_cd(&mut self, command: &str) {
        let trimmed = command.trim();

        // Parse cd command (handle various forms: cd, cd -, cd ~, cd /path, etc.)
        if !trimmed.starts_with("cd")
            && !trimmed.starts_with("pushd")
            && !trimmed.starts_with("popd")
        {
            return;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.is_empty() || (parts[0] != "cd" && parts[0] != "pushd" && parts[0] != "popd") {
            return;
        }

        if let Some(terminal) = &mut self.terminal {
            let current_dir = terminal.get_working_directory().to_path_buf();

            let new_dir = if parts.len() == 1 || (parts[0] == "cd" && parts.len() == 1) {
                // cd with no arguments goes to home
                if let Some(home) = std::env::var_os("HOME") {
                    std::path::PathBuf::from(home)
                } else {
                    current_dir.clone()
                }
            } else if parts[1] == "-" {
                // cd - goes to previous directory
                if let Some(prev_dir) = self.state_manager.get_previous_directory() {
                    prev_dir
                } else {
                    debug!("No previous directory to return to");
                    current_dir.clone()
                }
            } else if parts[1] == "~" {
                // cd ~ goes to home
                if let Some(home) = std::env::var_os("HOME") {
                    std::path::PathBuf::from(home)
                } else {
                    current_dir.clone()
                }
            } else if parts[1].starts_with("~/") {
                // cd ~/path expands tilde
                if let Some(home) = std::env::var_os("HOME") {
                    std::path::PathBuf::from(home).join(&parts[1][2..])
                } else {
                    current_dir.clone()
                }
            } else if parts[1].starts_with('/') {
                // cd /absolute/path
                std::path::PathBuf::from(parts[1])
            } else {
                // cd relative/path
                current_dir.join(parts[1])
            };

            // Canonicalize and update if valid
            if let Ok(canonical) = new_dir.canonicalize() {
                if canonical.is_dir() && canonical != current_dir {
                    // Save current directory as previous before changing
                    self.state_manager
                        .set_previous_directory(Some(current_dir.clone()));

                    terminal.set_working_directory(canonical.clone());
                    debug!(
                        "Updated working directory from {:?} to: {:?}",
                        current_dir, canonical
                    );
                    // Update contexts and prompt after directory change
                    self.update_contexts();
                    self.update_prompt();
                }
            } else {
                debug!("Failed to change directory to: {:?}", new_dir);
            }
        }
    }

    /// Update the prompt display based on current working directory
    fn update_prompt(&mut self) {
        if let Some(terminal) = &self.terminal {
            let working_dir = terminal.get_working_directory();
            let base_prompt = self.prompt_formatter.render(working_dir);

            let mut left_context = String::new();
            let mut right_context = String::new();

            // Add environment contexts (venv, conda, nvm) on the LEFT
            if let Some(session) = self.state_manager.active_session() {
                if !session.active_contexts.is_empty() {
                    let context_str = session.active_contexts.join(" ");
                    left_context = format!("({}) ", context_str);
                }

                // Add git context on the RIGHT (if present)
                if let Some(git) = &session.git_context {
                    right_context = format!(" [{}]", git);
                }
            }

            // Format: (venv:name) ~/path$ [git:branch]
            let rendered_prompt = format!("{}{}{}", left_context, base_prompt, right_context);

            debug!(
                "Updated prompt to: '{}' (working_dir: {:?})",
                rendered_prompt, working_dir
            );
            self.input_prompt.set_prompt(&rendered_prompt);
        }
    }

    /// Update active environment contexts based on current shell environment
    /// Note: This only updates git context for now. Full env querying happens
    /// asynchronously after command completion to avoid blocking.
    fn update_contexts(&mut self) {
        // For now, just update git context (synchronous filesystem check)
        // Environment variable detection happens after command completion
        self.update_git_context();
    }

    /// Update just the git context (synchronous filesystem check)
    fn update_git_context(&mut self) {
        let git_context = if let Some(terminal) = &self.terminal {
            let working_dir = terminal.get_working_directory();

            if let Ok(repo) = git2::Repository::discover(working_dir) {
                if let Ok(head) = repo.head() {
                    if let Some(branch_name) = head.shorthand() {
                        let has_changes = if let Ok(statuses) = repo.statuses(None) {
                            !statuses.is_empty()
                        } else {
                            false
                        };

                        if has_changes {
                            Some(format!("{} *", branch_name))
                        } else {
                            Some(branch_name.to_string())
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Update git context
        if let Some(session) = self.state_manager.active_session_mut() {
            session.git_context = git_context;
        }
    }

    /// Parse environment output and update contexts
    fn parse_env_output(&mut self, output: &str) {
        info!("Parsing environment output: {}", output);
        let mut env = std::collections::HashMap::new();

        // Parse output lines like "VIRTUAL_ENV=/path/to/venv"
        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                // Only add non-empty values
                if !value.is_empty() {
                    info!("Found env var: {}={}", key, value);
                    env.insert(key.to_string(), value.to_string());
                } else {
                    info!("Skipping empty env var: {}", key);
                }
            }
        }

        // Detect contexts from environment
        let contexts = self.context_detector.detect_contexts(&env);
        info!("Detected {} contexts: {:?}", contexts.len(), contexts);

        // Format contexts for display (venv/conda/nvm only - git handled separately)
        let env_context_strings: Vec<String> = contexts.iter().map(|c| c.format_short()).collect();

        info!("Formatted context strings: {:?}", env_context_strings);

        // Get git context separately (already updated by update_git_context)
        // No need to recompute it here

        // Update state manager with env contexts
        if let Some(session) = self.state_manager.active_session_mut() {
            session.active_contexts = env_context_strings.clone();
            info!("Updated session contexts to: {:?}", session.active_contexts);
        }

        info!("Updated contexts from shell environment");
    }

    /// Check if a command is a TUI app that should open in fullscreen overlay
    fn is_tui_command(&self, command: &str) -> bool {
        let cmd_name = self.get_command_name(command);
        self.runtime_config
            .config()
            .tui_apps
            .fullscreen_commands
            .iter()
            .any(|prog| prog == &cmd_name)
    }

    /// Handle a TUI command by opening the fullscreen overlay
    async fn handle_tui_command(&mut self, command: String) -> Result<()> {
        info!("Handling TUI command: {}", command);

        // Create command block (mark it as TUI mode)
        let working_dir = self
            .terminal
            .as_ref()
            .map(|t| t.get_working_directory().to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            });
        // Clone command since it's used later, but move working_dir
        let command_for_block = command.clone();
        let mut command_block = CommandBlock::new(command_for_block, working_dir);
        command_block.mark_tui_mode(); // Special marker for TUI commands

        // Add to state manager - move instead of clone
        self.state_manager.add_command_block(command_block);

        // Create a new PTY for the TUI app
        if let Some(_terminal) = &mut self.terminal {
            if let Some(handle) = _terminal.pty_handle() {
                // Store handle ID before mutable borrow
                let handle_id = handle.id.len(); // Simple ID based on handle ID length

                // Send command to PTY
                {
                    let mut pty_manager = self.pty_manager.lock().await;

                    // Set TERM to xterm-256color for proper TUI support
                    let env_cmd = "export TERM=xterm-256color\n";
                    if let Err(e) = pty_manager.send_input(handle, env_cmd.as_bytes()).await {
                        warn!("Failed to set TERM for TUI command: {}", e);
                    }

                    // Brief delay to let env command process
                    std::thread::sleep(std::time::Duration::from_millis(10));

                    let cmd = format!("{}\n", command);
                    if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                        warn!("Failed to send TUI command to PTY: {}", e);
                        return Err(e);
                    }
                } // Drop pty_manager lock here

                // Start the overlay with PTY handle ID
                self.tui_overlay.start(command.clone(), handle_id);
                self.set_status_message(Some(format!("Running TUI app: {}", command)));

                info!("TUI overlay started for command: {}", command);
            }
        }

        Ok(())
    }

    /// Check if a command is interactive (TUI-based) and may not work well in block mode
    fn is_interactive_command(&self, command: &str) -> bool {
        let cmd_name = self.get_command_name(command);

        // List of known interactive TUI programs (not handled by TUI overlay)
        let interactive_programs = [
            "python", "python3", "node", "irb", "ruby", "ssh", // REPLs and remote shells
        ];

        interactive_programs.iter().any(|&prog| cmd_name == prog)
    }

    /// Extract the command name from a command line
    fn get_command_name(&self, command: &str) -> String {
        command.split_whitespace().next().unwrap_or("").to_string()
    }

    /// Handle TUI app exit - mark command block as completed without output
    fn handle_tui_exit(&mut self, command: String) {
        info!("TUI app exited: {}", command);

        // Find the command block and mark it as completed
        if let Some(history) = self.state_manager.command_history_mut() {
            if let Some(block) = history.iter_mut().rev().find(|b| {
                b.command == command && b.status == mosaicterm::models::ExecutionStatus::TuiMode
            }) {
                // Mark as completed (no output will be shown)
                block.mark_completed(std::time::Duration::from_secs(0));
                info!("TUI command block marked as completed: {}", command);
            }
        }

        self.set_status_message(Some(format!("TUI app exited: {}", command)));
    }

    /// Update application state
    pub fn update_state(&mut self) {
        let terminal_ready = self.terminal.is_some();
        self.state_manager.set_terminal_ready(terminal_ready);

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
        self.state_manager.set_status_message(message.clone());
    }

    /// Start loading indicator with message
    pub fn start_loading(&mut self, message: impl Into<String>) {
        let msg_string = message.into();
        self.state_manager
            .set_loading(true, Some(msg_string.clone()));
        self.state_manager.app_state_mut().loading_frame = 0;
    }

    /// Stop loading indicator
    pub fn stop_loading(&mut self) {
        self.state_manager.set_loading(false, None);
    }

    /// Get loading spinner character for current frame
    fn loading_spinner(&self) -> &'static str {
        const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER_FRAMES[self.state_manager.loading_frame() % SPINNER_FRAMES.len()]
    }

    /// Convert technical error to user-friendly message
    ///
    /// Translates internal errors into actionable messages for end users.
    fn user_friendly_error(&self, error: &mosaicterm::error::Error) -> String {
        use mosaicterm::error::Error;

        match error {
            // PTY errors
            Error::PtyCreationFailed { .. } => {
                "Could not create terminal session. Please check your system configuration."
                    .to_string()
            }
            Error::CommandSpawnFailed { .. } => {
                "Could not start shell. Please verify your shell path in settings.".to_string()
            }
            Error::PtyHandleNotFound { .. }
            | Error::PtyStreamsNotFound { .. }
            | Error::InvalidPtyHandle => {
                "Terminal session error. Try restarting the application.".to_string()
            }
            Error::PtyReaderCloneFailed { .. } | Error::PtyWriterTakeFailed { .. } => {
                "Terminal I/O setup failed. Try restarting the application.".to_string()
            }
            Error::PtyInputSendFailed { .. } | Error::PtyReadFailed { .. } => {
                "Failed to communicate with terminal. Try restarting.".to_string()
            }

            // Signal errors
            Error::SignalSendFailed { .. } | Error::SignalNotSupported { .. } => {
                "Could not send signal to process.".to_string()
            }
            Error::ProcessNotRegistered { .. } | Error::NoPidAvailable { .. } => {
                "Process not found. It may have already terminated.".to_string()
            }

            // Command errors
            Error::CommandNotFound { command } => {
                format!(
                    "Command '{}' not found. Please check if it's installed and in PATH.",
                    command
                )
            }
            Error::CommandValidationFailed { reason, .. } => {
                format!("Command blocked: {}", reason)
            }
            Error::CommandTimeout { .. } => {
                "Command timed out. You can adjust timeout settings in configuration.".to_string()
            }
            Error::EmptyCommand => "Command cannot be empty.".to_string(),
            Error::NoPreviousCommand => "No previous command in history.".to_string(),

            // Configuration errors
            Error::ConfigLoadFailed { .. }
            | Error::ConfigSaveFailed { .. }
            | Error::ConfigWatchFailed { .. }
            | Error::ConfigNotFound
            | Error::ConfigValidationFailed { .. }
            | Error::ConfigSerializationFailed { .. }
            | Error::ConfigParseFailed { .. } => {
                format!("Configuration issue: {}. Using default settings.", error)
            }
            Error::ShellConfigNotFound { .. } => {
                "Shell configuration not found. Using defaults.".to_string()
            }
            Error::ThemeNotFound { theme_name } => {
                format!("Theme '{}' not found. Using default theme.", theme_name)
            }
            Error::ThemeAlreadyExists { .. } => "Theme already exists.".to_string(),
            Error::CannotRemoveBuiltInTheme { .. } => "Cannot remove built-in theme.".to_string(),
            Error::ThemeExportFailed { .. } | Error::ThemeImportFailed { .. } => {
                "Theme operation failed.".to_string()
            }
            Error::UnknownComponent { .. } | Error::UnknownColorScheme { .. } => {
                "Invalid theme component or scheme.".to_string()
            }

            // Terminal errors
            Error::NoPtyHandleAvailable => {
                "No terminal session available. Try restarting.".to_string()
            }
            Error::OutputBufferFull { .. } => {
                "Output buffer full. Command output was truncated.".to_string()
            }
            Error::Toml(e) => {
                format!("Configuration file error: {}. Using default settings.", e)
            }
            Error::Serde(e) => {
                format!("Data format error: {}. Please check your configuration.", e)
            }
            Error::Regex(e) => {
                format!("Pattern error: {}. Please check your syntax.", e)
            }
            Error::Io(e) => {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "Permission denied. Please check file permissions.".to_string()
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    "File or command not found. Please check the path.".to_string()
                } else {
                    format!("I/O error: {}. Please try again.", e)
                }
            }
            Error::Other(msg) => {
                // Try to make generic errors more helpful
                if msg.contains("not found") {
                    format!("Not found: {}. Please check your input.", msg)
                } else if msg.contains("parse") || msg.contains("syntax") {
                    format!("Syntax error: {}. Please check your command.", msg)
                } else {
                    format!("Error: {}. If this persists, please report it.", msg)
                }
            }
        }
    }

    /// Render context menu for command blocks
    fn render_context_menu(&mut self, ctx: &egui::Context) {
        // Check if we have an active context menu
        if let Some(block_id) = &self
            .command_blocks
            .interaction_state()
            .context_menu_block
            .clone()
        {
            if let Some(menu_pos) = self.command_blocks.interaction_state().context_menu_pos {
                // Find the command block and extract data we need
                let command_history = self.state_manager.get_command_history();
                if let Some(block) = command_history.iter().find(|b| &b.id == block_id) {
                    let command = block.command.clone();
                    let status = block.status;
                    let output_lines: Vec<String> = block
                        .output
                        .iter()
                        .map(|line| line.text.as_str().to_string())
                        .collect();

                    // Create context menu
                    let mut menu_open = true;
                    egui::Window::new("Context Menu")
                        .fixed_pos(menu_pos)
                        .resizable(false)
                        .collapsible(false)
                        .title_bar(false)
                        .show(ctx, |ui| {
                            ui.set_min_width(150.0);

                            // Rerun command
                            if ui.button("🔄 Rerun Command").clicked() {
                                // Execute the same command again (non-blocking)
                                // Clone once and reuse
                                let command_to_rerun = command.clone();
                                self.input_prompt.add_to_history(command_to_rerun.clone());
                                let _ = self
                                    .async_tx
                                    .send(AsyncRequest::ExecuteCommand(command_to_rerun.clone()));
                                menu_open = false;
                            }

                            // Kill running command (only if still running)
                            if status == ExecutionStatus::Running
                                && ui.button("❌ Kill Command").clicked()
                            {
                                self.handle_interrupt_specific_command(block_id.clone());
                                menu_open = false;
                            }

                            ui.separator();

                            // Copy command
                            if ui.button("📋 Copy Command").clicked() {
                                if let Ok(mut clipboard) = Clipboard::new() {
                                    let _ = clipboard.set_text(&command);
                                }
                                menu_open = false;
                            }

                            // Copy output
                            if ui.button("📄 Copy Output").clicked() {
                                let output_text = output_lines.join("\n");
                                if let Ok(mut clipboard) = Clipboard::new() {
                                    let _ = clipboard.set_text(&output_text);
                                }
                                menu_open = false;
                            }

                            // Copy both
                            if ui.button("📋📄 Copy Both").clicked() {
                                let output_text = output_lines.join("\n");
                                let both_text = format!("{}\n{}", command, output_text);
                                if let Ok(mut clipboard) = Clipboard::new() {
                                    let _ = clipboard.set_text(&both_text);
                                }
                                menu_open = false;
                            }
                        });

                    // Close menu if clicked outside or if an action was taken
                    if !menu_open {
                        self.command_blocks
                            .interaction_state_mut()
                            .context_menu_block = None;
                        self.command_blocks.interaction_state_mut().context_menu_pos = None;
                    }

                    // Close menu on any click outside
                    if ctx.input(|i| i.pointer.any_click()) {
                        if let Some(mouse_pos) = ctx.input(|i| i.pointer.hover_pos()) {
                            // Check if click is outside the menu area (rough approximation)
                            let menu_rect =
                                egui::Rect::from_min_size(menu_pos, egui::vec2(150.0, 120.0));
                            if !menu_rect.contains(mouse_pos) {
                                self.command_blocks
                                    .interaction_state_mut()
                                    .context_menu_block = None;
                                self.command_blocks.interaction_state_mut().context_menu_pos = None;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Background task loop to handle async operations
    async fn async_operation_loop(
        request_rx: &mut mpsc::UnboundedReceiver<AsyncRequest>,
        result_tx: mpsc::UnboundedSender<AsyncResult>,
        pty_manager: Arc<Mutex<PtyManager>>,
        terminal_factory: TerminalFactory,
    ) {
        info!("Starting async operation loop");

        while let Some(request) = request_rx.recv().await {
            match request {
                AsyncRequest::InitTerminal => {
                    info!("Processing async InitTerminal request");
                    let result = Self::async_initialize_terminal(&terminal_factory).await;
                    match result {
                        Ok(()) => {
                            let _ = result_tx.send(AsyncResult::TerminalInitialized);
                        }
                        Err(e) => {
                            let _ = result_tx.send(AsyncResult::TerminalInitFailed(e.to_string()));
                        }
                    }
                }
                AsyncRequest::ExecuteCommand(command) => {
                    info!("Processing async ExecuteCommand: {}", command);
                    // Command execution happens in the main thread via PTY
                    // The async loop just needs to signal completion
                    // For now, we'll handle this differently
                }
                AsyncRequest::RestartPty => {
                    info!("Processing async RestartPty request");
                    let result = Self::async_restart_pty(&pty_manager, &terminal_factory).await;
                    match result {
                        Ok(()) => {
                            let _ = result_tx.send(AsyncResult::PtyRestarted);
                        }
                        Err(e) => {
                            let _ = result_tx.send(AsyncResult::PtyRestartFailed(e.to_string()));
                        }
                    }
                }
                AsyncRequest::SendInterrupt(handle_id) => {
                    info!("Processing async SendInterrupt for handle: {}", handle_id);
                    let result = Self::async_send_interrupt(&pty_manager, &handle_id).await;
                    match result {
                        Ok(()) => {
                            let _ = result_tx.send(AsyncResult::InterruptSent);
                        }
                        Err(e) => {
                            let _ = result_tx.send(AsyncResult::InterruptFailed(e.to_string()));
                        }
                    }
                }
            }
        }

        info!("Async operation loop ended");
    }

    /// Helper: Initialize terminal in background
    async fn async_initialize_terminal(terminal_factory: &TerminalFactory) -> Result<()> {
        info!("Async terminal initialization started");

        // Get runtime config from environment or use defaults
        let runtime_config = RuntimeConfig::new().unwrap_or_else(|e| {
            error!("Failed to create runtime config: {}", e);
            RuntimeConfig::new_minimal()
        });

        let shell_type = match runtime_config.config().terminal.shell_type {
            ModelShellType::Bash => ModelShellType::Bash,
            ModelShellType::Zsh => ModelShellType::Zsh,
            ModelShellType::Fish => ModelShellType::Fish,
            _ => ModelShellType::Bash,
        };

        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
        let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();

        // Suppress prompts
        environment.insert("PS1".to_string(), "".to_string());
        environment.insert("PS2".to_string(), "".to_string());
        environment.insert("TERM".to_string(), "dumb".to_string());

        let session = TerminalSession::with_environment(shell_type, working_dir, environment);
        let _terminal = terminal_factory.create_and_initialize(session).await?;

        info!("Async terminal initialization completed");
        Ok(())
    }

    /// Helper: Restart PTY in background
    async fn async_restart_pty(
        _pty_manager: &Arc<Mutex<PtyManager>>,
        _terminal_factory: &TerminalFactory,
    ) -> Result<()> {
        info!("Async PTY restart started");

        // For now, just log - full implementation would recreate terminal
        // This is complex because we need to update app state

        info!("Async PTY restart completed");
        Ok(())
    }

    /// Helper: Send interrupt signal in background
    async fn async_send_interrupt(
        pty_manager: &Arc<Mutex<PtyManager>>,
        handle_id: &str,
    ) -> Result<()> {
        info!("Async interrupt signal for handle: {}", handle_id);

        let mut pty_manager = pty_manager.lock().await;

        // Find the PTY handle
        let pty_handle = mosaicterm::pty::PtyHandle {
            id: handle_id.to_string(),
            pid: None,
        };

        // First, send Ctrl+C (ASCII 3) directly to the PTY input
        // This is the polite way - gives the process a chance to clean up
        info!("Sending Ctrl+C to PTY");
        let _ = pty_manager.send_input(&pty_handle, &[3]).await;

        // Get the shell PID
        if let Ok(pty_info) = pty_manager.get_info(&pty_handle) {
            if let Some(shell_pid) = pty_info.pid {
                info!("Killing process tree for shell PID: {}", shell_pid);

                // Kill the entire process tree (shell + all children)
                // This ensures long-running commands like sleep, find, etc. are all killed
                // Uses platform abstraction for cross-platform support
                if let Err(e) = mosaicterm::pty::process_tree::kill_process_tree(shell_pid) {
                    warn!("Failed to kill process tree: {}", e);
                }

                // Wait a moment for processes to terminate gracefully
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                // If still running, check and log
                if let Ok(children) =
                    mosaicterm::pty::process_tree::get_all_descendant_pids(shell_pid)
                {
                    if !children.is_empty() {
                        info!("Some processes still running, attempting force kill");
                        // Try again - platform implementation should handle force kill
                        let _ = mosaicterm::pty::process_tree::kill_process_tree(shell_pid);
                    }
                }
            } else {
                return Err(mosaicterm::error::Error::NoPidAvailable {
                    handle_id: handle_id.to_string(),
                });
            }
        } else {
            return Err(mosaicterm::error::Error::PtyHandleNotFound {
                handle_id: handle_id.to_string(),
            });
        }

        Ok(())
    }
}

impl eframe::App for MosaicTermApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Handle keyboard shortcut for performance metrics (Ctrl+Shift+P)
        if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::P)) {
            self.metrics_panel.toggle();
        }

        // Only print debug once per second to avoid spam
        use std::sync::Mutex;
        static LAST_DEBUG_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);
        {
            let now = std::time::Instant::now();
            if let Ok(mut last_time) = LAST_DEBUG_TIME.lock() {
                let should_print = match *last_time {
                    None => true,
                    Some(prev) => now.duration_since(prev).as_secs() >= 1,
                };
                if should_print {
                    println!("🔄 MosaicTerm UI is rendering...");
                    *last_time = Some(now);
                }
            } else {
                // Mutex is poisoned - log but don't panic
                debug!("Debug time mutex is poisoned, skipping debug print");
            }
        }

        // Auto-refresh completion cache if needed (checks every frame but only refreshes after 5 min timeout)
        if let Err(e) = self.completion_provider.refresh_command_cache_if_needed() {
            debug!("Failed to refresh completion cache: {}", e);
        }

        // Periodic cleanup of terminated PTY processes (every 30 seconds)
        {
            use std::sync::Mutex;
            static LAST_CLEANUP_TIME: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let now = std::time::Instant::now();
            if let Ok(mut last_time) = LAST_CLEANUP_TIME.lock() {
                let should_cleanup = match *last_time {
                    None => true,
                    Some(prev) => now.duration_since(prev).as_secs() >= 30,
                };

                if should_cleanup {
                    if let Ok(mut pty_manager) = self.pty_manager.try_lock() {
                        let count_before = pty_manager.active_count();
                        pty_manager.cleanup_terminated();
                        let count_after = pty_manager.active_count();

                        if count_before > count_after {
                            info!(
                                "Cleaned up {} terminated PTY process(es)",
                                count_before - count_after
                            );
                        }

                        *last_time = Some(now);
                    }
                }
            } else {
                debug!("Cleanup time mutex is poisoned, skipping cleanup");
            }
        }

        // Update memory statistics periodically (every 5 seconds)
        {
            use std::sync::Mutex;
            static LAST_STATS_UPDATE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
            let now = std::time::Instant::now();
            if let Ok(mut last_time) = LAST_STATS_UPDATE.lock() {
                let should_update = match *last_time {
                    None => true,
                    Some(prev) => now.duration_since(prev).as_secs() >= 5,
                };

                if should_update {
                    self.state_manager.update_memory_stats();
                    *last_time = Some(now);
                }
            } else {
                debug!("Stats update mutex is poisoned, skipping stats update");
            }
        }

        // Initialize terminal on first startup
        if self.terminal.is_none()
            && !self.state_manager.is_terminal_ready()
            && !self.state_manager.is_initialization_attempted()
        {
            self.state_manager.set_initialization_attempted(true);

            info!("Initializing terminal session...");

            // Show loading indicator
            self.start_loading("Initializing terminal...");

            // Send async request to initialize terminal (non-blocking)
            if let Err(e) = self.async_tx.send(AsyncRequest::InitTerminal) {
                error!("Failed to send InitTerminal request: {}", e);
                self.stop_loading();
                self.state_manager.show_error(
                    "Initialization Error",
                    format!("Failed to initialize terminal: {}", e),
                    true, // critical
                );
            }
        }

        // Update application state
        self.update_state();

        // Animate loading spinner if active
        if self.state_manager.is_loading() {
            self.state_manager.increment_loading_frame();

            ctx.request_repaint(); // Keep animating
        }

        // Update window title with application state
        self.update_window_title(frame);

        // Set up visual style
        self.setup_visual_style(ctx);

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx, frame);

        // Show loading overlay if active
        if self.state_manager.is_loading() {
            egui::Area::new("loading_overlay")
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgba_premultiplied(30, 30, 40, 240))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(100, 100, 200),
                        ))
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .rounding(egui::Rounding::same(4.0));

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(self.loading_spinner())
                                    .size(20.0)
                                    .color(egui::Color32::from_rgb(100, 200, 255)),
                            );
                            let loading_msg = self.state_manager.loading_message();
                            if !loading_msg.is_empty() {
                                ui.label(
                                    egui::RichText::new(loading_msg)
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(200, 200, 200)),
                                );
                            }
                        });
                    });
                });
        }

        // Show error dialog if present
        if let Some(error) = self.state_manager.error_dialog() {
            let error_clone = error.clone();
            egui::Window::new(&error_clone.title)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.set_min_width(400.0);
                    ui.set_max_width(600.0);

                    // Error icon and message
                    ui.horizontal(|ui| {
                        let icon = if error_clone.is_critical {
                            "⛔"
                        } else {
                            "⚠️"
                        };
                        let icon_color = if error_clone.is_critical {
                            egui::Color32::from_rgb(220, 50, 50)
                        } else {
                            egui::Color32::from_rgb(255, 165, 0)
                        };

                        ui.label(egui::RichText::new(icon).size(32.0).color(icon_color));

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&error_clone.message)
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(220, 220, 220)),
                            );
                        });
                    });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(5.0);

                    // OK button
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(egui::RichText::new("  OK  ").size(14.0))
                            .clicked()
                        {
                            self.state_manager.clear_error();
                        }
                    });
                });
        }

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
                    },
                );

                // HISTORY AREA ABOVE INPUT - Scrollable, with commands stacking from newest to oldest
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), history_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        self.render_command_history_area(ui);
                    },
                );
            });
        });

        // Render context menu if active
        self.render_context_menu(ctx);

        // Render performance metrics panel if visible (Ctrl+Shift+P to toggle)
        if self.metrics_panel.is_visible() {
            let pty_count = if let Ok(pty_manager) = self.pty_manager.try_lock() {
                pty_manager.active_count()
            } else {
                0
            };
            let stats = self.state_manager.statistics();
            // Create a temporary UI for the window
            egui::Area::new(egui::Id::new("metrics_area"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    self.metrics_panel.render(ui, stats, pty_count);
                });
        }

        // Render TUI overlay if active
        if self.tui_overlay.is_active() {
            // Handle input for TUI app BEFORE rendering
            if let Some(input_data) = self.tui_overlay.handle_input(ctx) {
                debug!(
                    "TUI overlay: Sending {} bytes of input to PTY",
                    input_data.len()
                );
                // Send input to PTY
                if let Some(_handle_id) = self.tui_overlay.pty_handle() {
                    if let Ok(mut pty_manager) = self.pty_manager.try_lock() {
                        // Get the actual PTY handle from the terminal
                        if let Some(terminal) = &self.terminal {
                            if let Some(pty_handle) = terminal.pty_handle() {
                                if let Err(e) = executor::block_on(async {
                                    pty_manager.send_input(pty_handle, &input_data).await
                                }) {
                                    warn!("Failed to send input to TUI app: {}", e);
                                }
                            }
                        }
                    } else {
                        warn!("Failed to acquire PTY lock for TUI input");
                    }
                }
            }

            // Render overlay UI
            let overlay_closed = self.tui_overlay.render(ctx);
            if overlay_closed || self.tui_overlay.has_exited() {
                info!("TUI overlay closing");

                // Send Ctrl+C to kill the TUI process and reset terminal
                if let Some(terminal) = &self.terminal {
                    if let Some(pty_handle) = terminal.pty_handle() {
                        if let Ok(mut pty_manager) = self.pty_manager.try_lock() {
                            executor::block_on(async {
                                // Send Ctrl+C (ASCII 3) to terminate the process
                                let _ = pty_manager.send_input(pty_handle, &[3]).await;

                                // Wait a moment for process to die
                                std::thread::sleep(std::time::Duration::from_millis(50));

                                // Reset terminal to dumb mode for regular commands
                                let reset_cmd = "export TERM=dumb\n";
                                let _ = pty_manager
                                    .send_input(pty_handle, reset_cmd.as_bytes())
                                    .await;

                                // Send a reset sequence to clear any weird state
                                let _ = pty_manager.send_input(pty_handle, b"\x1bc").await;
                                // ESC c (reset)
                            });
                            info!("Sent Ctrl+C and reset terminal");
                        }
                    }
                }

                // TUI app exited, mark command block as completed with empty output
                if let Some(cmd) = self.tui_overlay.command() {
                    self.handle_tui_exit(cmd.to_string());
                }

                self.tui_overlay.stop();
            }
        }

        // Handle async operations
        self.handle_async_operations(ctx);

        // Poll for async operation results (non-blocking)
        self.poll_async_results();

        // Only repaint when needed to save CPU
        // Repaint if: command is running, has pending output, user input changed, or TUI overlay active
        let needs_repaint = self.state_manager.last_command_time().is_some()
            || (self
                .terminal
                .as_ref()
                .map(|t| t.has_pending_output())
                .unwrap_or(false))
            || self.completion_popup.is_visible()
            || self.tui_overlay.is_active();

        if needs_repaint {
            // Repaint immediately for active operations
            ctx.request_repaint();

            // For TUI overlay, request very fast repaints (16ms = ~60fps) for smooth updates
            if self.tui_overlay.is_active() {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        } else {
            // Check again in 100ms for idle state (efficient polling)
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
        // Build dynamic title based on terminal state
        let title = if self.state_manager.app_state().terminal_ready {
            let stats = self.state_manager.statistics();
            let cmd_count = stats.total_commands;

            // Show command count and current directory if available
            if let Some(session) = self.state_manager.active_session() {
                let dir_name = session
                    .working_directory
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("~");
                format!("MosaicTerm - {} [{} cmds]", dir_name, cmd_count)
            } else {
                format!("MosaicTerm - Ready [{} cmds]", cmd_count)
            }
        } else {
            "MosaicTerm - Initializing...".to_string()
        };

        // Note: eframe 0.24 doesn't have viewport() method on frame.info()
        // This infrastructure is ready for when the API becomes available

        // Alternative: Try using native window handle if available
        // This is a workaround that may work on some platforms
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::atomic::{AtomicBool, Ordering};
            static TITLE_UPDATE_ATTEMPTED: AtomicBool = AtomicBool::new(false);

            // Only attempt once per second to avoid overhead
            if !TITLE_UPDATE_ATTEMPTED.swap(true, Ordering::Relaxed) {
                // Store title in a static for potential future use
                static CURRENT_TITLE: std::sync::Mutex<Option<String>> =
                    std::sync::Mutex::new(None);

                if let Ok(mut current) = CURRENT_TITLE.lock() {
                    *current = Some(title.clone());
                }

                // Reset the flag after a delay
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    TITLE_UPDATE_ATTEMPTED.store(false, Ordering::Relaxed);
                });
            }
        }

        // Note: As of eframe 0.24, there's no direct API to update window title at runtime.
        // The title is set once at window creation via ViewportBuilder.
        // This implementation prepares the infrastructure for when eframe adds support.
        let _ = title; // Use the variable to avoid unused warning
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
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::monospace(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(11.0));

        // Apply the style
        ctx.set_style(style);
    }

    /// Handle keyboard shortcuts and navigation
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
            }
            ctx.input_mut(|i| i.events.clear());
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
    fn handle_interrupt_command(&mut self) {
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
    fn handle_interrupt_specific_command(&mut self, block_id: String) {
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
    fn handle_clear_screen(&mut self) {
        // Clear command history (visual blocks) but preserve input history for arrow keys
        self.state_manager.clear_command_history();

        info!("Clear screen requested (Ctrl+L)");
        self.set_status_message(Some("Screen cleared".to_string()));
    }

    /// Handle exit (Ctrl+D)
    fn handle_exit(&mut self) {
        // Send EOF to shell (Ctrl+D)
        if let Some(_terminal) = &mut self.terminal {
            std::mem::drop(_terminal.process_input("\x04")); // EOF character
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

    /// Render the fixed input area at the bottom
    fn render_fixed_input_area(&mut self, ui: &mut egui::Ui) {
        // Create a fixed input block with clear visual boundaries
        let input_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(25, 25, 35))
            .stroke(egui::Stroke::new(
                2.0,
                egui::Color32::from_rgb(100, 100, 150),
            ))
            .inner_margin(egui::Margin::symmetric(15.0, 10.0))
            .outer_margin(egui::Margin::symmetric(5.0, 5.0));

        let frame_response = input_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Input prompt label - use the custom prompt from config
                ui.label(
                    egui::RichText::new(self.input_prompt.prompt_text())
                        .font(egui::FontId::monospace(16.0))
                        .color(egui::Color32::from_rgb(100, 200, 100))
                        .strong(),
                );

                // Get current input for display
                let old_input = self.input_prompt.current_input().to_string();
                let mut current_input = old_input.clone();

                // Check for keys BEFORE TextEdit consumes them
                let tab_pressed = ui.input(|i| i.key_pressed(egui::Key::Tab));
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let up_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                let down_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));

                // Input field - take full width
                // We set .lock_focus(true) to prevent Tab from moving focus away
                let input_response = ui.add(
                    egui::TextEdit::singleline(&mut current_input)
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(f32::INFINITY)
                        .hint_text("Type a command and press Enter...")
                        .margin(egui::Vec2::new(8.0, 6.0))
                        .lock_focus(true), // Prevent Tab from changing focus
                );

                // Store input rect for positioning completion popup
                let input_rect = input_response.rect;

                // Always ensure the input keeps focus
                input_response.request_focus();

                // Check if input changed (for filtering)
                let input_changed = old_input != current_input;

                // Update the input prompt with the current input (but avoid resetting if completion was just applied)
                if !self.state_manager.completion_just_applied() {
                    self.input_prompt.set_input(current_input.clone());
                } else {
                    // Completion was just applied, force cursor to end
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), input_response.id)
                    {
                        let ccursor = egui::text::CCursor::new(current_input.len());
                        state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
                        state.store(ui.ctx(), input_response.id);
                    }
                    self.state_manager.set_completion_just_applied(false);
                }

                // Skip keyboard handling if history search is active
                if !self.history_search_active {
                    // Handle keys based on popup state
                    if self.completion_popup.is_visible() {
                        // Popup is open - Tab/arrows navigate, Enter selects, Escape closes
                        if tab_pressed || down_pressed {
                            self.completion_popup.select_next();
                        } else if up_pressed {
                            self.completion_popup.select_previous();
                        } else if escape_pressed {
                            self.completion_popup.hide();
                        } else if input_changed {
                            // Update completions when typing
                            let working_dir = self
                                .terminal
                                .as_ref()
                                .map(|t| t.get_working_directory().to_path_buf())
                                .unwrap_or_else(|| {
                                    std::env::current_dir()
                                        .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                                });
                            if let Ok(result) = self
                                .completion_provider
                                .get_completions(&current_input, &working_dir)
                            {
                                if !result.is_empty() {
                                    let popup_pos =
                                        egui::pos2(input_rect.left(), input_rect.bottom() + 5.0);
                                    self.completion_popup.show(result, popup_pos);
                                } else {
                                    self.completion_popup.hide();
                                }
                            }
                        }
                    } else {
                        // Popup is closed - Tab/Tab opens it
                        if tab_pressed {
                            self.handle_tab_completion(&current_input, input_rect);
                        } else if up_pressed {
                            // Navigate history when popup is closed
                            self.input_prompt.navigate_history_previous();
                        } else if down_pressed {
                            self.input_prompt.navigate_history_next();
                        }
                    }

                    // Handle Enter key to submit command
                    if input_response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        // Check if completion popup is visible
                        if self.completion_popup.is_visible() {
                            // Select completion
                            let selected_text = self
                                .completion_popup
                                .get_selected_item()
                                .map(|item| item.text.clone());
                            if let Some(text) = selected_text {
                                self.apply_completion(&text);
                                self.state_manager.set_completion_just_applied(true);
                                // Flag for cursor positioning
                            }
                            self.completion_popup.hide();
                        } else if !current_input.trim().is_empty() {
                            let command = current_input.clone();
                            self.input_prompt.add_to_history(command.clone());
                            self.input_prompt.clear_input();

                            // Handle the command (non-blocking)
                            let _ = executor::block_on(self.handle_command_input(command));
                        }
                    }
                }

                (input_response, input_rect)
            })
        });

        // Render completion popup if visible (outside the input frame)
        let (_input_response, input_rect) = frame_response.inner.inner;
        if self.completion_popup.is_visible() {
            if let Some(selected_text) = self.completion_popup.render(ui.ctx(), input_rect) {
                // Apply the selected completion
                self.apply_completion(&selected_text);
                self.state_manager.set_completion_just_applied(true); // Flag for cursor positioning
                self.completion_popup.hide();
            }
        }

        // Render history search popup if active
        if self.history_search_active {
            self.render_history_search_popup(ui.ctx(), input_rect);
        }
    }

    /// Handle tab key press for completions
    fn handle_tab_completion(&mut self, input: &str, input_rect: egui::Rect) {
        let now = std::time::Instant::now();

        // Check if this is a double-tab (within 500ms)
        let is_double_tab = self
            .state_manager
            .last_tab_press()
            .map(|last| now.duration_since(last).as_millis() < 500)
            .unwrap_or(false);

        debug!(
            "Tab pressed! Input: '{}', Double-tab: {}",
            input, is_double_tab
        );

        self.state_manager.set_last_tab_press(Some(now));

        // Show completions on double-tab
        if is_double_tab {
            let working_dir = self
                .terminal
                .as_ref()
                .map(|t| t.get_working_directory().to_path_buf())
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
                });

            debug!("Getting completions for '{}' in {:?}", input, working_dir);

            // Get completions
            if let Ok(result) = self
                .completion_provider
                .get_completions(input, &working_dir)
            {
                debug!("Got {} completions", result.len());
                if result.len() == 1 {
                    // Only one match - auto-complete it
                    let completion = &result.suggestions[0];
                    self.apply_completion(&completion.text);
                    self.state_manager.set_completion_just_applied(true);
                    debug!("Auto-completed single match: {}", completion.text);
                } else if !result.is_empty() {
                    // Multiple matches - show popup
                    let popup_pos = egui::pos2(input_rect.left(), input_rect.bottom() + 5.0);
                    self.completion_popup.show(result, popup_pos);
                    debug!("Showing completion popup at {:?}", popup_pos);
                } else {
                    debug!("No completions found");
                }
            } else {
                warn!("Failed to get completions");
            }
        }
        // First tab does nothing - wait for double-tab
    }

    /// Apply a completion to the input
    fn apply_completion(&mut self, completion: &str) {
        let current_input = self.input_prompt.current_input();
        let parts: Vec<&str> = current_input.split_whitespace().collect();

        let new_input = if parts.len() <= 1 {
            // Completing command - add space after
            format!("{} ", completion)
        } else {
            // Completing argument (path) - append to existing path
            let last_part = parts.last().unwrap_or(&"");

            // Find where the last path component starts
            // Handle cases like "cd Desktop/Do" -> "cd Desktop/Documents/"
            let last_slash_pos = last_part.rfind('/');
            let prefix = if let Some(pos) = last_slash_pos {
                &last_part[..=pos]
            } else {
                ""
            };

            // Build the new last part by combining prefix and completion
            let new_last_part = if prefix.is_empty() {
                completion.to_string()
            } else {
                format!("{}{}", prefix, completion)
            };

            // Replace the last part with the completed version
            let mut new_parts = parts[..parts.len() - 1].to_vec();
            new_parts.push(&new_last_part);

            // Don't add space after directories to allow continuing to tab through subdirs
            new_parts.join(" ")
        };

        self.input_prompt.set_input(new_input);
    }

    /// Render the history search popup (Ctrl+R)
    fn render_history_search_popup(&mut self, ctx: &egui::Context, input_rect: egui::Rect) {
        // Position popup above the input
        let popup_width = input_rect.width().max(600.0);
        let popup_height = 400.0;
        let popup_x = input_rect.left();
        let popup_y = input_rect.top() - popup_height - 10.0;

        // Create popup above input
        egui::Area::new("history_search")
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_rgb(30, 30, 40))
                    .stroke(egui::Stroke::new(
                        2.0,
                        egui::Color32::from_rgb(100, 150, 255),
                    ))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.set_height(popup_height);

                        ui.vertical(|ui| {
                            // Title
                            ui.horizontal(|ui| {
                                ui.heading(
                                    egui::RichText::new("🔍 Search Command History (Ctrl+R)")
                                        .color(egui::Color32::from_rgb(150, 200, 255)),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                "↑↓ navigate • Enter to select • Esc to close",
                                            )
                                            .color(egui::Color32::GRAY)
                                            .size(11.0),
                                        );
                                    },
                                );
                            });

                            ui.separator();
                            ui.add_space(8.0);

                            // Search input - always request focus
                            let search_id = egui::Id::new("history_search_input");
                            let search_response = ui.add(
                                egui::TextEdit::singleline(&mut self.history_search_query)
                                    .hint_text("Type to search... (fuzzy matching)")
                                    .font(egui::FontId::monospace(14.0))
                                    .desired_width(f32::INFINITY)
                                    .id(search_id),
                            );

                            // Force focus on the search input
                            ui.memory_mut(|mem| mem.request_focus(search_id));
                            search_response.request_focus();

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Search results
                            let results = self.history_manager.search(&self.history_search_query);

                            // Handle arrow key navigation (check if search field is focused)
                            let selected_idx = self.state_manager.get_history_search_selected();
                            let search_has_focus = ui.memory(|mem| mem.focus() == Some(search_id));

                            if search_has_focus || search_response.has_focus() {
                                if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                                    let new_selected = if results.is_empty() {
                                        0
                                    } else {
                                        (selected_idx + 1).min(results.len() - 1)
                                    };
                                    self.state_manager.set_history_search_selected(new_selected);
                                    ctx.input_mut(|i| i.events.clear()); // Consume event
                                } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                                    let new_selected = selected_idx.saturating_sub(1);
                                    self.state_manager.set_history_search_selected(new_selected);
                                    ctx.input_mut(|i| i.events.clear()); // Consume event
                                } else if ctx.input(|i| i.key_pressed(egui::Key::Enter))
                                    && !results.is_empty()
                                {
                                    // Select the highlighted command
                                    if let Some(command) = results.get(selected_idx) {
                                        self.input_prompt.set_input(command.clone());
                                        self.history_search_active = false;
                                        self.state_manager.set_history_search_selected(0);
                                        ctx.input_mut(|i| i.events.clear()); // Consume event
                                    }
                                }
                            }

                            egui::ScrollArea::vertical()
                                .max_height(280.0)
                                .show(ui, |ui| {
                                    if results.is_empty() {
                                        ui.label(
                                            egui::RichText::new("No matching commands found")
                                                .color(egui::Color32::GRAY)
                                                .italics(),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} commands found",
                                                results.len()
                                            ))
                                            .color(egui::Color32::GRAY)
                                            .size(11.0),
                                        );
                                        ui.add_space(4.0);

                                        for (idx, command) in results.iter().enumerate().take(50) {
                                            let is_selected = idx == selected_idx;
                                            let response = ui.add(
                                                egui::Button::new(
                                                    egui::RichText::new(command)
                                                        .font(egui::FontId::monospace(13.0))
                                                        .color(if is_selected {
                                                            egui::Color32::from_rgb(255, 255, 255)
                                                        } else {
                                                            egui::Color32::from_rgb(200, 200, 220)
                                                        }),
                                                )
                                                .fill(if is_selected {
                                                    egui::Color32::from_rgb(60, 100, 180)
                                                } else if idx % 2 == 0 {
                                                    egui::Color32::from_rgb(25, 25, 35)
                                                } else {
                                                    egui::Color32::from_rgb(20, 20, 30)
                                                })
                                                .frame(false)
                                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                                            );

                                            if response.clicked() {
                                                // Apply the selected command to input
                                                self.input_prompt.set_input(command.clone());
                                                self.history_search_active = false;
                                                self.state_manager.set_history_search_selected(0);
                                            }

                                            if response.hovered() {
                                                self.state_manager.set_history_search_selected(idx);
                                            }
                                        }
                                    }
                                });
                        });
                    });
            });

        // Handle Escape to close
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.history_search_active = false;
            self.state_manager.set_history_search_selected(0);
        }
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
                    ui.label(
                        egui::RichText::new("MosaicTerm")
                            .font(egui::FontId::proportional(16.0))
                            .color(egui::Color32::from_rgb(200, 200, 255))
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Ready")
                                .font(egui::FontId::proportional(12.0))
                                .color(egui::Color32::from_rgb(150, 255, 150)),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!(
                                "History: {}",
                                self.state_manager.get_command_history().len()
                            ))
                            .font(egui::FontId::monospace(12.0))
                            .color(egui::Color32::from_rgb(200, 200, 200)),
                        );
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
                        let command_history = self.state_manager.get_command_history();
                        for (i, block) in command_history.iter().enumerate() {
                            if let Some((block_id, pos)) =
                                Self::render_single_command_block_static(ui, block, i)
                            {
                                // Right-click detected, show context menu
                                self.command_blocks
                                    .interaction_state_mut()
                                    .context_menu_block = Some(block_id);
                                self.command_blocks.interaction_state_mut().context_menu_pos =
                                    Some(pos);
                            }
                        }

                        // If no commands, show welcome message
                        if command_history.is_empty() {
                            ui.add_space(50.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("🎉 Welcome to MosaicTerm!")
                                        .font(egui::FontId::proportional(24.0))
                                        .color(egui::Color32::from_rgb(255, 200, 100))
                                        .strong(),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new("Type a command in the input area below")
                                        .font(egui::FontId::proportional(16.0))
                                        .color(egui::Color32::from_rgb(200, 200, 255)),
                                );
                            });
                        }
                    });
                });
        });
    }

    /// Render a single command block (static version to avoid borrow checker issues)
    fn render_single_command_block_static(
        ui: &mut egui::Ui,
        block: &CommandBlock,
        _index: usize,
    ) -> Option<(String, egui::Pos2)> {
        let block_frame = egui::Frame::none()
            .fill(egui::Color32::from_rgb(45, 45, 55))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 100)))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .outer_margin(egui::Margin::symmetric(0.0, 4.0));

        let frame_response = block_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                // Command header with timestamp and status
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&block.command)
                            .font(egui::FontId::monospace(14.0))
                            .color(egui::Color32::from_rgb(200, 200, 255)),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        let (status_text, status_color) = match block.status {
                            ExecutionStatus::Running => {
                                ("Running", egui::Color32::from_rgb(255, 200, 0))
                            }
                            ExecutionStatus::Completed => {
                                ("Completed", egui::Color32::from_rgb(0, 255, 100))
                            }
                            ExecutionStatus::Failed => {
                                ("Failed", egui::Color32::from_rgb(255, 100, 100))
                            }
                            ExecutionStatus::Cancelled => {
                                ("Cancelled", egui::Color32::from_rgb(255, 165, 0))
                            }
                            ExecutionStatus::Pending => {
                                ("Pending", egui::Color32::from_rgb(150, 150, 150))
                            }
                            ExecutionStatus::TuiMode => {
                                ("TUI Mode", egui::Color32::from_rgb(150, 100, 255))
                            }
                        };

                        ui.label(
                            egui::RichText::new(status_text)
                                .font(egui::FontId::proportional(12.0))
                                .color(status_color),
                        );
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
                        for line in block.output.iter() {
                            // Show all output lines with ANSI color support
                            if !line.ansi_codes.is_empty() {
                                // Use ANSI text renderer for colored text
                                let mut ansi_renderer =
                                    mosaicterm::ui::text::AnsiTextRenderer::new();
                                if let Err(e) =
                                    ansi_renderer.render_ansi_text(ui, &line.text, &line.ansi_codes)
                                {
                                    // Fallback to plain text if ANSI rendering fails
                                    debug!(
                                        "ANSI rendering failed: {}, falling back to plain text",
                                        e
                                    );
                                    ui.label(
                                        egui::RichText::new(&line.text)
                                            .font(egui::FontId::monospace(12.0))
                                            .color(egui::Color32::from_rgb(180, 180, 200)),
                                    );
                                }
                            } else {
                                // Plain text without ANSI codes
                                ui.label(
                                    egui::RichText::new(&line.text)
                                        .font(egui::FontId::monospace(12.0))
                                        .color(egui::Color32::from_rgb(180, 180, 200)),
                                );
                            }
                        }
                    });
                }

                // Timestamp
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", block.timestamp.format("%H:%M:%S")))
                            .font(egui::FontId::monospace(10.0))
                            .color(egui::Color32::from_rgb(120, 120, 140)),
                    );
                });
            });
        });

        // Check if mouse is over this block and right-click was pressed
        if frame_response.response.hovered() && ui.input(|i| i.pointer.secondary_clicked()) {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                return Some((block.id.clone(), pos));
            }
        }

        None
    }

    /// Poll for async operation results (non-blocking)
    fn poll_async_results(&mut self) {
        // Try to receive all pending results without blocking
        while let Ok(result) = self.async_rx.try_recv() {
            match result {
                AsyncResult::TerminalInitialized => {
                    info!("Terminal initialized successfully (async)");
                    // We need to actually create the terminal in this thread
                    // For now, we'll call the blocking initialize
                    match executor::block_on(self.initialize_terminal()) {
                        Ok(()) => {
                            self.stop_loading();
                            // terminal_ready is already set in initialize_terminal()
                        }
                        Err(e) => {
                            error!("Failed to finalize terminal init: {}", e);
                            self.stop_loading();
                            let user_msg = self.user_friendly_error(&e);
                            self.set_status_message(Some(user_msg));
                        }
                    }
                }
                AsyncResult::TerminalInitFailed(msg) => {
                    error!("Terminal initialization failed: {}", msg);
                    self.stop_loading();
                    self.state_manager.show_error(
                        "Terminal Initialization Failed",
                        format!("Failed to initialize terminal: {}\n\nPlease check your shell configuration and try restarting the application.", msg),
                        true, // critical
                    );
                }
                AsyncResult::PtyRestarted => {
                    info!("PTY restarted successfully");
                    self.stop_loading();
                    // Reinitialize terminal
                    match executor::block_on(self.initialize_terminal()) {
                        Ok(()) => {
                            self.set_status_message(Some("Shell session restarted".to_string()));
                        }
                        Err(e) => {
                            error!("Failed to reinitialize after restart: {}", e);
                            let user_msg = self.user_friendly_error(&e);
                            self.set_status_message(Some(user_msg));
                        }
                    }
                }
                AsyncResult::PtyRestartFailed(msg) => {
                    error!("PTY restart failed: {}", msg);
                    self.stop_loading();
                    self.state_manager.show_error(
                        "Restart Failed",
                        format!("Failed to restart shell session: {}\n\nYou may need to restart the application.", msg),
                        false, // not critical - can continue using old session
                    );
                }
                AsyncResult::InterruptSent => {
                    info!("Interrupt signal sent successfully");
                    self.set_status_message(Some("Process interrupted".to_string()));
                }
                AsyncResult::InterruptFailed(msg) => {
                    warn!("Interrupt signal failed: {}", msg);
                    self.state_manager.show_error(
                        "Interrupt Failed",
                        format!("Failed to interrupt the running command: {}\n\nThe process may still be running.", msg),
                        false, // not critical
                    );
                }
                _ => {
                    // Other results not yet implemented
                    debug!("Received unhandled async result");
                }
            }
        }
    }

    /// Handle async operations (called from update) - SIMPLIFIED VERSION
    fn handle_async_operations(&mut self, _ctx: &egui::Context) {
        // SIMPLIFIED: Poll PTY output and add to current command (no complex prompt detection)
        let mut should_update_contexts = false; // Track if we need to update contexts
        let mut env_query_output = None; // Track environment query output
        let mut timeout_kill_status_message: Option<String> = None; // Track timeout kill status

        if let Some(_terminal) = &mut self.terminal {
            if let Some(handle) = _terminal.pty_handle() {
                // Use blocking lock with retry to avoid dropping output
                // This is a temporary fix until we move to async channels (TASK-007)
                let mut pty_manager_opt = None;
                for attempt in 0..3 {
                    match self.pty_manager.try_lock() {
                        Ok(guard) => {
                            pty_manager_opt = Some(guard);
                            break;
                        }
                        Err(_) if attempt < 2 => {
                            // Brief wait and retry
                            std::thread::sleep(std::time::Duration::from_micros(100));
                            continue;
                        }
                        Err(_) => {
                            warn!("Failed to acquire PTY lock after 3 attempts - output may be delayed");
                            return;
                        }
                    }
                }

                if let Some(mut pty_manager) = pty_manager_opt {
                    if let Ok(data) = pty_manager.try_read_output_now(handle) {
                        if !data.is_empty() {
                            // If TUI overlay is active, send RAW output there (don't process it!)
                            if self.tui_overlay.is_active() {
                                debug!("Routing {} bytes to TUI overlay", data.len());

                                // Send raw bytes directly to overlay - TUI apps need ANSI codes!
                                self.tui_overlay.add_raw_output(&data);

                                // Check if output contains shell prompt (indicating TUI app exited)
                                let data_str = String::from_utf8_lossy(&data);
                                let has_prompt = data_str.ends_with("$ ")
                                    || data_str.ends_with("% ")
                                    || data_str.ends_with("> ")
                                    || data_str.contains("@")
                                        && (data_str.contains("$") || data_str.contains("%"));

                                if has_prompt {
                                    debug!("Detected prompt in TUI output, marking as exited");
                                    self.tui_overlay.mark_exited();
                                }

                                return; // Don't process output for command blocks
                            }

                            // Check for ANSI clear screen sequences (\x1b[H\x1b[2J or \x1b[2J)
                            let data_str = String::from_utf8_lossy(&data);
                            let is_clear_sequence = data_str.contains("\x1b[H\x1b[2J")
                                || data_str.contains("\x1b[2J")
                                || (data_str.contains("\x1b[H") && data_str.len() < 20); // Short sequences with just cursor home

                            if is_clear_sequence {
                                info!(
                                    "Detected ANSI clear screen sequence, clearing command history"
                                );
                                self.state_manager.clear_command_history();
                                // Skip processing this output as it's just a clear command
                            } else {
                                // Process output
                                debug!(
                                    "PTY read {} bytes: {:?}",
                                    data.len(),
                                    String::from_utf8_lossy(&data)
                                );
                                // Process output and get the lines directly from the return value
                                let ready_lines = futures::executor::block_on(async {
                                    _terminal
                                        .process_output(
                                            &data,
                                            mosaicterm::terminal::StreamType::Stdout,
                                        )
                                        .await
                                        .unwrap_or_else(|e| {
                                            debug!("Error processing output: {}", e);
                                            Vec::new()
                                        })
                                });

                                debug!("Got {} lines from terminal", ready_lines.len());

                                // Get the last command block from state_manager (single source of truth)
                                // Get last_command_time before mutable borrow
                                let last_command_time = self.state_manager.last_command_time();
                                let mut lines_count = 0;
                                let mut should_clear_command_time = false;
                                if let Some(command_history) =
                                    self.state_manager.command_history_mut()
                                {
                                    if let Some(last_block) = command_history.last_mut() {
                                        // Skip output processing if this command is in TUI mode
                                        if last_block.status
                                            == mosaicterm::models::ExecutionStatus::TuiMode
                                        {
                                            debug!("Skipping output for TUI mode command");
                                            return;
                                        }

                                        let has_ready_lines = !ready_lines.is_empty();

                                        // Batch process all lines at once for better performance
                                        let mut lines_to_add =
                                            Vec::with_capacity(ready_lines.len());
                                        // Clone command text once for use in multiple places
                                        let command_text = last_block.command.trim().to_string();
                                        let current_output_count = last_block.output.len();

                                        // Check for environment query marker (state persists across batches)
                                        info!("Processing {} ready_lines for env query (in_progress={})", ready_lines.len(), self.env_query_in_progress);
                                        for (idx, line) in ready_lines.iter().enumerate() {
                                            let line_text = line.text.trim();
                                            if line_text.contains("MOSAICTERM_ENV_QUERY:START") {
                                                info!(
                                                    "Found ENV_QUERY:START marker at line {}",
                                                    idx
                                                );
                                                self.env_query_in_progress = true;
                                                self.env_query_lines.clear(); // Start fresh
                                                continue;
                                            }
                                            if line_text.contains("MOSAICTERM_ENV_QUERY:END") {
                                                info!("Found ENV_QUERY:END marker at line {} (collected {} lines total)", idx, self.env_query_lines.len());
                                                self.env_query_in_progress = false;
                                                // Store for processing after borrow ends
                                                if !self.env_query_lines.is_empty() {
                                                    env_query_output =
                                                        Some(self.env_query_lines.join("\n"));
                                                }
                                                continue;
                                            }
                                            if self.env_query_in_progress {
                                                info!(
                                                    "Collecting env line {}: '{}'",
                                                    idx, line_text
                                                );
                                                self.env_query_lines.push(line_text.to_string());
                                            } else if !line_text.is_empty() {
                                                info!(
                                                    "Skipping non-env line {}: '{}'",
                                                    idx, line_text
                                                );
                                            }
                                        }

                                        // Check for exit code marker
                                        let mut found_exit_code: Option<i32> = None;
                                        for line in ready_lines.iter() {
                                            let line_text = line.text.trim();
                                            if line_text.starts_with("MOSAICTERM_EXITCODE:") {
                                                if let Some(exit_code_str) =
                                                    line_text.strip_prefix("MOSAICTERM_EXITCODE:")
                                                {
                                                    if let Ok(exit_code) =
                                                        exit_code_str.trim().parse::<i32>()
                                                    {
                                                        found_exit_code = Some(exit_code);
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        // Apply exit code if found
                                        if let Some(exit_code) = found_exit_code {
                                            let elapsed_ms =
                                                if let Some(start_time) = last_command_time {
                                                    start_time.elapsed().as_millis()
                                                } else {
                                                    0
                                                };

                                            if exit_code == 0 {
                                                last_block.mark_completed(
                                                    std::time::Duration::from_millis(
                                                        elapsed_ms.try_into().unwrap_or(1000),
                                                    ),
                                                );
                                                debug!(
                                                    "✅ Command completed with exit code 0: {}",
                                                    command_text
                                                );
                                            } else {
                                                last_block.mark_failed(
                                                    std::time::Duration::from_millis(
                                                        elapsed_ms.try_into().unwrap_or(1000),
                                                    ),
                                                    exit_code,
                                                );
                                                debug!(
                                                    "❌ Command failed with exit code {}: {}",
                                                    exit_code, command_text
                                                );
                                            }
                                            should_clear_command_time = true;
                                        }

                                        for (idx, line) in ready_lines.iter().enumerate() {
                                            let line_text = line.text.trim();
                                            let is_first_few_lines = current_output_count + idx < 5;

                                            // Skip exit code marker, env query markers, and the echo commands
                                            if line_text.starts_with("MOSAICTERM_EXITCODE:")
                                                || line_text.contains("echo \"MOSAICTERM_EXITCODE:")
                                                || line_text == "echo \"MOSAICTERM_EXITCODE:$?\""
                                                || line_text.contains("MOSAICTERM_ENV_QUERY")
                                                || line_text.starts_with("echo 'MOSAICTERM_ENV_QUERY")
                                                || line_text.starts_with("echo \"VIRTUAL_ENV=")
                                                || line_text.starts_with("echo \"CONDA_DEFAULT_ENV=")
                                                || line_text.starts_with("echo \"NVM_BIN=")
                                                || line_text.starts_with("echo \"RBENV_VERSION=")
                                                // Also skip the OUTPUT of those echo commands
                                                || line_text.starts_with("VIRTUAL_ENV=")
                                                || line_text.starts_with("CONDA_DEFAULT_ENV=")
                                                || line_text.starts_with("NVM_BIN=")
                                                || line_text.starts_with("RBENV_VERSION=")
                                            {
                                                continue;
                                            }

                                            // Skip if:
                                            // 1. Exact match of command
                                            // 2. First few lines and is a prefix of command (partial echo)
                                            // 3. Contains "^C" (Ctrl+C echo from shell) or is empty
                                            let should_skip = line_text == command_text
                                                || (is_first_few_lines
                                                    && !line_text.is_empty()
                                                    && command_text.starts_with(line_text)
                                                    && line_text.len() <= 3) // Only skip very short prefixes
                                                || line_text.contains("^C") // Skip any line with Ctrl+C echo
                                                || line_text.is_empty(); // Skip empty lines

                                            if !should_skip {
                                                // Truncate line if too long
                                                let mut line_to_add = line.clone();
                                                if line_to_add.text.len() > MAX_LINE_LENGTH {
                                                    line_to_add.text.truncate(MAX_LINE_LENGTH);
                                                    line_to_add.text.push_str("... [truncated]");
                                                }
                                                lines_to_add.push(line_to_add);
                                            }
                                        }

                                        // Add all lines at once (batch operation)
                                        if !lines_to_add.is_empty() {
                                            last_block.add_output_lines(lines_to_add.clone());
                                            lines_count = lines_to_add.len();
                                        }

                                        // Check if we need to truncate old lines (after batch add)
                                        if last_block.output.len() > MAX_OUTPUT_LINES_PER_COMMAND {
                                            let to_remove = last_block.output.len()
                                                - MAX_OUTPUT_LINES_PER_COMMAND
                                                + 1000;
                                            last_block.output.drain(0..to_remove);
                                            // Add truncation notice at the beginning
                                            let truncation_notice =
                                                mosaicterm::models::OutputLine {
                                                    text: format!(
                                                "... [truncated {} lines due to size limit] ...",
                                                to_remove
                                            ),
                                                    ansi_codes: vec![],
                                                    line_number: 0,
                                                    timestamp: chrono::Utc::now(),
                                                };
                                            last_block.output.insert(0, truncation_notice);
                                            warn!(
                                            "Truncated {} lines from command output (limit: {})",
                                            to_remove, MAX_OUTPUT_LINES_PER_COMMAND
                                        );
                                        }

                                        // Also handle partial data (no newlines) as immediate output
                                        if !has_ready_lines {
                                            let text = String::from_utf8_lossy(&data);
                                            debug!("Processing partial data: '{}'", text.trim());

                                            let trimmed_text = text.trim();

                                            // Check if this looks like a shell prompt (for completion detection)
                                            let looks_like_prompt = !trimmed_text.is_empty()
                                                && (trimmed_text.ends_with("$ ")
                                                    || trimmed_text.ends_with("% ")
                                                    || trimmed_text.ends_with("> ")
                                                    || trimmed_text.ends_with("$")
                                                    || trimmed_text.ends_with("%")
                                                    || trimmed_text.ends_with(">")
                                                    || (trimmed_text.contains("@")
                                                        && (trimmed_text.contains("$")
                                                            || trimmed_text.contains("%"))));

                                            // If this looks like a prompt, check if it's AFTER command output (indicating completion)
                                            if looks_like_prompt && !last_block.output.is_empty() {
                                                debug!(
                                                    "🎯 Detected shell prompt after output: '{}'",
                                                    trimmed_text
                                                );

                                                // Parse ANSI codes and add as output line for completion detection
                                                let mut ansi_parser =
                                                mosaicterm::terminal::ansi_parser::AnsiParser::new();
                                                let parsed_result =
                                                ansi_parser.parse(trimmed_text).unwrap_or_else(|_| {
                                                    mosaicterm::terminal::ansi_parser::ParsedText::new(
                                                        trimmed_text.to_string(),
                                                    )
                                                });

                                                let clean_text = parsed_result.clean_text.clone();

                                                // Add the prompt as an output line
                                                let prompt_line = mosaicterm::models::OutputLine {
                                                    text: parsed_result.clean_text,
                                                    ansi_codes: parsed_result.ansi_codes,
                                                    line_number: 0,
                                                    timestamp: chrono::Utc::now(),
                                                };
                                                last_block.add_output_line(prompt_line);
                                                debug!(
                                                "Added prompt line for completion detection: '{}'",
                                                clean_text
                                            );
                                            } else if !trimmed_text.is_empty()
                                                && trimmed_text != last_block.command.trim()
                                                && !looks_like_prompt
                                            {
                                                // Regular output (not command echo, not prompt)
                                                // Parse ANSI codes from the text before adding to output
                                                let mut ansi_parser =
                                                mosaicterm::terminal::ansi_parser::AnsiParser::new();
                                                let parsed_result =
                                                ansi_parser.parse(trimmed_text).unwrap_or_else(|_| {
                                                    mosaicterm::terminal::ansi_parser::ParsedText::new(
                                                        trimmed_text.to_string(),
                                                    )
                                                });

                                                let clean_text = parsed_result.clean_text.clone();

                                                // Create output line with clean text and parsed ANSI codes
                                                let mut partial_line =
                                                    mosaicterm::models::OutputLine {
                                                        text: parsed_result.clean_text,
                                                        ansi_codes: parsed_result.ansi_codes,
                                                        line_number: 0,
                                                        timestamp: chrono::Utc::now(),
                                                    };

                                                // Truncate if too long
                                                if partial_line.text.len() > MAX_LINE_LENGTH {
                                                    partial_line.text.truncate(MAX_LINE_LENGTH);
                                                    partial_line.text.push_str("... [truncated]");
                                                }

                                                last_block.add_output_line(partial_line);
                                                debug!(
                                                "Added partial data as output line (clean): '{}'",
                                                clean_text
                                            );

                                                // Check if we need to truncate old lines (same check as above)
                                                if last_block.output.len()
                                                    > MAX_OUTPUT_LINES_PER_COMMAND
                                                {
                                                    let to_remove = last_block.output.len()
                                                        - MAX_OUTPUT_LINES_PER_COMMAND
                                                        + 1000;
                                                    last_block.output.drain(0..to_remove);
                                                    let truncation_notice = mosaicterm::models::OutputLine {
                                                    text: format!("... [truncated {} lines due to size limit] ...", to_remove),
                                                    ansi_codes: vec![],
                                                    line_number: 0,
                                                    timestamp: chrono::Utc::now(),
                                                };
                                                    last_block.output.insert(0, truncation_notice);
                                                }
                                            }
                                        }

                                        // Prompt-based completion detection using existing CommandCompletionDetector
                                        if let Some(start_time) = last_command_time {
                                            let elapsed_ms = start_time.elapsed().as_millis();
                                            let elapsed_secs = (elapsed_ms / 1000) as u64;

                                            // Clone the necessary data to avoid borrowing conflicts
                                            let output_lines_clone: Vec<_> =
                                                last_block.output.clone();
                                            let command_clone = last_block.command.clone();

                                            // Use the terminal's completion detector to check if command is done
                                            let is_complete = if !output_lines_clone.is_empty() {
                                                // Check if the latest output contains a shell prompt indicating completion
                                                let recent_lines = if output_lines_clone.len() > 3 {
                                                    &output_lines_clone
                                                        [output_lines_clone.len() - 3..]
                                                } else {
                                                    &output_lines_clone
                                                };

                                                // Create a completion detector and check for command completion
                                                // The detector will work on the clean text (without ANSI codes)
                                                let completion_detector = mosaicterm::terminal::prompt::CommandCompletionDetector::new();
                                                let is_complete = completion_detector
                                                    .is_command_complete(recent_lines);

                                                // Debug: Log what we're checking for completion
                                                if !is_complete && elapsed_ms > 500 {
                                                    // Only log after some time to avoid spam
                                                    debug!("🔍 Checking completion for command '{}' ({}ms elapsed)", command_clone, elapsed_ms);
                                                    for (i, line) in recent_lines.iter().enumerate()
                                                    {
                                                        debug!(
                                                            "  Line {}: '{}'",
                                                            i,
                                                            line.text.replace('\n', "\\n")
                                                        );
                                                    }
                                                } else if is_complete {
                                                    debug!(
                                                    "✅ Prompt detection found completion for: {}",
                                                    command_clone
                                                );
                                                    for (i, line) in recent_lines.iter().enumerate()
                                                    {
                                                        debug!(
                                                            "  Completion line {}: '{}'",
                                                            i, line.text
                                                        );
                                                    }
                                                }

                                                is_complete
                                            } else {
                                                false
                                            };

                                            // Check if command might be interactive/long-running (for fallback timeout)
                                            let is_interactive_command =
                                                last_block.command.contains("top")
                                                    || last_block.command.contains("htop")
                                                    || last_block.command.contains("vim")
                                                    || last_block.command.contains("nano")
                                                    || last_block.command.contains("less")
                                                    || last_block.command.contains("man")
                                                    || last_block.command.starts_with("ssh ")
                                                    || last_block.command.contains(" | ")
                                                    || last_block.command.contains(" > ")
                                                    || last_block.command.contains(" >> ");

                                            // Get timeout configuration
                                            let timeout_config =
                                                &self.runtime_config.config().terminal.timeout;
                                            let timeout_secs = if is_interactive_command {
                                                timeout_config.interactive_command_timeout_secs
                                            } else {
                                                timeout_config.regular_command_timeout_secs
                                            };

                                            if is_complete {
                                                // Command completed based on prompt detection - mark as completed
                                                last_block.mark_completed(
                                                    std::time::Duration::from_millis(
                                                        elapsed_ms.try_into().unwrap_or(1000),
                                                    ),
                                                );
                                                should_clear_command_time = true;
                                                debug!(
                                            "✅ Command completed based on prompt detection: {}",
                                            last_block.command
                                        );
                                            } else if timeout_secs > 0
                                                && elapsed_secs >= timeout_secs
                                            {
                                                // Command has exceeded configured timeout
                                                warn!(
                                                    "⏰ Command exceeded timeout of {}s: {}",
                                                    timeout_secs, command_clone
                                                );

                                                // Add timeout notice to output
                                                let timeout_notice =
                                                    mosaicterm::models::OutputLine {
                                                        text: format!(
                                                "\n[Timeout: Command exceeded {}s limit]",
                                                timeout_secs
                                            ),
                                                        ansi_codes: vec![],
                                                        line_number: 0,
                                                        timestamp: chrono::Utc::now(),
                                                    };
                                                last_block.output.push(timeout_notice);

                                                // Mark as completed (with timeout flag)
                                                last_block.mark_completed(
                                                    std::time::Duration::from_secs(timeout_secs),
                                                );
                                                should_clear_command_time = true;

                                                // If kill_on_timeout is true, send kill signal to PTY process
                                                if timeout_config.kill_on_timeout {
                                                    info!(
                                                        "Killing timed-out command: {}",
                                                        command_clone
                                                    );
                                                    let kill_result = if let Some(terminal) =
                                                        &self.terminal
                                                    {
                                                        if let Some(handle) = terminal.pty_handle()
                                                        {
                                                            let handle_id = handle.id.clone();
                                                            // Send kill signal asynchronously
                                                            self.async_tx
                                                                .send(AsyncRequest::SendInterrupt(
                                                                    handle_id.clone(),
                                                                ))
                                                                .map_err(|e| e.to_string())
                                                        } else {
                                                            Err("No PTY handle available"
                                                                .to_string())
                                                        }
                                                    } else {
                                                        Err("No terminal available".to_string())
                                                    };

                                                    match kill_result {
                                                        Ok(_) => {
                                                            info!("Kill signal sent for timed-out command");
                                                            timeout_kill_status_message = Some(format!(
                                                                "Command timed out and was killed after {}s",
                                                                timeout_secs
                                                            ));
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to send kill request for timeout: {}", e);
                                                            timeout_kill_status_message = Some(format!(
                                                                "Failed to kill timed-out command: {}",
                                                                e
                                                            ));
                                                        }
                                                    }
                                                }
                                            } else if !is_interactive_command
                                                && elapsed_ms > 1000
                                                && !output_lines_clone.is_empty()
                                            {
                                                // For regular commands, use simple completion detection as fallback
                                                // Check if the last line looks like it could be a prompt or completion
                                                if let Some(last_line) = output_lines_clone.last() {
                                                    let text = last_line.text.trim();
                                                    // Simple heuristics for completion detection
                                                    let looks_complete = text.is_empty() ||  // Empty line often indicates completion
                                                text.ends_with("$") ||              // Shell prompt ending
                                                text.ends_with("%") ||              // Zsh prompt ending  
                                                text.ends_with(">") ||              // Fish/PowerShell prompt ending
                                                text.contains("@") && (text.contains("$") || text.contains("%")); // user@host$ pattern

                                                    if looks_complete {
                                                        last_block.mark_completed(
                                                            std::time::Duration::from_millis(
                                                                elapsed_ms
                                                                    .try_into()
                                                                    .unwrap_or(1000),
                                                            ),
                                                        );
                                                        should_clear_command_time = true;
                                                        debug!("✅ Command completed based on simple heuristics: {} (last line: '{}')", command_clone, text);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Update statistics after borrow ends
                                if lines_count > 0 {
                                    self.state_manager.statistics_mut().total_output_lines +=
                                        lines_count;
                                }

                                // Clear command time if needed (after borrow ends)
                                if should_clear_command_time {
                                    self.state_manager.clear_last_command_time();
                                    should_update_contexts = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Set timeout kill status message after all borrows are released
        if let Some(msg) = timeout_kill_status_message {
            self.set_status_message(Some(msg));
        }

        // Update contexts after pty_manager borrow is released
        if should_update_contexts {
            self.update_contexts();

            // Also trigger environment query
            if let Some(terminal) = &self.terminal {
                if let Some(handle) = terminal.pty_handle() {
                    let marker = "MOSAICTERM_ENV_QUERY";
                    // Print each variable explicitly with its name, even if empty
                    let query_cmd = format!(
                        "echo '{}:START'; echo \"VIRTUAL_ENV=${{VIRTUAL_ENV}}\"; echo \"CONDA_DEFAULT_ENV=${{CONDA_DEFAULT_ENV}}\"; echo \"NVM_BIN=${{NVM_BIN}}\"; echo \"RBENV_VERSION=${{RBENV_VERSION}}\"; echo '{}:END'\n",
                        marker, marker
                    );

                    info!("Sending environment query command");
                    // Try to send query (non-blocking)
                    if let Ok(mut pty_manager) = self.pty_manager.try_lock() {
                        match futures::executor::block_on(async {
                            pty_manager.send_input(handle, query_cmd.as_bytes()).await
                        }) {
                            Ok(_) => info!("Successfully sent environment query"),
                            Err(e) => warn!("Failed to send environment query: {}", e),
                        }
                    } else {
                        warn!("Could not acquire pty_manager lock for environment query");
                    }
                }
            }

            self.update_prompt();
        }

        // Parse environment output if we received one
        if let Some(env_output) = env_query_output {
            self.parse_env_output(&env_output);
            self.update_prompt();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = MosaicTermApp::new();
        assert!(app.terminal.is_none()); // Terminal starts as None
        assert!(!app.state_manager.is_terminal_ready());
    }

    #[test]
    fn test_app_state() {
        // AppState is deprecated - state is now managed through StateManager
        let app = MosaicTermApp::new();
        assert_eq!(app.state_manager.app_state().status_message, None);
    }

    #[test]
    fn test_status_message() {
        let mut app = MosaicTermApp::new();
        app.set_status_message(Some("Test message".to_string()));
        assert_eq!(
            app.state_manager.app_state().status_message,
            Some("Test message".to_string())
        );

        app.set_status_message(None);
        assert!(app.state_manager.app_state().status_message.is_none());
    }
}
