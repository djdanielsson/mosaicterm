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
//! ## Module Organization
//!
//! - `mod.rs` - Core application struct, eframe::App impl, UI rendering, PTY polling
//! - `async_ops.rs` - Background async task loop for terminal init, direct execution
//! - `commands.rs` - Command detection and classification (TUI, cd, interactive, exit)
//! - `context.rs` - Environment context detection (venv, conda, nvm) and git info
//! - `input.rs` - Keyboard shortcuts and input handling
//! - `prompt.rs` - Prompt building with contexts and SSH support
//! - `ssh.rs` - SSH session detection, remote prompt parsing, session lifecycle
//!
//! ### Main Components
//!
//! - `MosaicTermApp`: Core application state and lifecycle
//! - `handle_async_operations()`: Processes PTY output in the update loop
//! - `poll_async_results()`: Receives results from background async tasks
//! - `render_*()`: UI rendering methods for input, history, popups
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
//! - **Size Limits:** Enforces max lines per command (10K) and chars per line (10K)
//! - **Async I/O:** Background task handles terminal init and direct command execution

// Submodules
mod async_ops;
mod commands;
mod context;
mod input;
#[allow(dead_code)]
pub mod pane_tree;
mod prompt;
mod ssh;

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
use tracing::{debug, error, info, warn};

// Output size limits to prevent memory leaks
const MAX_OUTPUT_LINES_PER_COMMAND: usize = 10_000;
const MAX_LINE_LENGTH: usize = 10_000;

// Atomic flags for native macOS menu bar actions
#[cfg(target_os = "macos")]
static NATIVE_MENU_ABOUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
static NATIVE_MENU_DEV: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
static NATIVE_MENU_PERF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Async operation request sent from UI to background task
#[derive(Debug, Clone)]
pub(crate) enum AsyncRequest {
    /// Execute a command
    ExecuteCommand(String, std::path::PathBuf), // command and working directory
    /// Initialize terminal
    InitTerminal,
    /// Restart PTY session
    RestartPty,
    /// Send interrupt signal
    SendInterrupt(String), // PTY handle ID
}

/// Async operation result sent from background task to UI
#[derive(Debug, Clone)]
pub(crate) enum AsyncResult {
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
    /// Direct command execution completed (non-PTY)
    DirectCommandCompleted(CommandBlock),
    /// Direct command execution failed
    DirectCommandFailed { command: String, error: String },
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
    /// Terminal emulator instance (primary/single-pane mode)
    terminal: Option<Terminal>,
    /// Multi-pane tree (Phase 3)
    pane_tree: Option<pane_tree::PaneTree>,
    /// PTY manager for process management (with per-terminal locking)
    pty_manager: Arc<PtyManager>,
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
    /// Ghost completion text (shown dimmed after cursor, accepted with Tab or Right arrow)
    ghost_completion: Option<String>,
    /// Cached prompt segments for colored rendering
    prompt_segments: Vec<mosaicterm::config::prompt::PromptSegment>,
    /// Whether custom fonts have been loaded
    fonts_loaded: bool,
    /// Whether to show the About dialog
    show_about_dialog: bool,
    /// Whether to show the Dev panel
    show_dev_panel: bool,
    /// Startup messages (info/warnings collected during init)
    startup_messages: Vec<String>,
    /// Whether the application window currently has OS focus
    window_has_focus: bool,
    /// Flag to show history search popup
    history_search_active: bool,
    /// Current history search query
    history_search_query: String,
    /// Flag to request focus on history search input (set when popup opens)
    history_search_needs_focus: bool,
    /// Prompt formatter for custom prompts
    prompt_formatter: PromptFormatter,
    /// Context detector for environment tracking (venv, nvm, conda, etc.)
    context_detector: ContextDetector,
    /// Tracks whether the shell had foreground children last frame (for command completion)
    shell_had_children: bool,
    /// Tokio runtime for async operations
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    /// Channel for sending async requests from UI to background
    async_tx: mpsc::UnboundedSender<AsyncRequest>,
    /// Channel for receiving async results from background to UI
    async_rx: mpsc::UnboundedReceiver<AsyncResult>,
    /// Fullscreen TUI overlay for interactive apps
    tui_overlay: mosaicterm::ui::TuiOverlay,
    /// SSH prompt overlay for interactive authentication
    ssh_prompt_overlay: mosaicterm::ui::SshPromptOverlay,
    /// Buffer for accumulating output to detect SSH prompts
    ssh_prompt_buffer: String,
    /// Whether we're currently in an SSH session
    ssh_session_active: bool,
    /// The SSH command that started the session (e.g., "ssh user@host")
    ssh_session_command: Option<String>,
    /// The remote prompt captured from SSH output
    ssh_remote_prompt: Option<String>,
    /// UI color theme (cached egui colors from config)
    ui_colors: mosaicterm::ui::UiColors,
    /// Tool availability detection (Phase 5)
    tool_availability: ToolAvailability,
    /// Tmux session manager (Phase 4)
    tmux_manager: Option<mosaicterm::session::TmuxSessionManager>,
    /// Whether to show the session restore dialog
    show_session_restore_dialog: bool,
    /// Pending sessions to offer for restore
    pending_restore_sessions: Vec<mosaicterm::session::tmux_backend::SessionInfo>,
}

/// Detected tool availability on the system
#[derive(Debug, Clone)]
pub struct ToolAvailability {
    pub fzf: bool,
    pub zoxide: bool,
    pub tmux: bool,
    pub fd: bool,
    pub bat: bool,
    pub eza: bool,
}

impl ToolAvailability {
    pub fn detect() -> Self {
        Self {
            fzf: command_exists("fzf"),
            zoxide: command_exists("zoxide"),
            tmux: command_exists("tmux"),
            fd: command_exists("fd"),
            bat: command_exists("bat"),
            eza: command_exists("eza"),
        }
    }
}

/// Shell-quote a string using single quotes to prevent injection.
/// Any embedded single quotes are escaped as `'\''`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Search system font directories for a font matching `family_name`.
/// Returns the raw TTF/OTF bytes if found, None otherwise.
fn find_system_font(family_name: &str) -> Option<Vec<u8>> {
    let search_dirs: Vec<std::path::PathBuf> = {
        let mut dirs = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if let Some(home) = dirs::home_dir() {
                dirs.push(home.join("Library/Fonts"));
            }
            dirs.push(std::path::PathBuf::from("/Library/Fonts"));
            dirs.push(std::path::PathBuf::from("/System/Library/Fonts"));
            dirs.push(std::path::PathBuf::from(
                "/System/Library/Fonts/Supplemental",
            ));
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(home) = dirs::home_dir() {
                dirs.push(home.join(".local/share/fonts"));
                dirs.push(home.join(".fonts"));
            }
            dirs.push(std::path::PathBuf::from("/usr/share/fonts"));
            dirs.push(std::path::PathBuf::from("/usr/local/share/fonts"));
        }

        #[cfg(target_os = "windows")]
        {
            if let Some(windir) = std::env::var_os("WINDIR") {
                dirs.push(std::path::PathBuf::from(windir).join("Fonts"));
            }
            if let Some(local) = dirs::data_local_dir() {
                dirs.push(local.join("Microsoft\\Windows\\Fonts"));
            }
        }

        dirs
    };

    // Try fc-list first (Linux, some macOS with fontconfig)
    if let Some(path) = find_font_via_fc_list(family_name) {
        info!("Found font via fc-list: {}", path.display());
        return std::fs::read(&path).ok();
    }

    // Normalize the family name for matching: lowercase, strip spaces/hyphens
    let normalized = family_name.to_lowercase().replace([' ', '-'], "");

    let mut best_match: Option<std::path::PathBuf> = None;

    for dir in &search_dirs {
        if !dir.is_dir() {
            continue;
        }
        // Deep walk (Nix, Homebrew, etc. nest fonts deeply)
        if let Ok(entries) = walk_font_dir(dir) {
            for path in entries {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext != "ttf" && ext != "otf" {
                    continue;
                }

                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .replace([' ', '-'], "");

                if !stem.contains(&normalized) {
                    continue;
                }

                let is_regular = stem.ends_with("regular")
                    || stem == normalized
                    || stem.ends_with("nerdfontmono")
                    || stem.ends_with("nerdfontmonoregular")
                    || stem.ends_with("nerdfontregular");
                let is_bold = stem.contains("bold");
                let is_italic = stem.contains("italic") || stem.contains("oblique");
                let is_thin =
                    stem.contains("thin") || stem.contains("extralight") || stem.contains("light");

                if is_regular && !is_bold && !is_italic && !is_thin {
                    best_match = Some(path);
                    break;
                } else if best_match.is_none() && !is_bold && !is_italic && !is_thin {
                    best_match = Some(path);
                }
            }
        }
        if best_match.is_some() {
            break;
        }
    }

    if let Some(path) = &best_match {
        info!("Found font file: {}", path.display());
        std::fs::read(path).ok()
    } else {
        None
    }
}

/// Try to find a font file path using fc-list (fontconfig)
fn find_font_via_fc_list(family_name: &str) -> Option<std::path::PathBuf> {
    let output = std::process::Command::new("fc-list")
        .arg(format!("{}:style=Regular", family_name))
        .arg("--format=%{file}")
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let path_str = String::from_utf8_lossy(&output.stdout);
    let path = std::path::PathBuf::from(path_str.trim());
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

/// Recursively walk a directory collecting font file paths
fn walk_font_dir(dir: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    walk_font_dir_inner(dir, 0, &mut files)?;
    Ok(files)
}

fn walk_font_dir_inner(
    dir: &std::path::Path,
    depth: usize,
    files: &mut Vec<std::path::PathBuf>,
) -> std::io::Result<()> {
    if depth > 10 {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_font_dir_inner(&path, depth + 1, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

/// Send a system notification (macOS: osascript, Linux: notify-send)
fn send_system_notification(title: &str, message: &str) {
    let title = title.to_string();
    let message = message.to_string();

    // Run in background thread to avoid blocking the UI
    std::thread::spawn(move || {
        send_system_notification_sync(&title, &message);
    });
}

fn send_system_notification_sync(title: &str, message: &str) {
    #[cfg(target_os = "macos")]
    {
        let icon_path = find_app_icon_path();

        // Try terminal-notifier first (supports custom app icons)
        if let Some(icon) = &icon_path {
            if let Ok(status) = std::process::Command::new("terminal-notifier")
                .arg("-title")
                .arg(title)
                .arg("-message")
                .arg(message)
                .arg("-appIcon")
                .arg(icon)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
            {
                if status.success() {
                    return;
                }
            }
        }

        // Fallback: osascript (works on all macOS, but icon is Script Editor's)
        let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_msg = message.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escaped_msg, escaped_title
        );

        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    #[cfg(target_os = "linux")]
    {
        let icon_path = find_app_icon_path();
        let mut cmd = std::process::Command::new("notify-send");
        if let Some(icon) = &icon_path {
            cmd.arg("--icon").arg(icon);
        }
        let _ = cmd
            .arg(title)
            .arg(message)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (title, message);
    }
}

/// Embedded icon bytes (same as main.rs) for notification use.
const EMBEDDED_ICON_PNG: &[u8] = include_bytes!("../../icon.png");

/// Return a path to the app icon suitable for system notification commands.
///
/// First checks well-known installed locations, then extracts the embedded
/// icon to a cache directory so it is available even for standalone binaries.
fn find_app_icon_path() -> Option<String> {
    let installed_candidates = [
        "/usr/share/icons/hicolor/256x256/apps/mosaicterm.png",
        "/usr/share/pixmaps/mosaicterm.png",
    ];

    for path in &installed_candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Check next to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let exe_icon = dir.join("icon.png");
            if exe_icon.exists() {
                return exe_icon.to_str().map(|s| s.to_string());
            }
        }
    }

    // Extract the embedded icon to a cache directory
    if let Some(cache_dir) = dirs::cache_dir() {
        let cache_icon = cache_dir.join("mosaicterm").join("icon.png");
        if cache_icon.exists() {
            return cache_icon.to_str().map(|s| s.to_string());
        }
        if let Some(parent) = cache_icon.parent() {
            if std::fs::create_dir_all(parent).is_ok()
                && std::fs::write(&cache_icon, EMBEDDED_ICON_PNG).is_ok()
            {
                return cache_icon.to_str().map(|s| s.to_string());
            }
        }
    }

    None
}

/// Set up the native macOS menu bar with About and Dev menu items.
/// This adds items to the existing native app menu that macOS provides.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn setup_native_menu_bar() {
    #[allow(unused_imports)]
    use cocoa::appkit::{NSApp, NSMenu, NSMenuItem};
    use cocoa::base::{nil, selector};
    use cocoa::foundation::{NSAutoreleasePool, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    #[allow(unused_imports)]
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();

        // Create our menu action handler class
        let handler_class_name = "MosaicTermMenuHandler";
        if Class::get(handler_class_name).is_none() {
            let superclass = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new(handler_class_name, superclass).unwrap();

            extern "C" fn about_action(_this: &Object, _cmd: Sel, _sender: *mut Object) {
                NATIVE_MENU_ABOUT.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            extern "C" fn dev_action(_this: &Object, _cmd: Sel, _sender: *mut Object) {
                NATIVE_MENU_DEV.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            extern "C" fn perf_action(_this: &Object, _cmd: Sel, _sender: *mut Object) {
                NATIVE_MENU_PERF.store(true, std::sync::atomic::Ordering::Relaxed);
            }

            decl.add_method(
                selector("aboutAction:"),
                about_action as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                selector("devAction:"),
                dev_action as extern "C" fn(&Object, Sel, *mut Object),
            );
            decl.add_method(
                selector("perfAction:"),
                perf_action as extern "C" fn(&Object, Sel, *mut Object),
            );

            decl.register();
        }

        let handler_class = Class::get(handler_class_name).unwrap();
        let handler: *mut Object = objc::msg_send![handler_class, new];
        // Handler is retained by menu items via setTarget; `new` returns +1 refcount
        // which keeps it alive for the app's lifetime.

        // Get the main menu
        let main_menu: *mut Object = objc::msg_send![app, mainMenu];
        if main_menu.is_null() {
            return;
        }

        // Get the app menu (first submenu = "MosaicTerm" menu)
        let app_menu_item: *mut Object = objc::msg_send![main_menu, itemAtIndex: 0_isize];
        if app_menu_item.is_null() {
            return;
        }
        let app_menu: *mut Object = objc::msg_send![app_menu_item, submenu];
        if app_menu.is_null() {
            return;
        }

        // Insert "About MosaicTerm" at position 0
        let about_title = NSString::alloc(nil).init_str("About MosaicTerm");
        let about_key = NSString::alloc(nil).init_str("");
        let about_item: *mut Object = objc::msg_send![
            NSMenuItem::alloc(nil),
            initWithTitle: about_title
            action: selector("aboutAction:")
            keyEquivalent: about_key
        ];
        let _: () = objc::msg_send![about_item, setTarget: handler];
        let _: () = objc::msg_send![app_menu, insertItem: about_item atIndex: 0_isize];

        // Insert separator after About
        let sep: *mut Object = objc::msg_send![Class::get("NSMenuItem").unwrap(), separatorItem];
        let _: () = objc::msg_send![app_menu, insertItem: sep atIndex: 1_isize];

        // Create "Dev" menu in the main menu bar
        let dev_menu = NSMenu::alloc(nil).initWithTitle_(NSString::alloc(nil).init_str("Dev"));

        let perf_title = NSString::alloc(nil).init_str("Performance Metrics");
        let perf_key = NSString::alloc(nil).init_str("");
        let perf_item: *mut Object = objc::msg_send![
            NSMenuItem::alloc(nil),
            initWithTitle: perf_title
            action: selector("perfAction:")
            keyEquivalent: perf_key
        ];
        let _: () = objc::msg_send![perf_item, setTarget: handler];
        let _: () = objc::msg_send![dev_menu, addItem: perf_item];

        let log_title = NSString::alloc(nil).init_str("Startup Log");
        let log_key = NSString::alloc(nil).init_str("");
        let log_item: *mut Object = objc::msg_send![
            NSMenuItem::alloc(nil),
            initWithTitle: log_title
            action: selector("devAction:")
            keyEquivalent: log_key
        ];
        let _: () = objc::msg_send![log_item, setTarget: handler];
        let _: () = objc::msg_send![dev_menu, addItem: log_item];

        // Add Dev menu to main menu bar
        let dev_menu_item: *mut Object = msg_send![NSMenuItem::alloc(nil), init];
        let _: () = objc::msg_send![dev_menu_item, setSubmenu: dev_menu];
        let _: () = objc::msg_send![main_menu, addItem: dev_menu_item];
    }
}

/// Strip ANSI escape sequences from a string for plain-text analysis
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ESC [ ... (letter) sequences
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() || c == '~' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequences: ESC ] ... ST (or BEL)
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '\x07' || c == '\\' {
                        break;
                    }
                }
            } else {
                // Other ESC sequences: skip one more char
                chars.next();
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn command_exists(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
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

        // Create PTY manager (with per-terminal locking for better concurrency)
        let pty_manager = Arc::new(PtyManager::new());

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

        // Create prompt formatter from config with style and custom segments
        let prompt_format = runtime_config.config().terminal.prompt_format.clone();
        let prompt_style = runtime_config.config().prompt.style.clone();
        let prompt_segments = runtime_config.config().prompt.segments.clone();
        info!(
            "Loading prompt format from config: '{}', style: {:?}",
            prompt_format, prompt_style
        );
        let prompt_formatter = PromptFormatter::new(prompt_format)
            .with_style(prompt_style)
            .with_custom_segments(prompt_segments);

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
        let runtime_config_clone = runtime_config.clone();

        // Spawn background task to handle async operations
        runtime.spawn(async move {
            async_ops::async_operation_loop(
                &mut request_rx,
                result_tx,
                pty_manager_clone,
                terminal_factory_clone,
                runtime_config_clone,
            )
            .await;
        });

        // Initialize StateManager and add demo commands directly
        let mut state_manager = StateManager::new();
        Self::add_demo_commands(&mut state_manager);

        // Create UI colors from theme before moving runtime_config
        let theme = &runtime_config.config().ui.theme;
        info!(
            "🎨 Theme colors loaded - background: {:?}, foreground: {:?}",
            theme.background, theme.foreground
        );
        info!(
            "🎨 Block colors - status_running: {:?}, status_completed: {:?}",
            theme.blocks.status_running, theme.blocks.status_completed
        );
        let ui_colors = mosaicterm::ui::UiColors::from_theme(theme);

        let tool_availability = ToolAvailability::detect();
        info!(
            "Detected tools: fzf={}, zoxide={}, tmux={}, fd={}, bat={}, eza={}",
            tool_availability.fzf,
            tool_availability.zoxide,
            tool_availability.tmux,
            tool_availability.fd,
            tool_availability.bat,
            tool_availability.eza
        );

        let tmux_manager = if tool_availability.tmux && runtime_config.config().session.persistence
        {
            let mgr = mosaicterm::session::TmuxSessionManager::new();
            let sessions = mgr.list_mosaicterm_sessions();
            if !sessions.is_empty() {
                info!("Found {} existing MosaicTerm tmux sessions", sessions.len());
            }
            Some(mgr)
        } else {
            None
        };

        let pending_restore_sessions = tmux_manager
            .as_ref()
            .map(|mgr| mgr.list_mosaicterm_sessions())
            .unwrap_or_default();
        let show_session_restore_dialog = !pending_restore_sessions.is_empty();

        Self {
            state_manager,
            terminal: None,
            pane_tree: None,
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
            ghost_completion: None,
            prompt_segments: vec![],
            fonts_loaded: false,
            show_about_dialog: false,
            show_dev_panel: false,
            startup_messages: Vec::new(),
            window_has_focus: true,
            history_search_active: false,
            history_search_query: String::new(),
            history_search_needs_focus: false,
            prompt_formatter,
            context_detector: ContextDetector::new(),
            shell_had_children: false,
            runtime,
            async_tx: request_tx,
            async_rx: result_rx,
            tui_overlay: mosaicterm::ui::TuiOverlay::new(),
            ssh_prompt_overlay: mosaicterm::ui::SshPromptOverlay::new(),
            ssh_prompt_buffer: String::new(),
            ssh_session_active: false,
            ssh_session_command: None,
            ssh_remote_prompt: None,
            ui_colors,
            tool_availability,
            tmux_manager,
            show_session_restore_dialog,
            pending_restore_sessions,
        }
    }

    /// Create application with runtime configuration
    pub fn with_config(runtime_config: RuntimeConfig) -> Self {
        let mut app = Self::new();
        app.prompt_formatter =
            PromptFormatter::new(runtime_config.config().terminal.prompt_format.clone())
                .with_style(runtime_config.config().prompt.style.clone())
                .with_custom_segments(runtime_config.config().prompt.segments.clone());
        app.ui_colors = mosaicterm::ui::UiColors::from_theme(&runtime_config.config().ui.theme);
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
                block.add_output_line(mosaicterm::models::OutputLine::new(
                    working_dir.to_string_lossy(),
                ));
                block.mark_completed(std::time::Duration::from_millis(50));
            } else if cmd == "echo 'Hello from MosaicTerm!'" {
                block.add_output_line(mosaicterm::models::OutputLine::new(
                    "Hello from MosaicTerm!",
                ));
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

        // Prefer configured working_directory, then CWD (if not /), then $HOME.
        let working_dir = self
            .runtime_config
            .config()
            .terminal
            .working_directory
            .as_ref()
            .filter(|d| d.is_dir())
            .cloned()
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .filter(|p| p != std::path::Path::new("/"))
            })
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| std::path::PathBuf::from("/"));

        // Create environment with prompt suppression
        let mut environment: std::collections::HashMap<String, String> = std::env::vars().collect();

        // Use xterm-256color so TUI apps work correctly; suppress shell prompts
        // via environment variables. MosaicTerm renders its own prompt, so we need
        // the underlying shell to be quiet.
        environment.insert("TERM".to_string(), "xterm-256color".to_string());

        // Tell MosaicTerm-aware shell init to suppress itself
        environment.insert("MOSAICTERM".to_string(), "1".to_string());

        match shell_type {
            ModelShellType::Bash => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string());
                environment.insert("PS3".to_string(), "".to_string());
                environment.insert("PS4".to_string(), "".to_string());
            }
            ModelShellType::Zsh => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string());
                environment.insert("PS3".to_string(), "".to_string());
                environment.insert("PS4".to_string(), "".to_string());
                // Disable prompt themes in zsh
                environment.insert("ZSH_THEME".to_string(), "".to_string());
                // Prevent p10k instant prompt
                environment.insert("POWERLEVEL9K_INSTANT_PROMPT".to_string(), "off".to_string());
                // Suppress zsh partial-line marker (the % at end of unterminated lines)
                environment.insert("PROMPT_EOL_MARK".to_string(), "".to_string());
            }
            ModelShellType::Fish => {
                environment.insert("fish_prompt".to_string(), "".to_string());
            }
            ModelShellType::Ksh => {
                environment.insert("PS1".to_string(), "".to_string());
            }
            ModelShellType::Csh | ModelShellType::Tcsh => {
                environment.insert("prompt".to_string(), "".to_string());
            }
            ModelShellType::Dash => {
                environment.insert("PS1".to_string(), "".to_string());
            }
            ModelShellType::PowerShell => {
                environment.insert("PROMPT".to_string(), "".to_string());
            }
            ModelShellType::Cmd | ModelShellType::Other => {
                environment.insert("PS1".to_string(), "".to_string());
                environment.insert("PS2".to_string(), "".to_string());
                environment.insert("PS3".to_string(), "".to_string());
                environment.insert("PS4".to_string(), "".to_string());
            }
        }

        // Install shell integration via startup files (not PTY input).
        // We use the app PID as a stable identifier since the shell PID
        // isn't known until after spawn.
        let integration_pid = std::process::id();

        match shell_type {
            ModelShellType::Zsh => {
                if let Some(zdotdir) = mosaicterm::pty::shell_state::create_zdotdir(integration_pid)
                {
                    environment.insert("ZDOTDIR".to_string(), zdotdir.display().to_string());
                    info!("Set ZDOTDIR to {:?} for shell integration", zdotdir);
                }
            }
            ModelShellType::Bash => {
                if let Some(rcfile) =
                    mosaicterm::pty::shell_state::create_bash_rcfile(integration_pid)
                {
                    // We need to remove --noediting and add --rcfile
                    // This is handled below when building the session
                    environment.insert(
                        "MOSAICTERM_BASH_RCFILE".to_string(),
                        rcfile.display().to_string(),
                    );
                    info!("Created bash rcfile at {:?} for shell integration", rcfile);
                }
            }
            _ => {}
        }

        let session = TerminalSession::with_environment(shell_type, working_dir, environment);

        // Create and initialize terminal (shell is spawned here)
        let terminal = self.terminal_factory.create_and_initialize(session).await?;
        self.terminal = Some(terminal);

        // Update state manager
        self.state_manager.set_terminal_ready(true);

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

        // Intercept `z` and `zi` commands when zoxide is available
        let command = if self.tool_availability.zoxide {
            let parts: Vec<&str> = command.split_whitespace().collect();
            if let Some(&cmd) = parts.first() {
                if cmd == "z" || cmd == "zi" {
                    if parts.len() >= 2 {
                        let query = parts[1..].join(" ");
                        if let Some(resolved) = Self::zoxide_query(&query) {
                            info!("zoxide resolved '{} {}' -> 'cd {}'", cmd, query, resolved);
                            // Shell-quote the resolved path to prevent injection
                            let quoted = shell_quote(&resolved);
                            format!("cd {}", quoted)
                        } else {
                            warn!("zoxide found no match for '{}', passing through", query);
                            command
                        }
                    } else {
                        // Bare `z` goes home, matching zoxide default behavior
                        info!("Bare 'z' -> 'cd ~'");
                        "cd ~".to_string()
                    }
                } else {
                    command
                }
            } else {
                command
            }
        } else {
            command
        };

        // Check if this is a TUI command that should open in fullscreen overlay
        if self.is_tui_command(&command) {
            info!(
                "TUI command detected, opening fullscreen overlay: {}",
                command
            );
            return self.handle_tui_command(command).await;
        }

        // Check if this is an SSH command - track it for session management
        if self.is_ssh_command(&command) {
            info!("SSH command detected, will track session: {}", command);
            let host = self.extract_ssh_host(&command);
            mosaicterm::security_audit::log_ssh_connection(&host);
            self.ssh_session_command = Some(command.clone());
            // Session will be activated after successful authentication
        }

        // Check if this is an exit command while in SSH session
        if self.ssh_session_active && self.is_exit_command(&command) {
            info!("Exit command detected in SSH session, will end session");
            // Session will be deactivated when we detect the connection closed
        }

        // Check if this is an interactive command and warn the user
        // Skip warning for SSH since we handle it specially
        if self.is_interactive_command(&command) && !self.is_ssh_command(&command) {
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
                    // PtyManager is already async and thread-safe, no lock needed
                    let pty_manager = &*self.pty_manager;
                    let cmd = format!("{}\n", command);
                    if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                        warn!("Failed to send clear command to PTY: {}", e);
                    }
                }
            }
            return Ok(());
        }

        // Check if we should use direct execution (faster, cleaner)
        // IMPORTANT: Skip direct execution when in SSH session - all commands must go through PTY
        if !self.ssh_session_active && DirectExecutor::check_direct_execution(&command) {
            info!("Using direct execution for command: {}", command);

            // Create command block and mark as running
            let working_dir = self
                .terminal
                .as_ref()
                .map(|t| t.get_working_directory().to_path_buf())
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
                });
            let command_for_block = command.clone();
            let mut command_block = CommandBlock::new(command_for_block, working_dir.clone());
            command_block.mark_running();

            // Add to state manager first
            self.state_manager.add_command_block(command_block);

            // Record command execution time
            self.state_manager.set_last_command_time();

            // Clone command and working directory before sending to async loop (they will be moved)
            let command_for_async = command.clone();
            let working_dir_for_async = working_dir.clone();
            // Send to async loop for execution
            if let Err(e) = self.async_tx.send(AsyncRequest::ExecuteCommand(
                command_for_async,
                working_dir_for_async,
            )) {
                error!(
                    "Failed to send direct execution request: {}, falling back to PTY",
                    e
                );
                // Fall back to PTY execution - continue with normal flow below
            } else {
                // Direct execution is now handled asynchronously
                return Ok(());
            }
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
        self.state_manager.set_last_command_time();

        // UI will be updated automatically on the next frame

        if let Some(_terminal) = &mut self.terminal {
            if let Some(handle) = _terminal.pty_handle() {
                let pty_manager = &*self.pty_manager;

                let cmd = format!("{}\n", command);
                if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                    warn!("Failed to send input to PTY: {}", e);
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

    #[allow(dead_code)]
    fn is_cd_command(&self, command: &str) -> bool {
        commands::is_cd_command(command)
    }

    /// Update the prompt display based on current working directory
    fn update_prompt(&mut self) {
        self.prompt_segments = prompt::build_prompt_segments(
            self.terminal.as_ref(),
            &self.state_manager,
            &self.prompt_formatter,
            self.ssh_session_active,
            self.ssh_remote_prompt.as_deref(),
            self.ssh_session_command.as_deref(),
        );
        let prompt_str: String = self
            .prompt_segments
            .iter()
            .map(|s| s.text.clone())
            .collect();
        self.input_prompt.set_prompt(&prompt_str);
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
        let git_context = context::detect_git_context(self.terminal.as_ref());
        context::update_state_git_context(&mut self.state_manager, git_context);
    }

    /// Parse environment output and update contexts
    fn parse_env_output(&mut self, output: &str) {
        let working_dir = self
            .terminal
            .as_ref()
            .map(|t| t.get_working_directory().to_path_buf());
        let env_context_strings = context::parse_env_and_detect_contexts(
            output,
            &self.context_detector,
            working_dir.as_deref(),
        );
        context::update_state_env_contexts(&mut self.state_manager, env_context_strings);
        info!("Updated contexts from shell environment");
    }

    /// Sync shell state (CWD, environment) directly from the OS after a command completes.
    fn sync_shell_state(&mut self) {
        let shell_pid = self
            .terminal
            .as_ref()
            .and_then(|t| t.pty_handle())
            .and_then(|h| h.pid);

        // Update CWD from OS
        if let Some(pid) = shell_pid {
            if let Some(os_cwd) = mosaicterm::pty::shell_state::read_cwd(pid) {
                if let Some(terminal) = &mut self.terminal {
                    let current = terminal.get_working_directory().to_path_buf();
                    if os_cwd != current {
                        info!("CWD updated from OS: {:?} -> {:?}", current, os_cwd);
                        terminal.set_working_directory(os_cwd.clone());
                        self.state_manager.set_previous_directory(Some(current));

                        if self.tool_availability.zoxide {
                            Self::zoxide_add_background(&os_cwd);
                        }
                    }
                }
            }
        }

        self.update_contexts();

        // Read env vars from the precmd state file (keyed by app PID)
        let app_pid = std::process::id();
        let state_path = mosaicterm::pty::shell_state::state_file_path(app_pid);
        if let Ok(contents) = std::fs::read_to_string(&state_path) {
            let env_lines: String = contents
                .lines()
                .filter(|l| !l.starts_with("EXIT:"))
                .collect::<Vec<_>>()
                .join("\n");
            if !env_lines.is_empty() {
                self.parse_env_output(&env_lines);
            }
        }

        self.update_prompt();
    }

    fn zoxide_query(query: &str) -> Option<String> {
        let output = std::process::Command::new("zoxide")
            .args(["query", "--", query])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
        None
    }

    fn zoxide_add_background(dir: &std::path::Path) {
        let dir_str = dir.display().to_string();
        std::thread::spawn(move || {
            let _ = std::process::Command::new("zoxide")
                .args(["add", &dir_str])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        });
    }

    /// Check if a command is a TUI app that should open in fullscreen overlay
    fn is_tui_command(&self, command: &str) -> bool {
        commands::is_tui_command(command, self.runtime_config.config())
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
                let handle_id = handle.id.clone();

                // Send the TUI command to PTY (TERM is already set in the environment)
                {
                    let pty_manager = &*self.pty_manager;
                    let cmd = format!("{}\n", command);
                    if let Err(e) = pty_manager.send_input(handle, cmd.as_bytes()).await {
                        warn!("Failed to send TUI command to PTY: {}", e);
                        return Err(e);
                    }
                }

                self.tui_overlay.start(command.clone(), handle_id);
                self.set_status_message(Some(format!("Running TUI app: {}", command)));

                info!("TUI overlay started for command: {}", command);
            }
        }

        Ok(())
    }

    /// Check if a command is interactive (TUI-based) and may not work well in block mode
    fn is_interactive_command(&self, command: &str) -> bool {
        commands::is_interactive_command(command)
    }

    /// Check if a command is an exit/logout command
    fn is_exit_command(&self, command: &str) -> bool {
        commands::is_exit_command(command)
    }

    /// Extract the command name from a command line
    fn get_command_name(&self, command: &str) -> String {
        commands::get_command_name(command)
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
            Error::PtyInputSendFailed { .. }
            | Error::PtyReadFailed { .. }
            | Error::PtyStreamDisconnected => {
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

    fn render_session_restore_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Restore Session")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Previous MosaicTerm sessions found")
                        .font(egui::FontId::monospace(13.0))
                        .color(self.ui_colors.foreground),
                );
                ui.add_space(8.0);

                let sessions = self.pending_restore_sessions.clone();
                for session in &sessions {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&session.name)
                                .font(egui::FontId::monospace(12.0))
                                .color(self.ui_colors.accent),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} windows - {}",
                                session.windows, session.created
                            ))
                            .font(egui::FontId::monospace(11.0))
                            .color(self.ui_colors.status_bar.text),
                        );
                    });
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Restore").clicked() {
                        info!("User chose to restore tmux sessions");
                        self.show_session_restore_dialog = false;
                    }
                    if ui.button("Start Fresh").clicked() {
                        if let Some(mgr) = &self.tmux_manager {
                            let killed = mgr.kill_all_mosaicterm_sessions();
                            info!("Killed {} old tmux sessions", killed);
                        }
                        self.show_session_restore_dialog = false;
                        self.pending_restore_sessions.clear();
                    }
                });

                if self.runtime_config.config().session.auto_restore {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("(auto-restore is enabled in config)")
                            .font(egui::FontId::monospace(10.0))
                            .color(egui::Color32::from_rgb(120, 120, 140)),
                    );
                }
            });
    }

    fn render_about_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about_dialog;
        egui::Window::new("About MosaicTerm")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("MosaicTerm")
                            .font(egui::FontId::proportional(22.0))
                            .color(self.ui_colors.accent)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .font(egui::FontId::monospace(13.0))
                            .color(egui::Color32::from_rgb(160, 160, 180)),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("A block-based terminal emulator")
                            .font(egui::FontId::proportional(12.0))
                            .color(egui::Color32::from_rgb(140, 140, 160)),
                    );
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // Configuration
                let cfg = self.runtime_config.config();
                ui.label(
                    egui::RichText::new("Configuration")
                        .font(egui::FontId::monospace(11.0))
                        .color(self.ui_colors.accent)
                        .strong(),
                );
                ui.add_space(2.0);

                let info_color = egui::Color32::from_rgb(160, 160, 180);
                let label_color = egui::Color32::from_rgb(120, 120, 140);

                for (label, value) in [
                    ("Font", cfg.ui.font_family.as_str()),
                    ("Font size", &cfg.ui.font_size.to_string()),
                    ("Prompt style", &format!("{:?}", cfg.prompt.style)),
                    ("Shell", &cfg.terminal.shell_path.display().to_string()),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:>12}:", label))
                                .font(egui::FontId::monospace(11.0))
                                .color(label_color),
                        );
                        ui.label(
                            egui::RichText::new(value)
                                .font(egui::FontId::monospace(11.0))
                                .color(info_color),
                        );
                    });
                }

                ui.add_space(8.0);

                // Tool integrations
                ui.label(
                    egui::RichText::new("Tool Integrations")
                        .font(egui::FontId::monospace(11.0))
                        .color(self.ui_colors.accent)
                        .strong(),
                );
                ui.add_space(2.0);

                let tools = &self.tool_availability;
                for (name, avail) in [
                    ("fzf", tools.fzf),
                    ("zoxide", tools.zoxide),
                    ("tmux", tools.tmux),
                    ("fd", tools.fd),
                    ("bat", tools.bat),
                    ("eza", tools.eza),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:>12}:", name))
                                .font(egui::FontId::monospace(11.0))
                                .color(label_color),
                        );
                        let (icon, color) = if avail {
                            ("found", egui::Color32::from_rgb(100, 200, 100))
                        } else {
                            ("not found", egui::Color32::from_rgb(120, 120, 140))
                        };
                        ui.label(
                            egui::RichText::new(icon)
                                .font(egui::FontId::monospace(11.0))
                                .color(color),
                        );
                    });
                }

                ui.add_space(8.0);

                // Startup log
                if !self.startup_messages.is_empty() {
                    ui.label(
                        egui::RichText::new("Startup Log")
                            .font(egui::FontId::monospace(11.0))
                            .color(self.ui_colors.accent)
                            .strong(),
                    );
                    ui.add_space(2.0);
                    for msg in &self.startup_messages {
                        ui.label(
                            egui::RichText::new(msg)
                                .font(egui::FontId::monospace(10.0))
                                .color(egui::Color32::from_rgb(140, 140, 160)),
                        );
                    }
                }

                ui.add_space(8.0);

                // Platform info
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} | Rust {} | egui {}",
                            std::env::consts::OS,
                            std::env::consts::ARCH,
                            "stable",
                            "0.24",
                        ))
                        .font(egui::FontId::monospace(9.0))
                        .color(egui::Color32::from_rgb(100, 100, 120)),
                    );
                });
            });
        self.show_about_dialog = open;
    }

    fn render_dev_panel(&mut self, ctx: &egui::Context) {
        let mut open = self.show_dev_panel;
        egui::Window::new("Dev Info")
            .open(&mut open)
            .collapsible(true)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                let label_color = egui::Color32::from_rgb(120, 120, 140);
                let info_color = egui::Color32::from_rgb(160, 160, 180);

                ui.label(
                    egui::RichText::new("Startup Messages")
                        .font(egui::FontId::monospace(11.0))
                        .color(self.ui_colors.accent)
                        .strong(),
                );
                ui.add_space(2.0);
                if self.startup_messages.is_empty() {
                    ui.label(
                        egui::RichText::new("(none)")
                            .font(egui::FontId::monospace(10.0))
                            .color(label_color),
                    );
                } else {
                    for msg in &self.startup_messages {
                        ui.label(
                            egui::RichText::new(msg)
                                .font(egui::FontId::monospace(10.0))
                                .color(info_color),
                        );
                    }
                }
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("Runtime State")
                        .font(egui::FontId::monospace(11.0))
                        .color(self.ui_colors.accent)
                        .strong(),
                );
                ui.add_space(2.0);

                let cmd_count = self
                    .state_manager
                    .command_history()
                    .map(|h| h.len())
                    .unwrap_or(0);
                let working_dir = self
                    .terminal
                    .as_ref()
                    .map(|t| t.get_working_directory().display().to_string())
                    .unwrap_or_else(|| "(no terminal)".to_string());

                for (k, v) in [
                    (
                        "Terminal ready",
                        self.state_manager.is_terminal_ready().to_string(),
                    ),
                    ("Working dir", working_dir),
                    ("Command count", cmd_count.to_string()),
                    ("TUI overlay", self.tui_overlay.is_active().to_string()),
                    (
                        "SSH session",
                        self.ssh_prompt_overlay.is_active().to_string(),
                    ),
                    (
                        "Prompt style",
                        format!("{:?}", self.runtime_config.config().prompt.style),
                    ),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:>16}:", k))
                                .font(egui::FontId::monospace(10.0))
                                .color(label_color),
                        );
                        ui.label(
                            egui::RichText::new(&v)
                                .font(egui::FontId::monospace(10.0))
                                .color(info_color),
                        );
                    });
                }

                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("Platform")
                        .font(egui::FontId::monospace(11.0))
                        .color(self.ui_colors.accent)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} | Rust stable | egui 0.24",
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                    ))
                    .font(egui::FontId::monospace(9.0))
                    .color(egui::Color32::from_rgb(100, 100, 120)),
                );
            });
        self.show_dev_panel = open;
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
                // Find the command block and extract all data before borrowing self mutably
                let block_data = {
                    let command_history = self.state_manager.get_command_history();
                    command_history
                        .iter()
                        .find(|b| &b.id == block_id)
                        .map(|block| {
                            (
                                block.command.clone(),
                                block.status,
                                block
                                    .output
                                    .iter()
                                    .map(|line| line.text.clone())
                                    .collect::<Vec<_>>(),
                                block.working_directory.clone(),
                            )
                        })
                };
                if let Some((command, status, output_lines, working_dir)) = block_data {
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
                                let command_to_rerun = command.clone();
                                let working_dir_to_rerun = working_dir.clone();
                                self.input_prompt.add_to_history(command_to_rerun.clone());
                                let _ = self.async_tx.send(AsyncRequest::ExecuteCommand(
                                    command_to_rerun.clone(),
                                    working_dir_to_rerun,
                                ));
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
                            // Use a generous rect that covers the menu including padding/shadow.
                            // The menu has 5-6 buttons with separators; 220x250 is a safe upper bound.
                            let menu_rect =
                                egui::Rect::from_min_size(menu_pos, egui::vec2(220.0, 250.0));
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
}

impl eframe::App for MosaicTermApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Own a clone so we can pass `&mut ui` to nested UIs (e.g. TUI overlay) while still using ctx.
        let ctx_owned = ui.ctx().clone();
        let ctx: &egui::Context = &ctx_owned;

        // Track window focus state for background notifications
        self.window_has_focus = ctx.input(|i| i.focused);

        // Handle keyboard shortcut for performance metrics (Ctrl+Shift+P)
        if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::P)) {
            self.metrics_panel.toggle();
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
                    // PtyManager is already async and thread-safe, no lock needed
                    {
                        let pty_manager = &*self.pty_manager;
                        let cleaned =
                            executor::block_on(async { pty_manager.cleanup_terminated().await });

                        if cleaned > 0 {
                            info!("Cleaned up {} terminated PTY process(es)", cleaned);
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

        // Load custom fonts and set up native menu bar on first frame
        if !self.fonts_loaded {
            self.load_fonts(ctx);
            self.fonts_loaded = true;

            #[cfg(target_os = "macos")]
            setup_native_menu_bar();
        }

        // Set up visual style
        self.setup_visual_style(ctx);

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx, frame);

        // Show loading overlay if active
        if self.state_manager.is_loading() {
            egui::Area::new(egui::Id::new("loading_overlay"))
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgba_premultiplied(30, 30, 40, 240))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(100, 100, 200),
                        ))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .corner_radius(egui::CornerRadius::same(4));

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

        // Session restore dialog (Phase 4)
        if self.show_session_restore_dialog {
            self.render_session_restore_dialog(ctx);
        }

        // Check for native menu bar actions (set by macOS menu callbacks)
        #[cfg(target_os = "macos")]
        {
            use std::sync::atomic::Ordering;
            if NATIVE_MENU_ABOUT.load(Ordering::Relaxed) {
                NATIVE_MENU_ABOUT.store(false, Ordering::Relaxed);
                self.show_about_dialog = true;
            }
            if NATIVE_MENU_DEV.load(Ordering::Relaxed) {
                NATIVE_MENU_DEV.store(false, Ordering::Relaxed);
                self.show_dev_panel = !self.show_dev_panel;
            }
            if NATIVE_MENU_PERF.load(Ordering::Relaxed) {
                NATIVE_MENU_PERF.store(false, Ordering::Relaxed);
                self.metrics_panel.toggle();
            }
        }

        // Non-macOS: provide keyboard shortcut fallbacks
        #[cfg(not(target_os = "macos"))]
        {
            if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::A)) {
                self.show_about_dialog = !self.show_about_dialog;
            }
            if ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::D)) {
                self.show_dev_panel = !self.show_dev_panel;
            }
        }

        if self.show_about_dialog {
            self.render_about_dialog(ctx);
        }
        if self.show_dev_panel {
            self.render_dev_panel(ctx);
        }

        // When the TUI overlay is active, it takes over the entire UI area.
        // Otherwise, render the normal terminal layout.
        if self.tui_overlay.is_active() {
            // Handle input for TUI app BEFORE rendering
            if let Some(input_data) = self.tui_overlay.handle_input(ctx) {
                debug!(
                    "TUI overlay: Sending {} bytes of input to PTY",
                    input_data.len()
                );
                // Send input to PTY
                if let Some(_handle_id) = self.tui_overlay.pty_handle() {
                    let pty_manager = &*self.pty_manager;
                    if let Some(terminal) = &self.terminal {
                        if let Some(pty_handle) = terminal.pty_handle() {
                            if let Err(e) = executor::block_on(async {
                                pty_manager.send_input(pty_handle, &input_data).await
                            }) {
                                warn!("Failed to send input to TUI app: {}", e);
                            }
                        }
                    }
                }
            }

            // Apply pending PTY resize if the overlay detected a size change
            if let Some((rows, cols)) = self.tui_overlay.take_pending_resize() {
                if let Some(terminal) = &self.terminal {
                    if let Some(pty_handle) = terminal.pty_handle() {
                        let pty_manager = &*self.pty_manager;
                        if let Err(e) = executor::block_on(async {
                            pty_manager.resize_pty(pty_handle, rows, cols).await
                        }) {
                            warn!("Failed to resize PTY for TUI overlay: {}", e);
                        }
                    }
                }
            }

            // Render overlay UI (takes the full root ui)
            let overlay_closed = self.tui_overlay.render(ctx, ui);
            if overlay_closed || self.tui_overlay.has_exited() {
                info!("TUI overlay closing");

                // Send Ctrl+C to kill the TUI process and reset terminal
                if let Some(terminal) = &self.terminal {
                    if let Some(pty_handle) = terminal.pty_handle() {
                        let pty_manager = &*self.pty_manager;

                        executor::block_on(async {
                            let _ = pty_manager.send_input(pty_handle, &[3]).await;
                        });

                        std::thread::sleep(std::time::Duration::from_millis(100));

                        executor::block_on(async {
                            let _ = pty_manager.send_input(pty_handle, b"\n").await;
                        });

                        std::thread::sleep(std::time::Duration::from_millis(50));

                        executor::block_on(async {
                            let _ = pty_manager.drain_output(pty_handle).await;
                        });

                        info!("Sent Ctrl+C and cleared PTY buffer after TUI exit");
                    }
                }

                if let Some(cmd) = self.tui_overlay.command() {
                    self.handle_tui_exit(cmd.to_string());
                }

                self.tui_overlay.stop();
            }
        } else {
            // Main layout with scrollable history and pinned input
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(self.ui_colors.background)
                        .inner_margin(egui::Margin::same(2)),
                )
                .show_inside(ui, |ui| {
                    let available_height = ui.available_height();
                    let input_height = 60.0;
                    let history_height = (available_height - input_height).max(100.0);

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

            // Render context menu if active (only in normal mode)
            self.render_context_menu(ctx);
        }

        // Render performance metrics panel if visible
        if self.metrics_panel.is_visible() {
            let pty_count = executor::block_on(async { self.pty_manager.active_count().await });
            let stats = self.state_manager.statistics();
            self.metrics_panel.render_with_ctx(ctx, stats, pty_count);
        }

        // Render SSH prompt overlay if active
        if self.ssh_prompt_overlay.is_active() {
            let should_close = self.ssh_prompt_overlay.render(ctx);

            if should_close {
                // Check if user submitted input or cancelled
                if self.ssh_prompt_overlay.was_cancelled() {
                    // User cancelled - send Ctrl+C to abort SSH
                    if let Some(terminal) = &self.terminal {
                        if let Some(pty_handle) = terminal.pty_handle() {
                            let pty_manager = &*self.pty_manager;
                            executor::block_on(async {
                                let _ = pty_manager.send_input(pty_handle, &[3]).await;
                                // Ctrl+C
                            });
                            info!("SSH prompt cancelled, sent Ctrl+C");
                        }
                    }
                    self.ssh_prompt_overlay.hide();
                } else if let Some(input) = self.ssh_prompt_overlay.take_input() {
                    // Send user input to PTY with newline
                    if let Some(terminal) = &self.terminal {
                        if let Some(pty_handle) = terminal.pty_handle() {
                            let pty_manager = &*self.pty_manager;
                            let input_with_newline = format!("{}\n", input);
                            if let Err(e) = executor::block_on(async {
                                pty_manager
                                    .send_input(pty_handle, input_with_newline.as_bytes())
                                    .await
                            }) {
                                warn!("Failed to send SSH response to PTY: {}", e);
                            } else {
                                info!("Sent SSH response to PTY");

                                // After successful authentication response, activate SSH session
                                // The session will capture the remote prompt from output
                                if self.ssh_session_command.is_some() && !self.ssh_session_active {
                                    info!("Activating SSH session after authentication");
                                    self.ssh_session_active = true;
                                    self.ssh_prompt_buffer.clear();

                                    // Update status to show we're connected
                                    if let Some(cmd) = &self.ssh_session_command {
                                        let host = self.extract_ssh_host(cmd);
                                        mosaicterm::security_audit::log_ssh_session_start(&host);
                                        self.set_status_message(Some(format!(
                                            "🔗 Connected to {}",
                                            host
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    self.ssh_prompt_overlay.hide();
                }
            }
        }

        // Handle async operations
        self.handle_async_operations(ctx);

        // Poll for async operation results (non-blocking)
        self.poll_async_results();

        // Only repaint when needed to save CPU
        // Repaint if: command is running, has pending output, user input changed, or overlays active
        let needs_repaint = self.state_manager.last_command_time().is_some()
            || (self
                .terminal
                .as_ref()
                .map(|t| t.has_pending_output())
                .unwrap_or(false))
            || self.completion_popup.is_visible()
            || self.tui_overlay.is_active()
            || self.ssh_prompt_overlay.is_active();

        if needs_repaint {
            // Repaint immediately for active operations
            ctx.request_repaint();

            // For TUI overlay or SSH prompt, request very fast repaints (16ms = ~60fps) for smooth updates
            if self.tui_overlay.is_active() || self.ssh_prompt_overlay.is_active() {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        } else {
            // Check again in 100ms for idle state (efficient polling)
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("MosaicTerm application shutting down");
        mosaicterm::pty::shell_state::cleanup_shell_files(std::process::id());
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

    /// Load a system font matching the configured font_family into egui.
    /// Falls back to egui's built-in monospace if the font isn't found.
    fn load_fonts(&mut self, ctx: &egui::Context) {
        let font_family = self.runtime_config.config().ui.font_family.clone();
        info!("Looking for system font: '{}'", font_family);

        if let Some(font_data) = find_system_font(&font_family) {
            let size = font_data.len();
            self.startup_messages.push(format!(
                "Loaded font '{}' ({:.1} KB)",
                font_family,
                size as f64 / 1024.0
            ));

            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "custom_mono".to_string(),
                Arc::new(egui::FontData::from_owned(font_data)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "custom_mono".to_string());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("custom_mono".to_string());

            ctx.set_fonts(fonts);
        } else {
            let msg = format!(
                "Font '{}' not found on system — using default monospace",
                font_family
            );
            warn!("{}", msg);
            self.startup_messages.push(msg.clone());
            send_system_notification("MosaicTerm", &msg);
        }

        // Collect tool availability info
        let tools = &self.tool_availability;
        let mut found = Vec::new();
        let mut missing = Vec::new();
        for (name, avail) in [
            ("fzf", tools.fzf),
            ("zoxide", tools.zoxide),
            ("tmux", tools.tmux),
            ("fd", tools.fd),
            ("bat", tools.bat),
            ("eza", tools.eza),
        ] {
            if avail {
                found.push(name);
            } else {
                missing.push(name);
            }
        }
        if !found.is_empty() {
            self.startup_messages
                .push(format!("Integrations: {}", found.join(", ")));
        }
        if !missing.is_empty() {
            self.startup_messages
                .push(format!("Not found: {}", missing.join(", ")));
        }

        let style = &self.runtime_config.config().prompt.style;
        self.startup_messages
            .push(format!("Prompt style: {:?}", style));
    }

    /// Set up visual style for the application
    fn setup_visual_style(&self, ctx: &egui::Context) {
        let font_size = self.runtime_config.config().ui.font_size as f32;
        let mut style = (*ctx.global_style()).clone();

        style.visuals.dark_mode = true;
        style.visuals.window_fill = self.ui_colors.background;
        style.visuals.panel_fill = self.ui_colors.background;
        style.visuals.window_corner_radius = egui::CornerRadius::same(4);
        style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 70));

        style.visuals.selection.bg_fill = self.ui_colors.selection;
        style.visuals.selection.stroke = egui::Stroke::new(1.0, self.ui_colors.accent);

        style.spacing.item_spacing = egui::vec2(4.0, 2.0);
        style.spacing.button_padding = egui::vec2(8.0, 3.0);
        style.spacing.window_margin = egui::Margin::same(4);

        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::monospace(font_size));
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(font_size),
        );

        ctx.set_global_style(style);
    }

    /// Render the fixed input area at the bottom
    fn render_fixed_input_area(&mut self, ui: &mut egui::Ui) {
        let input_frame = egui::Frame::new()
            .fill(self.ui_colors.background)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .outer_margin(egui::Margin::ZERO);

        let frame_response = input_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                // Render prompt segments with Powerline arrow separators
                ui.spacing_mut().item_spacing.x = 0.0;
                let font_size = self.runtime_config.config().ui.font_size as f32;
                let segments = self.prompt_segments.clone();
                let bg_color = self.ui_colors.background;
                for (i, seg) in segments.iter().enumerate() {
                    if let Some(seg_bg) = seg.bg {
                        let mut text = egui::RichText::new(&seg.text)
                            .font(egui::FontId::monospace(font_size))
                            .color(seg.fg)
                            .background_color(seg_bg);
                        if seg.bold {
                            text = text.strong();
                        }
                        ui.label(text);

                        // Draw Powerline arrow separator between segments
                        if let Some(sep) = seg.separator {
                            let next_bg =
                                segments.get(i + 1).and_then(|s| s.bg).unwrap_or(bg_color);
                            let sep_text = egui::RichText::new(sep.to_string())
                                .font(egui::FontId::monospace(font_size))
                                .color(seg_bg)
                                .background_color(next_bg);
                            ui.label(sep_text);
                        }
                    } else {
                        // Non-background segment: paint text directly via painter
                        // to guarantee color isn't overridden by widget styling.
                        let font = egui::FontId::monospace(font_size);
                        let galley =
                            ui.painter()
                                .layout_no_wrap(seg.text.clone(), font.clone(), seg.fg);
                        let (rect, _response) =
                            ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                        ui.painter().galley(rect.min, galley, egui::Color32::WHITE);
                    }
                }
                ui.spacing_mut().item_spacing.x = 4.0;

                let old_input = self.input_prompt.current_input().to_string();
                let mut current_input = old_input.clone();

                let tab_pressed = ui.input(|i| i.key_pressed(egui::Key::Tab));
                let escape_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let up_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                let down_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                let right_pressed = ui.input(|i| i.key_pressed(egui::Key::ArrowRight));

                // Render the TextEdit with no visible frame/border
                let input_response = ui
                    .scope(|ui| {
                        let bg = self.ui_colors.background;
                        let vis = ui.visuals_mut();
                        vis.extreme_bg_color = bg;
                        vis.widgets.inactive.bg_fill = bg;
                        vis.widgets.active.bg_fill = bg;
                        vis.widgets.hovered.bg_fill = bg;
                        vis.widgets.noninteractive.bg_fill = bg;
                        vis.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                        vis.widgets.active.bg_stroke = egui::Stroke::NONE;
                        vis.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                        vis.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
                        vis.selection.stroke = egui::Stroke::NONE;

                        ui.add(
                            egui::TextEdit::singleline(&mut current_input)
                                .font(egui::FontId::monospace(font_size))
                                .desired_width(f32::INFINITY)
                                .frame(egui::Frame::NONE)
                                .margin(egui::Vec2::new(2.0, 3.0))
                                .lock_focus(true),
                        )
                    })
                    .inner;

                // Store input rect for positioning completion popup
                let input_rect = input_response.rect;

                // Render ghost completion text (dimmed, after the current input)
                if let Some(ghost) = &self.ghost_completion {
                    if ghost.len() > current_input.len() {
                        let ghost_suffix = &ghost[current_input.len()..];
                        let mono_font = egui::FontId::monospace(13.0);
                        let input_text_width = ui.fonts_mut(|fonts| {
                            fonts.glyph_width(&mono_font, 'M') * current_input.len() as f32
                        });
                        let ghost_pos = egui::pos2(
                            input_rect.left() + 2.0 + input_text_width,
                            input_rect.top() + 3.0,
                        );
                        ui.painter().text(
                            ghost_pos,
                            egui::Align2::LEFT_TOP,
                            ghost_suffix,
                            mono_font,
                            egui::Color32::from_rgb(80, 85, 100),
                        );
                    }
                }

                // Focus management: the input should have focus unless the
                // user is selecting text in an output block.
                //
                // When the user clicks output text, that TextEdit gets focus
                // and the selection persists so they can Cmd/Ctrl+C.  Focus
                // returns to the input only when the user:
                //   - clicks on the input area (egui handles this naturally)
                //   - presses Escape
                //   - starts typing (any printable character)
                //   - nothing at all has focus (startup / after overlay close)
                if !self.history_search_active
                    && !self.ssh_prompt_overlay.is_active()
                    && !input_response.has_focus()
                {
                    let nothing_focused = ui.ctx().memory(|mem| mem.focused().is_none());
                    let explicit_return = ui.input(|i| {
                        i.key_pressed(egui::Key::Escape)
                            || i.events.iter().any(|e| matches!(e, egui::Event::Text(_)))
                    });
                    if nothing_focused || explicit_return {
                        input_response.request_focus();
                    }
                }

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
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
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
                        // Popup is closed
                        // Accept ghost completion with Tab or Right arrow (only at end of input)
                        let cursor_at_end = if let Some(state) =
                            egui::TextEdit::load_state(ui.ctx(), input_response.id)
                        {
                            state
                                .cursor
                                .char_range()
                                .map(|r| r.primary.index >= current_input.len())
                                .unwrap_or(true)
                        } else {
                            true
                        };

                        let accept_ghost = self.ghost_completion.is_some()
                            && (tab_pressed || (right_pressed && cursor_at_end));

                        if accept_ghost {
                            if let Some(ghost) = self.ghost_completion.take() {
                                self.input_prompt.set_input(ghost);
                                self.state_manager.set_completion_just_applied(true);
                            }
                        } else if tab_pressed {
                            self.handle_tab_completion(&current_input, input_rect);
                        } else if up_pressed {
                            self.input_prompt.navigate_history_previous();
                            self.ghost_completion = None;
                        } else if down_pressed {
                            self.input_prompt.navigate_history_next();
                            self.ghost_completion = None;
                        } else if escape_pressed {
                            self.ghost_completion = None;
                        }

                        if input_changed {
                            if current_input.is_empty() {
                                self.ghost_completion = None;
                            } else {
                                self.ghost_completion =
                                    self.compute_ghost_completion(&current_input);
                            }
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
                            self.ghost_completion = None;

                            // Handle the command (non-blocking)
                            if let Err(e) = executor::block_on(self.handle_command_input(command)) {
                                error!("Command execution failed: {}", e);
                                self.set_status_message(Some(format!("Error: {}", e)));
                            }
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

    /// Compute a ghost completion suggestion for the current input.
    /// Returns the full suggested text (including what's already typed).
    fn compute_ghost_completion(&mut self, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }

        let working_dir = self
            .terminal
            .as_ref()
            .map(|t| t.get_working_directory().to_path_buf())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"))
            });

        if let Ok(result) = self
            .completion_provider
            .get_completions(input, &working_dir)
        {
            if let Some(first) = result.suggestions.first() {
                let parts: Vec<&str> = input.rsplitn(2, char::is_whitespace).collect();
                if parts.len() == 2 {
                    let prefix = parts[1];
                    let suggested = format!("{} {}", prefix, first.text);
                    if suggested != input && suggested.starts_with(input) {
                        return Some(suggested);
                    }
                } else if first.text.starts_with(input) && first.text != input {
                    return Some(first.text.clone());
                }
            }
        }
        None
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
        // Request focus on search field if needed (before showing popup)
        if self.history_search_needs_focus {
            let search_id = egui::Id::new("history_search_input");
            ctx.memory_mut(|mem| mem.request_focus(search_id));
            self.history_search_needs_focus = false;
        }

        // Position popup above the input
        let popup_width = input_rect.width().max(600.0);
        let popup_height = 400.0;
        let popup_x = input_rect.left();
        let popup_y = input_rect.top() - popup_height - 10.0;

        // Create popup above input
        egui::Area::new(egui::Id::new("history_search"))
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(self.ui_colors.blocks.background)
                    .stroke(egui::Stroke::new(2.0, self.ui_colors.accent))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.set_height(popup_height);

                        ui.vertical(|ui| {
                            // Title
                            ui.horizontal(|ui| {
                                ui.heading(
                                    egui::RichText::new("🔍 Search Command History (Ctrl+R)")
                                        .color(self.ui_colors.status_bar.path),
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

                            // Search input - ensure it gets focus
                            let search_id = egui::Id::new("history_search_input");
                            let search_response = ui.add(
                                egui::TextEdit::singleline(&mut self.history_search_query)
                                    .hint_text("Type to search... (fuzzy matching)")
                                    .font(egui::FontId::monospace(14.0))
                                    .desired_width(f32::INFINITY)
                                    .id(search_id),
                            );

                            // Force focus on search field - try multiple methods
                            let search_has_focus = search_response.has_focus()
                                || ui.memory(|mem| mem.focused() == Some(search_id));
                            if self.history_search_needs_focus || !search_has_focus {
                                ctx.memory_mut(|mem| mem.request_focus(search_id));
                                search_response.request_focus();
                                self.history_search_needs_focus = false;
                            }

                            // If search field still doesn't have focus, intercept text input here as fallback
                            // This prevents duplication since we check focus status here
                            if !search_has_focus {
                                let mut text_to_add = String::new();
                                ctx.input(|i| {
                                    for event in &i.events {
                                        if let egui::Event::Text(text) = event {
                                            text_to_add.push_str(text);
                                        }
                                    }
                                });
                                if !text_to_add.is_empty() {
                                    self.history_search_query.push_str(&text_to_add);
                                }
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(8.0);

                            // Search results - use fzf if available for better fuzzy matching
                            let results = if !self.history_search_query.is_empty() {
                                let history_entries: Vec<String> = self
                                    .history_manager
                                    .entries()
                                    .iter()
                                    .rev()
                                    .cloned()
                                    .collect();
                                self.completion_provider.fzf_filter_history(
                                    &history_entries,
                                    &self.history_search_query,
                                )
                            } else {
                                self.history_manager.search(&self.history_search_query)
                            };

                            // Handle arrow key navigation (check if search field is focused)
                            let selected_idx = self.state_manager.get_history_search_selected();
                            let search_has_focus =
                                ui.memory(|mem| mem.focused() == Some(search_id));

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
        let colors = self.ui_colors.clone();

        ui.vertical(|ui| {
            let status_frame = egui::Frame::new()
                .fill(colors.status_bar.background)
                .inner_margin(egui::Margin::symmetric(6, 2));

            status_frame.show(ui, |ui| {
                ui.set_height(16.0);
                ui.horizontal(|ui| {
                    let (status_text, status_color) = match self.state_manager.status_message() {
                        Some(msg) => (msg, colors.status_bar.ssh_indicator),
                        None => ("Ready".to_string(), colors.status_bar.text),
                    };
                    ui.label(
                        egui::RichText::new(status_text)
                            .font(egui::FontId::monospace(11.0))
                            .color(status_color),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let history_len = self.state_manager.get_command_history().len();
                        if history_len > 0 {
                            ui.label(
                                egui::RichText::new(format!("{} cmds", history_len))
                                    .font(egui::FontId::monospace(11.0))
                                    .color(colors.status_bar.text),
                            );
                        }
                        if let Some(tree) = &self.pane_tree {
                            let count = tree.pane_count();
                            if count > 1 {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(format!("pane {}", tree.active_id()))
                                        .font(egui::FontId::monospace(11.0))
                                        .color(colors.accent),
                                );
                            }
                        }
                    });
                });
            });

            // Scrollable command history - commands from newest to oldest (bottom to top)
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .scroll_source(
                    egui::scroll_area::ScrollSource::SCROLL_BAR
                        | egui::scroll_area::ScrollSource::MOUSE_WHEEL,
                )
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Commands appear in execution order: oldest at top, newest at bottom
                        let command_history = self.state_manager.get_command_history();
                        for (i, block) in command_history.iter().enumerate() {
                            if let Some((block_id, pos)) =
                                Self::render_single_command_block_static(ui, block, i, &colors)
                            {
                                // Right-click detected, show context menu
                                self.command_blocks
                                    .interaction_state_mut()
                                    .context_menu_block = Some(block_id);
                                self.command_blocks.interaction_state_mut().context_menu_pos =
                                    Some(pos);
                            }
                        }

                        if command_history.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("MosaicTerm")
                                        .font(egui::FontId::proportional(28.0))
                                        .color(colors.accent)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new("Type a command below to get started")
                                        .font(egui::FontId::proportional(14.0))
                                        .color(colors.status_bar.text),
                                );
                                ui.add_space(20.0);
                                let hint_color = egui::Color32::from_rgb(100, 100, 120);
                                for hint in &[
                                    "Tab         Auto-complete commands and paths",
                                    "Up/Down     Navigate command history",
                                    "Ctrl+R      Search history",
                                    "Ctrl+L      Clear screen",
                                    "Ctrl+Q      Quit",
                                ] {
                                    ui.label(
                                        egui::RichText::new(*hint)
                                            .font(egui::FontId::monospace(12.0))
                                            .color(hint_color),
                                    );
                                }
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
        colors: &mosaicterm::ui::UiColors,
    ) -> Option<(String, egui::Pos2)> {
        let accent_color = match block.status {
            ExecutionStatus::Running => colors.blocks.status_running,
            ExecutionStatus::Failed => colors.blocks.status_failed,
            ExecutionStatus::Completed => colors.blocks.status_completed,
            ExecutionStatus::Cancelled => colors.blocks.status_cancelled,
            ExecutionStatus::Pending => colors.blocks.border,
            ExecutionStatus::TuiMode => colors.blocks.status_tui,
        };

        let is_compact = block.output.is_empty();

        // Base block: very subtle background lift from main bg
        let block_bg = egui::Color32::from_rgb(
            colors.background.r().saturating_add(4),
            colors.background.g().saturating_add(4),
            colors.background.b().saturating_add(5),
        );

        let block_frame = egui::Frame::new()
            .fill(block_bg)
            .inner_margin(if is_compact {
                egui::Margin::symmetric(12, 4)
            } else {
                egui::Margin::symmetric(12, 6)
            })
            .outer_margin(egui::Margin {
                left: 4,
                right: 4,
                top: 1,
                bottom: 1,
            })
            .corner_radius(egui::CornerRadius::same(4));

        // Reserve a shape slot BEFORE the block content for hover background.
        // This ensures the hover fill is drawn BEHIND the text.
        let hover_bg_idx = ui.painter().add(egui::Shape::Noop);

        let frame_response = block_frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&block.command)
                            .font(egui::FontId::monospace(12.5))
                            .color(colors.blocks.command_text)
                            .strong(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(block.timestamp.format("%H:%M:%S").to_string())
                                .font(egui::FontId::monospace(10.0))
                                .color(colors.blocks.timestamp),
                        );

                        let (status_text, status_color) = match block.status {
                            ExecutionStatus::Running => ("running", colors.blocks.status_running),
                            ExecutionStatus::Completed => ("ok", colors.blocks.status_completed),
                            ExecutionStatus::Failed => ("fail", colors.blocks.status_failed),
                            ExecutionStatus::Cancelled => {
                                ("cancel", colors.blocks.status_cancelled)
                            }
                            ExecutionStatus::Pending => ("...", colors.blocks.status_pending),
                            ExecutionStatus::TuiMode => ("tui", colors.blocks.status_tui),
                        };

                        ui.label(
                            egui::RichText::new(status_text)
                                .font(egui::FontId::monospace(10.0))
                                .color(status_color),
                        );
                    });
                });

                if !block.output.is_empty() {
                    ui.add_space(3.0);

                    let mono_font = egui::FontId::monospace(12.0);
                    let output_color = colors.blocks.output_text;
                    let (plain_text, layout_job) = mosaicterm::ui::text::build_output_layout_job(
                        &block.output,
                        mono_font.clone(),
                        output_color,
                    );

                    let job_for_layouter = layout_job;
                    let mut layouter =
                        move |ui: &egui::Ui, _buf: &dyn egui::TextBuffer, wrap_width: f32| {
                            let mut j = job_for_layouter.clone();
                            j.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(j))
                        };
                    let mut text_ref: &str = &plain_text;
                    ui.add(
                        egui::TextEdit::multiline(&mut text_ref)
                            .id_source(format!("output_{}", block.id))
                            .font(mono_font)
                            .desired_width(f32::INFINITY)
                            .frame(egui::Frame::NONE)
                            .layouter(&mut layouter),
                    );
                }
            });
        });

        // Hover effect: fill the pre-reserved shape slot so the background draws BEHIND text
        if frame_response.response.hovered() {
            let rect = frame_response.response.rect;

            let hover_bg = egui::Color32::from_rgb(
                colors.background.r().saturating_add(14),
                colors.background.g().saturating_add(14),
                colors.background.b().saturating_add(18),
            );
            ui.painter().set(
                hover_bg_idx,
                egui::Shape::rect_filled(rect, egui::CornerRadius::same(4), hover_bg),
            );

            // Border glow (on top is fine, these are decorative non-text shapes)
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(4),
                egui::Stroke::new(
                    0.5,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                ),
                egui::StrokeKind::Outside,
            );

            // Accent stripe on left edge
            let stripe = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.top()),
                egui::pos2(rect.left() + 2.5, rect.bottom()),
            );
            ui.painter().rect_filled(
                stripe,
                egui::CornerRadius {
                    nw: 4,
                    sw: 4,
                    ne: 0,
                    se: 0,
                },
                accent_color,
            );
        }

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
                AsyncResult::DirectCommandCompleted(command_block) => {
                    info!(
                        "Direct command execution completed: {}",
                        command_block.command
                    );
                    // Find the matching command block in history and update it
                    let command_history = self.state_manager.get_command_history();
                    if let Some(existing_block) = command_history.iter().find(|b| {
                        b.command == command_block.command && b.status == ExecutionStatus::Running
                    }) {
                        let block_id = existing_block.id.clone();
                        // Update status
                        self.state_manager
                            .update_command_block_status(&block_id, command_block.status);
                        // Update output lines
                        for output_line in command_block.output {
                            self.state_manager.add_output_line(&block_id, output_line);
                        }
                        // Update exit code if available
                        if let Some(exit_code) = command_block.exit_code {
                            if let Some(session) = self.state_manager.active_session_mut() {
                                if let Some(block) = session
                                    .command_history
                                    .iter_mut()
                                    .find(|b| b.id == block_id)
                                {
                                    block.exit_code = Some(exit_code);
                                }
                            }
                        }
                        info!(
                            "Updated command block {} with direct execution results",
                            block_id
                        );
                    } else {
                        // If not found, add it (shouldn't happen, but handle gracefully)
                        warn!("Direct command completed but block not found in history, adding new block");
                        self.state_manager.add_command_block(command_block);
                    }
                }
                AsyncResult::DirectCommandFailed { command, error } => {
                    error!("Direct command execution failed: {} - {}", command, error);
                    // Find the matching command block and mark it as failed
                    let command_history = self.state_manager.get_command_history();
                    if let Some(block) = command_history
                        .iter()
                        .find(|b| b.command == command && b.status == ExecutionStatus::Running)
                    {
                        let block_id = block.id.clone();
                        self.state_manager
                            .update_command_block_status(&block_id, ExecutionStatus::Failed);
                        self.state_manager.add_output_line(
                            &block_id,
                            mosaicterm::models::OutputLine::new(format!("Error: {}", error)),
                        );
                        // Set exit code to 1
                        if let Some(session) = self.state_manager.active_session_mut() {
                            if let Some(block) = session
                                .command_history
                                .iter_mut()
                                .find(|b| b.id == block_id)
                            {
                                block.exit_code = Some(1);
                            }
                        }
                    }
                }
                AsyncResult::CommandStarted(command_block) => {
                    // Command block already added to history when command was sent
                    // This is just a notification, no action needed
                    debug!("Command started: {}", command_block.command);
                }
                AsyncResult::CommandCompleted {
                    index,
                    status,
                    exit_code,
                } => {
                    // Update command block at the given index
                    if let Some(command_history) = self.state_manager.command_history_mut() {
                        if let Some(block) = command_history.get_mut(index) {
                            block.status = status;
                            block.exit_code = exit_code;
                            debug!(
                                "Command at index {} completed with status {:?}",
                                index, status
                            );
                        } else {
                            warn!("CommandCompleted for invalid index: {}", index);
                        }
                    }
                }
            }
        }
    }

    /// Check output lines for well-known shell error patterns.
    /// Returns an inferred exit code if an error pattern is found.
    fn detect_error_in_output(lines: &[mosaicterm::models::OutputLine]) -> Option<i32> {
        for line in lines {
            let t = line.text.trim();
            if t.is_empty() {
                continue;
            }
            let lower = t.to_ascii_lowercase();
            // zsh / bash: "command not found"
            if lower.contains("command not found") {
                return Some(127);
            }
            // bash: "No such file or directory" (when running a script path)
            if lower.contains("no such file or directory") {
                return Some(127);
            }
            // zsh/bash: "permission denied"
            if lower.contains("permission denied") {
                return Some(126);
            }
            // zsh/bash: "is a directory"
            if lower.ends_with("is a directory") {
                return Some(126);
            }
            // common: "syntax error"
            if lower.contains("syntax error") {
                return Some(2);
            }
            // zsh: "bad option" / "unknown option"
            if lower.contains("bad option") || lower.contains("unknown option") {
                return Some(1);
            }
            // git: "'foo' is not a git command"
            if lower.contains("is not a") && lower.contains("command") {
                return Some(1);
            }
            // segfault
            if lower.contains("segmentation fault") || lower.contains("killed") {
                return Some(139);
            }
        }
        None
    }

    /// Handle async operations (called from update) - SIMPLIFIED VERSION
    fn handle_async_operations(&mut self, _ctx: &egui::Context) {
        let bg_notification_threshold_ms: u128 = 10_000;
        // SIMPLIFIED: Poll PTY output and add to current command (no complex prompt detection)
        let mut should_update_contexts = false;
        let mut timeout_kill_status_message: Option<String> = None;
        let mut ssh_session_ended = false; // Track if SSH session ended
        let mut ssh_session_should_activate = false; // Track if SSH session should be activated
        let mut new_remote_prompt: Option<String> = None; // Track new remote prompt

        if let Some(_terminal) = &mut self.terminal {
            if let Some(handle) = _terminal.pty_handle() {
                // PtyManager is async and thread-safe, use async read
                // Use blocking executor since we're in a sync context
                let pty_manager = &*self.pty_manager;
                if let Ok(data) =
                    executor::block_on(async { pty_manager.try_read_output_now(handle).await })
                {
                    if !data.is_empty() {
                        // If TUI overlay is active, send RAW output there (don't process it!)
                        if self.tui_overlay.is_active() {
                            debug!("Routing {} bytes to TUI overlay", data.len());

                            // Send raw bytes directly to overlay - TUI apps need ANSI codes!
                            self.tui_overlay.add_raw_output(&data);

                            // Track alternate screen enter (TUI app starting)
                            let has_alt_screen_enter = data.windows(8).any(|w| w == b"\x1b[?1049h")
                                || data.windows(6).any(|w| w == b"\x1b[?47h");
                            if has_alt_screen_enter {
                                self.tui_overlay.note_alt_screen_enter();
                            }

                            // Only check for exit after the grace period
                            if !self.tui_overlay.in_grace_period() {
                                let has_alt_screen_exit =
                                    data.windows(8).any(|w| w == b"\x1b[?1049l")
                                        || data.windows(6).any(|w| w == b"\x1b[?47l");

                                // Only treat alt screen exit as app exit if we saw the enter first
                                if has_alt_screen_exit && self.tui_overlay.saw_alt_screen() {
                                    debug!("Detected alternate screen exit - TUI app exited");
                                    self.tui_overlay.mark_exited();
                                } else if !has_alt_screen_exit {
                                    let data_str = String::from_utf8_lossy(&data);
                                    let stripped = strip_ansi_codes(&data_str);
                                    let trimmed = stripped.trim();
                                    if let Some(last_line) = trimmed.lines().last() {
                                        let line = last_line.trim();
                                        let looks_like_prompt = line.ends_with("$ ")
                                            || line.ends_with("% ")
                                            || line.ends_with("# ")
                                            || line.ends_with("$")
                                            || line.ends_with("%")
                                            || line.ends_with("> ");
                                        if looks_like_prompt && line.len() < 200 {
                                            debug!(
                                                "Detected shell prompt after TUI exit: '{}'",
                                                line
                                            );
                                            self.tui_overlay.mark_exited();
                                        }
                                    }
                                }
                            }

                            return; // Don't process output for command blocks
                        }

                        // Check for SSH prompts that need user interaction
                        let data_str = String::from_utf8_lossy(&data);

                        // Accumulate output in buffer for SSH prompt detection
                        // Keep only last 2KB to avoid memory issues
                        self.ssh_prompt_buffer.push_str(&data_str);
                        if self.ssh_prompt_buffer.len() > 2048 {
                            let start = self.ssh_prompt_buffer.len() - 2048;
                            self.ssh_prompt_buffer = self.ssh_prompt_buffer[start..].to_string();
                        }

                        // Check for SSH prompts if overlay is not already active
                        if !self.ssh_prompt_overlay.is_active() {
                            if let Some((prompt_type, message)) =
                                mosaicterm::ui::SshPromptOverlay::detect_ssh_prompt(
                                    &self.ssh_prompt_buffer,
                                )
                            {
                                info!("Detected SSH prompt: {:?}", prompt_type);
                                self.ssh_prompt_overlay.show(prompt_type, message);
                                self.ssh_prompt_buffer.clear();
                                return; // Wait for user input before processing more
                            }
                        }

                        // SSH session management
                        if self.ssh_session_active {
                            // Check for SSH connection closed
                            if Self::detect_ssh_session_end_static(&data_str) {
                                info!("SSH session ended (detected end message)");
                                ssh_session_ended = true;
                            } else if let Some(prompt) =
                                Self::detect_remote_prompt_static(&data_str)
                            {
                                // Check if this prompt looks like the local prompt (different from remote)
                                // This helps detect when SSH exited and we're back to local shell
                                if Self::is_local_prompt_static(
                                    self.ssh_remote_prompt.as_ref(),
                                    &prompt,
                                ) {
                                    info!("SSH session ended (detected local prompt)");
                                    ssh_session_ended = true;
                                } else if self.ssh_remote_prompt.as_ref() != Some(&prompt) {
                                    // Same SSH session, just update the prompt (path might have changed)
                                    info!("Updated remote prompt: '{}'", prompt);
                                    new_remote_prompt = Some(prompt);
                                }
                            }
                        } else if self.ssh_session_command.is_some()
                            && !self.ssh_prompt_overlay.is_active()
                        {
                            // SSH command started but session not active yet (passwordless auth)
                            // Activate session if we detect a remote prompt
                            if let Some(prompt) = Self::detect_remote_prompt_static(&data_str) {
                                info!("Detected remote prompt for passwordless SSH, activating session");
                                ssh_session_should_activate = true;
                                new_remote_prompt = Some(prompt);
                            }
                        }

                        // Check for ANSI clear screen sequences (\x1b[H\x1b[2J or \x1b[2J)
                        let data_str = String::from_utf8_lossy(&data);
                        let is_clear_sequence = data_str.contains("\x1b[H\x1b[2J")
                            || data_str.contains("\x1b[2J")
                            || (data_str.contains("\x1b[H") && data_str.len() < 20); // Short sequences with just cursor home

                        if is_clear_sequence {
                            info!("Detected ANSI clear screen sequence, clearing command history");
                            self.state_manager.clear_command_history();
                            // Skip processing this output as it's just a clear command
                        } else {
                            debug!("PTY read {} bytes", data.len(),);

                            // --- All output goes through the OutputProcessor ---
                            // This ensures OSC / CSI sequences are properly
                            // stripped before any text reaches the UI.
                            let ready_lines = futures::executor::block_on(async {
                                _terminal
                                    .process_output(&data, mosaicterm::terminal::StreamType::Stdout)
                                    .await
                                    .unwrap_or_else(|e| {
                                        debug!("Error processing output: {}", e);
                                        Vec::new()
                                    })
                            });

                            // Peek at the partial (un-newlined) text the
                            // processor is accumulating — used for prompt
                            // detection below.
                            let partial_line_text: Option<String> =
                                _terminal.peek_partial_line().map(|s| s.to_owned());

                            debug!(
                                "Got {} lines from terminal, partial: {:?}",
                                ready_lines.len(),
                                partial_line_text.as_deref().unwrap_or("")
                            );

                            let last_command_time = self.state_manager.last_command_time();
                            let mut lines_count = 0;
                            let mut should_clear_command_time = false;

                            if let Some(command_history) = self.state_manager.command_history_mut()
                            {
                                if let Some(last_block) = command_history.last_mut() {
                                    if last_block.status
                                        == mosaicterm::models::ExecutionStatus::TuiMode
                                    {
                                        debug!("Skipping output for TUI mode command");
                                        return;
                                    }

                                    let command_text = last_block.command.trim().to_string();
                                    let current_output_count = last_block.output.len();

                                    // ---- Add ready (newline-terminated) lines ----
                                    let mut lines_to_add = Vec::with_capacity(ready_lines.len());

                                    for (idx, line) in ready_lines.iter().enumerate() {
                                        let line_text = line.text.trim();
                                        let is_first_few = current_output_count + idx < 5;

                                        let ends_with_prompt_char = line_text.ends_with("$ ")
                                            || line_text.ends_with("$")
                                            || line_text.ends_with("% ")
                                            || line_text.ends_with("%")
                                            || line_text.ends_with("> ")
                                            || line_text.ends_with(">")
                                            || line_text.ends_with("# ")
                                            || line_text.ends_with("#");

                                        // Classic prompt: "user@host:path$ " or "user@host path% "
                                        let classic_prompt = ends_with_prompt_char
                                            && (line_text.contains("@")
                                                || line_text.contains(":")
                                                || line_text.len() < 50
                                                || (self.ssh_session_active
                                                    && line_text.len() < 100));
                                        // Oh My Zsh / Powerline prompt: " user@host  ~  " (no trailing $/%,
                                        // but has user@host and ends with a path-like segment + whitespace)
                                        let ohmyzsh_prompt = line_text.contains("@")
                                            && line_text.len() < 120
                                            && (line_text.trim_end().ends_with("~")
                                                || line_text.trim_end().ends_with("/"));
                                        let looks_like_prompt = classic_prompt || ohmyzsh_prompt;

                                        // Detect user@host pattern (prompt from zsh/Oh My Zsh themes)
                                        let has_user_at_host =
                                            line_text.contains("@") && line_text.len() < 200;
                                        // Prompt line with command echo:
                                        //   " user@host  ~  eza -l"
                                        let is_prompt_command_echo = has_user_at_host
                                            && !command_text.is_empty()
                                            && line_text.ends_with(&command_text);

                                        // Filter prompts in BOTH local and SSH modes
                                        let should_skip = line_text == command_text
                                            || (is_first_few
                                                && !line_text.is_empty()
                                                && command_text.starts_with(line_text)
                                                && line_text.len() <= 3)
                                            || line_text.contains("^C")
                                            || line_text.is_empty()
                                            || looks_like_prompt
                                            || is_prompt_command_echo;

                                        // If we see a prompt and command is still running,
                                        // mark completed with proper exit code
                                        if looks_like_prompt
                                            && last_block.status
                                                == mosaicterm::models::ExecutionStatus::Running
                                        {
                                            let elapsed = last_command_time
                                                .map(|s| s.elapsed())
                                                .unwrap_or_default();
                                            let mut exit_code =
                                                mosaicterm::pty::shell_state::read_exit_code(
                                                    std::process::id(),
                                                )
                                                .unwrap_or(0);
                                            if exit_code == 0 {
                                                if let Some(err_code) =
                                                    Self::detect_error_in_output(&last_block.output)
                                                {
                                                    exit_code = err_code;
                                                }
                                            }
                                            last_block.mark_completed_with_code(elapsed, exit_code);
                                            should_clear_command_time = true;
                                            debug!(
                                                "Command completed (prompt in line batch, exit {}): {}",
                                                exit_code, last_block.command
                                            );
                                        }

                                        if !should_skip {
                                            let mut line_to_add = line.clone();
                                            if line_to_add.text.len() > MAX_LINE_LENGTH {
                                                line_to_add.text.truncate(MAX_LINE_LENGTH);
                                                line_to_add.text.push_str("... [truncated]");
                                            }
                                            lines_to_add.push(line_to_add);
                                        }
                                    }

                                    if !lines_to_add.is_empty() {
                                        last_block.add_output_lines(lines_to_add.clone());
                                        lines_count = lines_to_add.len();

                                        // Eagerly detect shell errors in the new output.
                                        // If the output contains "command not found" or similar,
                                        // mark the block as failed right now.
                                        if last_block.status
                                            == mosaicterm::models::ExecutionStatus::Running
                                        {
                                            if let Some(err_code) =
                                                Self::detect_error_in_output(&lines_to_add)
                                            {
                                                let elapsed = last_command_time
                                                    .map(|s| s.elapsed())
                                                    .unwrap_or_default();
                                                last_block
                                                    .mark_completed_with_code(elapsed, err_code);
                                                should_clear_command_time = true;
                                                debug!(
                                                    "Command failed (error pattern in output, exit {}): {}",
                                                    err_code, last_block.command
                                                );
                                            }
                                        }
                                    }

                                    if last_block.output.len() > MAX_OUTPUT_LINES_PER_COMMAND {
                                        let to_remove = last_block.output.len()
                                            - MAX_OUTPUT_LINES_PER_COMMAND
                                            + 1000;
                                        last_block.output.drain(0..to_remove);
                                        let truncation_notice =
                                            mosaicterm::models::OutputLine::new(format!(
                                                "... [truncated {} lines due to size limit] ...",
                                                to_remove
                                            ));
                                        last_block.output.insert(0, truncation_notice);
                                        warn!(
                                            "Truncated {} lines from command output (limit: {})",
                                            to_remove, MAX_OUTPUT_LINES_PER_COMMAND
                                        );
                                    }

                                    // ---- Prompt-based completion detection ----
                                    // Use the partial line from the OutputProcessor
                                    // (already has OSC stripped) for prompt detection.
                                    let prompt_detected =
                                        if let Some(ref partial) = partial_line_text {
                                            let pt = partial.trim();
                                            if pt.is_empty() {
                                                false
                                            } else {
                                                let classic = pt.ends_with("$ ")
                                                    || pt.ends_with("$")
                                                    || pt.ends_with("% ")
                                                    || pt.ends_with("%")
                                                    || pt.ends_with("> ")
                                                    || pt.ends_with(">")
                                                    || pt.ends_with("# ")
                                                    || pt.ends_with("#")
                                                    || (pt.contains("@")
                                                        && (pt.contains("$")
                                                            || pt.contains("%")
                                                            || pt.contains("#")));
                                                let ohmyzsh = pt.contains("@")
                                                    && pt.len() < 120
                                                    && (pt.trim_end().ends_with("~")
                                                        || pt.trim_end().ends_with("/"));
                                                classic || ohmyzsh
                                            }
                                        } else {
                                            false
                                        };

                                    if prompt_detected
                                        && last_block.status
                                            == mosaicterm::models::ExecutionStatus::Running
                                    {
                                        let elapsed = last_command_time
                                            .map(|s| s.elapsed())
                                            .unwrap_or_default();
                                        let mut exit_code =
                                            mosaicterm::pty::shell_state::read_exit_code(
                                                std::process::id(),
                                            )
                                            .unwrap_or(0);
                                        if exit_code == 0 {
                                            if let Some(err_code) =
                                                Self::detect_error_in_output(&last_block.output)
                                            {
                                                exit_code = err_code;
                                            }
                                        }
                                        last_block.mark_completed_with_code(elapsed, exit_code);
                                        should_clear_command_time = true;
                                        debug!(
                                            "Command completed (prompt in partial, exit {}): {}",
                                            exit_code, last_block.command
                                        );
                                    }

                                    // Also check completed lines for prompt-based completion
                                    if !should_clear_command_time {
                                        if let Some(start_time) = last_command_time {
                                            let elapsed_ms = start_time.elapsed().as_millis();
                                            let elapsed_secs = (elapsed_ms / 1000) as u64;

                                            let output_lines_clone: Vec<_> =
                                                last_block.output.clone();
                                            let command_clone = last_block.command.clone();

                                            let is_complete = if !output_lines_clone.is_empty() {
                                                let recent_lines = if output_lines_clone.len() > 3 {
                                                    &output_lines_clone
                                                        [output_lines_clone.len() - 3..]
                                                } else {
                                                    &output_lines_clone
                                                };
                                                let detector = mosaicterm::terminal::prompt::CommandCompletionDetector::new();
                                                detector.is_command_complete(recent_lines)
                                            } else {
                                                false
                                            };

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

                                            let timeout_config =
                                                &self.runtime_config.config().terminal.timeout;
                                            let timeout_secs = if is_interactive_command {
                                                timeout_config.interactive_command_timeout_secs
                                            } else {
                                                timeout_config.regular_command_timeout_secs
                                            };

                                            if is_complete {
                                                let mut exit_code =
                                                    mosaicterm::pty::shell_state::read_exit_code(
                                                        std::process::id(),
                                                    )
                                                    .unwrap_or(0);
                                                if exit_code == 0 {
                                                    if let Some(err_code) =
                                                        Self::detect_error_in_output(
                                                            &output_lines_clone,
                                                        )
                                                    {
                                                        exit_code = err_code;
                                                    }
                                                }
                                                last_block.mark_completed_with_code(
                                                    std::time::Duration::from_millis(
                                                        elapsed_ms.try_into().unwrap_or(1000),
                                                    ),
                                                    exit_code,
                                                );
                                                should_clear_command_time = true;
                                                debug!(
                                                    "Command completed based on prompt detection (exit {}): {}",
                                                    exit_code, command_clone
                                                );
                                            } else if timeout_secs > 0
                                                && elapsed_secs >= timeout_secs
                                            {
                                                warn!(
                                                    "Command exceeded timeout of {}s: {}",
                                                    timeout_secs, command_clone
                                                );
                                                let timeout_notice =
                                                    mosaicterm::models::OutputLine::new(format!(
                                                        "\n[Timeout: Command exceeded {}s limit]",
                                                        timeout_secs
                                                    ));
                                                last_block.output.push(timeout_notice);
                                                last_block.mark_completed(
                                                    std::time::Duration::from_secs(timeout_secs),
                                                );
                                                should_clear_command_time = true;

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
                                            }
                                        }
                                    }
                                }
                            }

                            if lines_count > 0 {
                                self.state_manager.statistics_mut().total_output_lines +=
                                    lines_count;
                            }

                            if should_clear_command_time {
                                self.state_manager.clear_last_command_time();
                                should_update_contexts = true;
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

        // Apply SSH session state changes after borrows are released
        if ssh_session_ended {
            self.end_ssh_session();
        } else {
            // Activate SSH session if detected (passwordless auth path)
            if ssh_session_should_activate {
                self.ssh_session_active = true;
                self.ssh_prompt_buffer.clear();
                if let Some(cmd) = &self.ssh_session_command {
                    let host = self.extract_ssh_host(cmd);
                    self.set_status_message(Some(format!("🔗 Connected to {}", host)));
                }
            }

            // Update remote prompt if captured
            if let Some(prompt) = new_remote_prompt {
                self.ssh_remote_prompt = Some(prompt);
                self.update_prompt();
            }
        }

        // Idle prompt detection: check the OutputProcessor's partial line
        // for a shell prompt even when no new data has arrived.  This catches
        // fast-failing commands (typos, command-not-found) where the error +
        // prompt arrive in one burst and no subsequent data triggers the
        // in-data-handler check.
        if !should_update_contexts && !self.ssh_session_active {
            if let Some(terminal) = &self.terminal {
                if let Some(partial) = terminal.peek_partial_line() {
                    let pt = partial.trim();
                    let prompt_detected = if pt.is_empty() {
                        false
                    } else {
                        let classic = pt.ends_with("$ ")
                            || pt.ends_with("$")
                            || pt.ends_with("% ")
                            || pt.ends_with("%")
                            || pt.ends_with("> ")
                            || pt.ends_with(">")
                            || pt.ends_with("# ")
                            || pt.ends_with("#")
                            || (pt.contains("@")
                                && (pt.contains("$") || pt.contains("%") || pt.contains("#")));
                        let ohmyzsh = pt.contains("@")
                            && pt.len() < 120
                            && (pt.trim_end().ends_with("~") || pt.trim_end().ends_with("/"));
                        classic || ohmyzsh
                    };

                    if prompt_detected {
                        let last_command_time = self.state_manager.last_command_time();
                        let mut did_complete = false;
                        if let Some(history) = self.state_manager.command_history_mut() {
                            if let Some(last_block) = history.last_mut() {
                                if last_block.status == mosaicterm::models::ExecutionStatus::Running
                                {
                                    let elapsed =
                                        last_command_time.map(|t| t.elapsed()).unwrap_or_default();
                                    let mut exit_code =
                                        mosaicterm::pty::shell_state::read_exit_code(
                                            std::process::id(),
                                        )
                                        .unwrap_or(0);
                                    if exit_code == 0 {
                                        if let Some(err_code) =
                                            Self::detect_error_in_output(&last_block.output)
                                        {
                                            exit_code = err_code;
                                        }
                                    }
                                    last_block.mark_completed_with_code(elapsed, exit_code);
                                    did_complete = true;
                                    debug!(
                                        "Command completed (idle prompt check, exit {}): {}",
                                        exit_code, last_block.command
                                    );
                                }
                            }
                        }
                        if did_complete {
                            self.state_manager.clear_last_command_time();
                            should_update_contexts = true;
                        }
                    }
                }
            }
        }

        // Process-tree-based command completion: poll whether the shell
        // still has child processes.  When children disappear the command is done.
        if !should_update_contexts && !self.ssh_session_active {
            if let Some(terminal) = &self.terminal {
                if let Some(handle) = terminal.pty_handle() {
                    if let Some(shell_pid) = handle.pid {
                        let has_children =
                            mosaicterm::pty::shell_state::has_foreground_children(shell_pid);

                        if self.shell_had_children && !has_children {
                            // Transition: children → no children → command finished
                            debug!(
                                "Shell {} children gone, marking command complete",
                                shell_pid
                            );

                            let last_command_time = self.state_manager.last_command_time();
                            if let Some(history) = self.state_manager.command_history_mut() {
                                if let Some(last_block) = history.last_mut() {
                                    if last_block.status
                                        == mosaicterm::models::ExecutionStatus::Running
                                    {
                                        let elapsed = last_command_time
                                            .map(|t| t.elapsed())
                                            .unwrap_or_default();
                                        let elapsed_ms = elapsed.as_millis();

                                        let mut exit_code =
                                            mosaicterm::pty::shell_state::read_exit_code(
                                                std::process::id(),
                                            )
                                            .unwrap_or(0);
                                        if exit_code == 0 {
                                            if let Some(err_code) =
                                                Self::detect_error_in_output(&last_block.output)
                                            {
                                                exit_code = err_code;
                                            }
                                        }

                                        if exit_code == 0 {
                                            last_block.mark_completed(elapsed);
                                        } else {
                                            last_block.mark_failed(elapsed, exit_code);
                                        }
                                        debug!(
                                            "Command '{}' finished via process tree (exit {})",
                                            last_block.command, exit_code
                                        );

                                        if !self.window_has_focus
                                            && elapsed_ms >= bg_notification_threshold_ms
                                        {
                                            let cmd_short = if last_block.command.len() > 40 {
                                                format!("{}...", &last_block.command[..40])
                                            } else {
                                                last_block.command.clone()
                                            };
                                            let status_str = if exit_code == 0 {
                                                "completed"
                                            } else {
                                                "failed"
                                            };
                                            send_system_notification(
                                                "MosaicTerm",
                                                &format!(
                                                    "Command {} ({}s): {}",
                                                    status_str,
                                                    elapsed_ms / 1000,
                                                    cmd_short
                                                ),
                                            );
                                        }

                                        self.state_manager.clear_last_command_time();
                                        should_update_contexts = true;
                                    }
                                }
                            }
                        }
                        self.shell_had_children = has_children;
                    }
                }
            }
        }

        // After command completes, sync CWD and env context from the OS
        if should_update_contexts {
            self.sync_shell_state();
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
