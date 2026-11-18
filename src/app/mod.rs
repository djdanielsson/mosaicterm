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
//! - `poll_panes()`: Processes PTY output in the update loop
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
//! - **Async I/O:** Background task handles terminal lifecycle requests

// Submodules
mod async_ops;
mod commands;
mod context;
mod input;
#[allow(dead_code)]
pub mod pane_tree;
mod persistent;
mod prompt;
mod ssh;

use arboard::Clipboard;
use eframe::egui;
use futures::executor;
use mosaicterm::completion::CompletionProvider;
use mosaicterm::config::{prompt::PromptFormatter, RuntimeConfig, TerminalConfig};
use mosaicterm::context::ContextDetector;
use mosaicterm::error::Result;
use mosaicterm::models::{CommandBlock, ExecutionStatus};
use mosaicterm::pty::{PtyHandle, PtyManager};
use mosaicterm::session::persistent::{PersistentSession, SessionHost, SessionSnapshot};
use mosaicterm::state_manager::StateManager;
use mosaicterm::terminal::{
    osc::{ShellMark, ShellMarkKind, TerminalEvent, TerminalEventParser},
    program_status::{ProgramEvent, ProgramStatuses, FEATURE_REPLY},
    Terminal, TerminalFactory,
};
use mosaicterm::ui::{
    CommandBlocks, CompletionPopup, InputPrompt, MetricsPanel, ScrollableHistory,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

// Output size limits to prevent memory leaks
const MAX_OUTPUT_LINES_PER_COMMAND: usize = 10_000;
const MAX_LINE_LENGTH: usize = 10_000;

fn trim_output_tail(buffer: &mut String) {
    const MAX_PROMPT_BYTES: usize = 2048;
    let start = buffer.ceil_char_boundary(buffer.len().saturating_sub(MAX_PROMPT_BYTES));
    buffer.drain(..start);
}

fn truncate_output_line(line: &mut String) {
    if line.len() > MAX_LINE_LENGTH {
        line.truncate(line.floor_char_boundary(MAX_LINE_LENGTH));
        line.push_str("... [truncated]");
    }
}

// Atomic flags for native macOS menu bar actions
#[cfg(target_os = "macos")]
static NATIVE_MENU_ABOUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
static NATIVE_MENU_DEV: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
static NATIVE_MENU_PERF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Operations on a specific pane's terminal.
pub(crate) enum AsyncRequest {
    StartShell {
        pane_id: String,
        config: Box<TerminalConfig>,
        working_dir: PathBuf,
        launch: ShellLaunch,
    },
    ListSessions(SessionHost),
    EndSession(PersistentSession),
    CloseTerminal(Box<Terminal>),
    SendInterrupt {
        pane_id: String,
        handle: PtyHandle,
    },
}

pub(super) enum ShellLaunch {
    Native,
    Persistent {
        session: PersistentSession,
        create: bool,
    },
}

pub(super) struct StartedShell {
    terminal: Box<Terminal>,
    snapshot: Option<SessionSnapshot>,
}

pub(crate) enum AsyncResult {
    ShellStarted {
        pane_id: String,
        result: Result<StartedShell>,
    },
    SessionsListed(Result<Vec<PersistentSession>>),
    SessionEnded {
        session: PersistentSession,
        result: Result<()>,
    },
    InterruptSent {
        pane_id: String,
        result: Result<()>,
    },
}

#[derive(Default)]
struct PaneSession {
    terminal: Option<Box<Terminal>>,
    initializing: bool,
    persistent: Option<PersistentSession>,
    status_message: Option<String>,
    input_prompt: InputPrompt,
    prompt_segments: Vec<mosaicterm::config::prompt::PromptSegment>,
    pending_block: Option<String>,
    expect_command_echo: bool,
    interrupted: bool,
    clear_pending_block: bool,
    shell_had_children: bool,
    shell_markers_ready: bool,
    shell_startup_deadline: Option<std::time::Instant>,
    reported_cwd: Option<PathBuf>,
    terminal_events: TerminalEventParser,
    program_statuses: ProgramStatuses,
    pty_size: Option<(u16, u16)>,
    tui_overlay: mosaicterm::ui::TuiOverlay,
    ssh_prompt_overlay: mosaicterm::ui::SshPromptOverlay,
    ssh_prompt_buffer: String,
    ssh_session_active: bool,
    ssh_session_command: Option<String>,
    ssh_remote_prompt: Option<String>,
}

impl PaneSession {
    fn awaiting_shell_prompt(&self) -> bool {
        self.shell_startup_deadline
            .is_some_and(|deadline| std::time::Instant::now() < deadline)
    }
}

/// Main MosaicTerm application
pub struct MosaicTermApp {
    /// Centralized state manager - single source of truth
    state_manager: StateManager,
    pane: PaneSession,
    pane_tree: pane_tree::PaneTree,
    pane_focus_requested: bool,
    /// PTY manager for process management (with per-terminal locking)
    pty_manager: Arc<PtyManager>,
    /// UI components
    command_blocks: CommandBlocks,
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
    /// Tokio runtime for async operations
    #[allow(dead_code)]
    runtime: tokio::runtime::Runtime,
    /// Channel for sending async requests from UI to background
    async_tx: mpsc::UnboundedSender<AsyncRequest>,
    /// Channel for receiving async results from background to UI
    async_rx: mpsc::UnboundedReceiver<AsyncResult>,
    /// UI color theme (cached egui colors from config)
    ui_colors: mosaicterm::ui::UiColors,
    /// Tool availability detection (Phase 5)
    tool_availability: ToolAvailability,
    sessions: persistent::SessionPanel,
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

        // Spawn background task to handle async operations
        runtime.spawn(async move {
            async_ops::async_operation_loop(
                &mut request_rx,
                result_tx,
                pty_manager_clone,
                terminal_factory,
            )
            .await;
        });

        // Initialize StateManager and add demo commands directly
        let mut state_manager = StateManager::new();
        Self::add_demo_commands(&mut state_manager);
        let session_id = state_manager
            .active_session()
            .expect("default session")
            .id
            .clone();
        let pane_tree = pane_tree::PaneTree::new(pane_tree::Pane::new("pane-0".into(), session_id));

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

        Self {
            state_manager,
            pane: PaneSession {
                input_prompt,
                ..Default::default()
            },
            pane_tree,
            pane_focus_requested: true,
            pty_manager,
            command_blocks,
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
            runtime,
            async_tx: request_tx,
            async_rx: result_rx,
            ui_colors,
            tool_availability,
            sessions: persistent::SessionPanel::default(),
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

    fn initial_working_dir(config: &TerminalConfig) -> PathBuf {
        config
            .working_directory
            .as_ref()
            .filter(|dir| dir.is_dir())
            .cloned()
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .filter(|dir| dir != std::path::Path::new("/"))
            })
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn request_shell(&mut self, working_dir: PathBuf) {
        let launch = match &self.pane.persistent {
            Some(session) => ShellLaunch::Persistent {
                session: session.clone(),
                create: false,
            },
            None if self.runtime_config.config().session.persistence => ShellLaunch::Persistent {
                session: PersistentSession::new(self.configured_host()),
                create: true,
            },
            None => ShellLaunch::Native,
        };
        self.launch_shell(working_dir, launch);
    }

    fn launch_shell(&mut self, working_dir: PathBuf, launch: ShellLaunch) {
        let pane_id = self.pane_tree.active_id().to_string();
        let config = Box::new(self.runtime_config.config().terminal.clone());
        if let ShellLaunch::Persistent { session, .. } = &launch {
            self.pane.persistent = Some(session.clone());
        }
        let request = AsyncRequest::StartShell {
            pane_id,
            config,
            working_dir,
            launch,
        };
        if self.async_tx.send(request).is_ok() {
            self.pane.initializing = true;
            self.start_loading("Starting shell...");
        } else {
            self.set_status_message(Some("Could not start shell".into()));
        }
    }

    fn can_submit_command(&self) -> bool {
        self.pane.terminal.is_some()
            && !self.pane.initializing
            && !self.pane.awaiting_shell_prompt()
            && self.pane.pending_block.is_none()
    }

    async fn send_shell_line(&self, handle: &PtyHandle, input: &str) -> Result<()> {
        let enter = if cfg!(windows) { "\r" } else { "\n" };
        self.pty_manager
            .send_input(handle, format!("{input}{enter}").as_bytes())
            .await
    }

    /// Send an interactive response without adding it to command history.
    async fn send_plain_input(&self, input: &str) -> Result<()> {
        let handle = self
            .pane
            .terminal
            .as_ref()
            .and_then(|terminal| terminal.pty_handle())
            .ok_or(mosaicterm::error::Error::NoPtyHandleAvailable)?;
        self.send_shell_line(handle, input).await
    }

    /// Submit the original command to the shell that owns this pane.
    pub async fn handle_command_input(&mut self, command: String) -> Result<()> {
        if command.trim().is_empty() || !self.can_submit_command() {
            return Ok(());
        }

        let Some(terminal) = self.pane.terminal.as_mut() else {
            return Ok(());
        };
        let Some(handle) = terminal.pty_handle().cloned() else {
            return Ok(());
        };
        let working_dir = terminal.get_working_directory().to_path_buf();
        terminal.clear_pending_output();

        let mut command_block = CommandBlock::new(command.clone(), working_dir);
        let is_tui = !self.pane.ssh_session_active && self.is_tui_command(&command);
        if is_tui {
            command_block.mark_tui_mode();
        } else {
            command_block.mark_running();
        }

        let enter = if cfg!(windows) { "\r" } else { "\n" };
        let input = format!("{command}{enter}");
        self.pty_manager
            .send_input(&handle, input.as_bytes())
            .await?;

        if let Err(error) = self.history_manager.add(command.clone()) {
            warn!("Failed to add command to history: {error}");
        }

        if !self.pane.ssh_session_active && self.is_ssh_command(&command) {
            mosaicterm::security_audit::log_ssh_connection(&self.extract_ssh_host(&command));
            self.pane.ssh_session_command = Some(command.clone());
        }

        self.pane.pending_block = Some(command_block.id.clone());
        self.pane.expect_command_echo = true;
        self.pane.shell_had_children = false;
        self.state_manager.add_command_block(command_block);
        self.state_manager.set_last_command_time();
        self.set_status_message(None);

        if is_tui {
            self.pane.tui_overlay.start(command, handle.id);
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn is_cd_command(&self, command: &str) -> bool {
        commands::is_cd_command(command)
    }

    /// Update the prompt display based on current working directory
    fn update_prompt(&mut self) {
        self.pane.prompt_segments = prompt::build_prompt_segments(
            self.pane.terminal.as_deref(),
            &self.state_manager,
            &self.prompt_formatter,
            self.pane.ssh_session_active,
            self.pane.ssh_remote_prompt.as_deref(),
            self.pane.ssh_session_command.as_deref(),
        );
        let prompt_str: String = self
            .pane
            .prompt_segments
            .iter()
            .map(|s| s.text.clone())
            .collect();
        self.pane.input_prompt.set_prompt(&prompt_str);
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
        let git_context = if self.is_persistent_remote() {
            None
        } else {
            context::detect_git_context(self.pane.terminal.as_deref())
        };
        context::update_state_git_context(&mut self.state_manager, git_context);
    }

    /// Parse environment output and update contexts
    fn parse_env_output(&mut self, output: &str) {
        let working_dir = self
            .pane
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
        let Some(terminal) = self.pane.terminal.as_mut() else {
            return;
        };
        let pid = terminal.pty_handle().and_then(|handle| handle.pid);
        let snapshot = terminal
            .shell_integration()
            .and_then(|integration| integration.read_state());
        let cwd = self
            .pane
            .reported_cwd
            .take()
            .or_else(|| snapshot.as_ref().map(|(cwd, _)| cwd.clone()))
            .or_else(|| pid.and_then(mosaicterm::pty::shell_state::read_cwd));

        if let Some(cwd) = cwd {
            let previous = terminal.get_working_directory().to_path_buf();
            if cwd != previous {
                terminal.set_working_directory(cwd.clone());
                self.state_manager.set_previous_directory(Some(previous));
                if let Some(session) = self.state_manager.active_session_mut() {
                    session.working_directory = cwd.clone();
                }

                if self.tool_availability.zoxide && !self.is_persistent_remote() {
                    Self::zoxide_add_background(&cwd);
                }
            }
        }
        if !self.is_persistent_remote() {
            self.update_contexts();
        }
        if let Some((_, environment)) = snapshot {
            self.parse_env_output(&environment);
        }
        self.update_prompt();
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

    fn is_tui_command(&self, command: &str) -> bool {
        commands::is_tui_command(command, self.runtime_config.config())
    }

    fn is_interactive_command(&self, command: &str) -> bool {
        commands::is_interactive_command(command)
    }

    /// Extract the command name from a command line
    fn get_command_name(&self, command: &str) -> String {
        commands::get_command_name(command)
    }

    /// Update application state
    pub fn update_state(&mut self) {
        let terminal_ready = self.pane.terminal.is_some();
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
        self.pane.status_message = message.clone();
        self.state_manager.set_status_message(message);
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
                    ("Shell", &cfg.terminal.shell().0.display().to_string()),
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
                    .pane
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
                    ("TUI overlay", self.pane.tui_overlay.is_active().to_string()),
                    (
                        "SSH session",
                        self.pane.ssh_prompt_overlay.is_active().to_string(),
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
                            )
                        })
                };
                if let Some((command, status, output_lines)) = block_data {
                    // Create context menu
                    let mut menu_open = true;
                    egui::Window::new("Context Menu")
                        .fixed_pos(menu_pos)
                        .resizable(false)
                        .collapsible(false)
                        .title_bar(false)
                        .show(ctx, |ui| {
                            ui.set_min_width(150.0);

                            if ui
                                .add_enabled(
                                    self.can_submit_command(),
                                    egui::Button::new("🔄 Rerun Command"),
                                )
                                .clicked()
                            {
                                match executor::block_on(self.handle_command_input(command.clone()))
                                {
                                    Ok(()) => {
                                        self.pane.input_prompt.add_to_history(command.clone())
                                    }
                                    Err(error) => self.set_status_message(Some(error.to_string())),
                                }
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

        if !self.state_manager.is_initialization_attempted() {
            self.state_manager.set_initialization_attempted(true);
            self.initialize_session_layout();
        }
        self.poll_async_results();
        self.poll_panes();

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
            egui::Area::new(egui::Id::unique("loading_overlay"))
                .fixed_pos(egui::pos2(10.0, 10.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::new()
                        .fill(egui::Color32::from_rgba_premultiplied(30, 30, 40, 240))
                        .stroke(egui::Stroke::new(
                            1.0_f32,
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
        if self.sessions.open {
            self.render_sessions(ctx);
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
            if ctx.input(|i| {
                i.modifiers.ctrl
                    && i.modifiers.alt
                    && i.modifiers.shift
                    && i.key_pressed(egui::Key::D)
            }) {
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
        if self.pane.tui_overlay.is_active() {
            // Handle input for TUI app BEFORE rendering
            if let Some(input_data) = self.pane.tui_overlay.handle_input(ctx) {
                debug!(
                    "TUI overlay: Sending {} bytes of input to PTY",
                    input_data.len()
                );
                // Send input to PTY
                if let Some(_handle_id) = self.pane.tui_overlay.pty_handle() {
                    let pty_manager = &*self.pty_manager;
                    if let Some(terminal) = &self.pane.terminal {
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

            if let Some(size) = self.pane.tui_overlay.take_pending_resize() {
                Self::resize_session(&self.pty_manager, &mut self.pane, size);
            }

            // Render overlay UI (takes the full root ui)
            let overlay_closed = self.pane.tui_overlay.render(ctx, ui);
            if overlay_closed || self.pane.tui_overlay.has_exited() {
                if overlay_closed && self.pane.pending_block.is_some() {
                    self.handle_interrupt_command();
                }
                self.pane.tui_overlay.stop();
            }
        } else {
            // Main layout with scrollable history and pinned input
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(self.ui_colors.background)
                        .inner_margin(egui::Margin::same(2)),
                )
                .show(ui, |ui| {
                    if self.runtime_config.config().session.persistence
                        && ui.button("Sessions").clicked()
                    {
                        self.open_sessions();
                    }
                    self.render_panes(ui);
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
        if self.pane.ssh_prompt_overlay.is_active() {
            let should_close = self.pane.ssh_prompt_overlay.render(ctx);

            if should_close {
                // Check if user submitted input or cancelled
                if self.pane.ssh_prompt_overlay.was_cancelled() {
                    // User cancelled - send Ctrl+C to abort SSH
                    if let Some(terminal) = &self.pane.terminal {
                        if let Some(pty_handle) = terminal.pty_handle() {
                            let pty_manager = &*self.pty_manager;
                            executor::block_on(async {
                                let _ = pty_manager.send_input(pty_handle, &[3]).await;
                                // Ctrl+C
                            });
                            info!("SSH prompt cancelled, sent Ctrl+C");
                        }
                    }
                    self.pane.ssh_prompt_overlay.hide();
                } else if let Some(input) = self.pane.ssh_prompt_overlay.take_input() {
                    // Send user input to PTY with newline
                    if let Some(terminal) = &self.pane.terminal {
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
                                if self.pane.ssh_session_command.is_some()
                                    && !self.pane.ssh_session_active
                                {
                                    info!("Activating SSH session after authentication");
                                    self.pane.ssh_session_active = true;
                                    self.pane.ssh_prompt_buffer.clear();

                                    // Update status to show we're connected
                                    if let Some(cmd) = &self.pane.ssh_session_command {
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
                    self.pane.ssh_prompt_overlay.hide();
                }
            }
        }

        // Only repaint when needed to save CPU
        // Repaint if: command is running, has pending output, user input changed, or overlays active
        let needs_repaint = self.state_manager.last_command_time().is_some()
            || (self
                .pane
                .terminal
                .as_ref()
                .map(|t| t.has_pending_output())
                .unwrap_or(false))
            || self.completion_popup.is_visible()
            || self.pane.tui_overlay.is_active()
            || self.pane.ssh_prompt_overlay.is_active();

        if needs_repaint {
            // Repaint immediately for active operations
            ctx.request_repaint();

            // For TUI overlay or SSH prompt, request very fast repaints (16ms = ~60fps) for smooth updates
            if self.pane.tui_overlay.is_active() || self.pane.ssh_prompt_overlay.is_active() {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        } else {
            // Check again in 100ms for idle state (efficient polling)
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        info!("MosaicTerm application shutting down");
        self.save_session_layout();
        if let Some(terminal) = self.pane.terminal.take() {
            executor::block_on(terminal.close());
        }
        self.pane_tree.for_each_pane_mut(|pane| {
            if let Some(terminal) = pane.session.terminal.take() {
                executor::block_on(terminal.close());
            }
        });
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
        style.visuals.window_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(50, 50, 70));

        style.visuals.selection.bg_fill = self.ui_colors.selection;
        style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, self.ui_colors.accent);

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
                let segments = self.pane.prompt_segments.clone();
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

                let old_input = self.pane.input_prompt.current_input().to_string();
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
                                .return_key(None)
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

                if std::mem::take(&mut self.pane_focus_requested) {
                    input_response.request_focus();
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
                    && !self.pane.ssh_prompt_overlay.is_active()
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
                    self.pane.input_prompt.set_input(current_input.clone());
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
                                .pane
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
                                .map(|r| {
                                    r.primary.index >= egui::text::CharIndex(current_input.len())
                                })
                                .unwrap_or(true)
                        } else {
                            true
                        };

                        let accept_ghost = self.ghost_completion.is_some()
                            && (tab_pressed || (right_pressed && cursor_at_end));

                        if accept_ghost {
                            if let Some(ghost) = self.ghost_completion.take() {
                                self.pane.input_prompt.set_input(ghost);
                                self.state_manager.set_completion_just_applied(true);
                            }
                        } else if tab_pressed {
                            self.handle_tab_completion(&current_input, input_rect);
                        } else if up_pressed {
                            self.pane.input_prompt.navigate_history_previous();
                            self.ghost_completion = None;
                        } else if down_pressed {
                            self.pane.input_prompt.navigate_history_next();
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

                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let plain_input = ui.input(|i| i.modifiers.alt);
                    if input_response.has_focus() && enter {
                        if plain_input && self.pane.pending_block.is_some() {
                            match executor::block_on(self.send_plain_input(&current_input)) {
                                Ok(()) => {
                                    self.pane.input_prompt.clear_input();
                                    self.ghost_completion = None;
                                }
                                Err(error) => {
                                    self.set_status_message(Some(self.user_friendly_error(&error)))
                                }
                            }
                        } else if self.completion_popup.is_visible() {
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
                        } else if !current_input.trim().is_empty() && self.can_submit_command() {
                            let command = current_input.clone();
                            match executor::block_on(self.handle_command_input(command.clone())) {
                                Ok(()) => {
                                    self.pane.input_prompt.add_to_history(command);
                                    self.pane.input_prompt.clear_input();
                                    self.ghost_completion = None;
                                }
                                Err(error) => {
                                    self.set_status_message(Some(self.user_friendly_error(&error)))
                                }
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
        if self.is_persistent_remote() {
            return None;
        }

        if input.is_empty() {
            return None;
        }

        let working_dir = self
            .pane
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
        if self.is_persistent_remote() {
            return;
        }

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
                .pane
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
        let current_input = self.pane.input_prompt.current_input();
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

        self.pane.input_prompt.set_input(new_input);
    }

    /// Render the history search popup (Ctrl+R)
    fn render_history_search_popup(&mut self, ctx: &egui::Context, input_rect: egui::Rect) {
        // Request focus on search field if needed (before showing popup)
        if self.history_search_needs_focus {
            let search_id = egui::Id::unique("history_search_input");
            ctx.memory_mut(|mem| mem.request_focus(search_id));
            self.history_search_needs_focus = false;
        }

        // Position popup above the input
        let popup_width = input_rect.width().max(600.0);
        let popup_height = 400.0;
        let popup_x = input_rect.left();
        let popup_y = input_rect.top() - popup_height - 10.0;

        // Create popup above input
        egui::Area::new(egui::Id::unique("history_search"))
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(self.ui_colors.blocks.background)
                    .stroke(egui::Stroke::new(2.0_f32, self.ui_colors.accent))
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
                            let search_id = egui::Id::unique("history_search_input");
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
                                        self.pane.input_prompt.set_input(command.clone());
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
                                                self.pane.input_prompt.set_input(command.clone());
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

    fn activate_pane(&mut self, id: &str) {
        let previous = self.pane_tree.active_id().to_string();
        if id != previous {
            self.pane_tree.set_active(id);
            self.switch_pane_session(&previous);
        }
    }

    fn switch_pane_session(&mut self, previous: &str) {
        if let Some(pane) = self.pane_tree.find_pane_mut(previous) {
            pane.session = std::mem::take(&mut self.pane);
        }
        let active = self.pane_tree.active_pane_mut().expect("active pane");
        self.pane = std::mem::take(&mut active.session);
        self.state_manager
            .set_active_session(active.session_id.clone());
        self.state_manager
            .set_terminal_ready(self.pane.terminal.is_some());
        self.state_manager
            .set_status_message(self.pane.status_message.clone());
        if self.pane.initializing {
            self.start_loading("Starting shell...");
        } else {
            self.stop_loading();
        }
        self.update_prompt();
        self.pane_focus_requested = true;
        self.ghost_completion = None;
        self.completion_popup.hide();
        self.history_search_active = false;
        self.command_blocks
            .interaction_state_mut()
            .context_menu_block = None;
    }

    fn resize_session(pty_manager: &PtyManager, session: &mut PaneSession, size: (u16, u16)) {
        if session.pty_size == Some(size) {
            return;
        }
        let Some(terminal) = session.terminal.as_mut() else {
            return;
        };
        let Some(handle) = terminal.pty_handle() else {
            return;
        };
        let (rows, cols) = size;
        match executor::block_on(pty_manager.resize_pty(handle, rows, cols)) {
            Ok(()) => {
                session.pty_size = Some(size);
                terminal.resize(usize::from(rows), usize::from(cols));
            }
            Err(error) => warn!("Failed to resize PTY: {error}"),
        }
    }

    fn resize_pane(&mut self, pane_id: &str, size: (u16, u16)) {
        let session = if self.pane_tree.active_id() == pane_id {
            &mut self.pane
        } else if let Some(pane) = self.pane_tree.find_pane_mut(pane_id) {
            &mut pane.session
        } else {
            return;
        };
        if !session.tui_overlay.is_active() {
            Self::resize_session(&self.pty_manager, session, size);
        }
    }

    fn render_panes(&mut self, ui: &mut egui::Ui) {
        let panes = self.pane_tree.layout(ui.available_rect_before_wrap());
        if let Some(position) = ui.input(|i| {
            i.pointer
                .any_pressed()
                .then(|| i.pointer.interact_pos())
                .flatten()
        }) {
            if let Some((id, _)) = panes.iter().find(|(_, rect)| rect.contains(position)) {
                self.activate_pane(id);
            }
        }

        const INPUT_HEIGHT: f32 = 60.0;
        const STATUS_HEIGHT: f32 = 20.0;
        let font = egui::FontId::monospace(self.runtime_config.config().ui.font_size as f32);
        let (cell_width, cell_height) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'M'), fonts.row_height(&font)));
        for (id, rect) in panes {
            let rows = ((rect.height() - INPUT_HEIGHT - STATUS_HEIGHT) / cell_height)
                .floor()
                .max(1.0) as u16;
            let cols = (rect.width() / cell_width).floor().max(1.0) as u16;
            self.resize_pane(&id, (rows, cols));
            let active = id == self.pane_tree.active_id();
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(&id)
                    .max_rect(rect)
                    .layout(egui::Layout::bottom_up(egui::Align::LEFT)),
            );
            child.set_clip_rect(rect.intersect(ui.clip_rect()));
            if self.pane_tree.pane_count() > 1 {
                let color = if active {
                    self.ui_colors.accent
                } else {
                    self.ui_colors.status_bar.background
                };
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, color),
                    egui::StrokeKind::Inside,
                );
            }
            child.allocate_ui_with_layout(
                egui::vec2(rect.width(), INPUT_HEIGHT.min(rect.height())),
                egui::Layout::left_to_right(egui::Align::LEFT),
                |ui| {
                    if active {
                        self.render_fixed_input_area(ui);
                    } else if let Some(pane) = self.pane_tree.find_pane(&id) {
                        ui.label(pane.session.input_prompt.prompt_text());
                        let draft = pane.session.input_prompt.current_input();
                        if !draft.is_empty() {
                            ui.label(draft);
                        }
                    }
                },
            );
            child.allocate_ui_with_layout(
                egui::vec2(rect.width(), (rect.height() - INPUT_HEIGHT).max(0.0)),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| self.render_command_history_area(ui, &id, active),
            );
        }
    }

    /// Render the command history area above the input
    fn render_command_history_area(&mut self, ui: &mut egui::Ui, pane_id: &str, active: bool) {
        let pane = self.pane_tree.find_pane(pane_id).expect("rendered pane");
        let session = self
            .state_manager
            .get_session(&pane.session_id)
            .expect("pane session");
        let command_history = &session.command_history;
        let status = if active { &self.pane } else { &pane.session };
        let colors = self.ui_colors.clone();

        ui.vertical(|ui| {
            let status_frame = egui::Frame::new()
                .fill(colors.status_bar.background)
                .inner_margin(egui::Margin::symmetric(6, 2));

            status_frame.show(ui, |ui| {
                ui.set_height(16.0);
                ui.horizontal(|ui| {
                    let (status_text, status_color) = match status.status_message.as_deref() {
                        Some(msg) => (msg, colors.status_bar.ssh_indicator),
                        None if status.initializing || status.awaiting_shell_prompt() => {
                            ("Starting shell…", colors.status_bar.text)
                        }
                        None if status.terminal.is_none() => {
                            ("Shell closed", colors.status_bar.text)
                        }
                        None if status.pending_block.is_some() => {
                            ("Running", colors.status_bar.text)
                        }
                        None => ("Ready", colors.status_bar.text),
                    };
                    let response = ui.label(
                        egui::RichText::new(status_text)
                            .font(egui::FontId::monospace(11.0))
                            .color(status_color),
                    );

                    if status.pending_block.is_some() {
                        response.on_hover_text("Alt+Enter sends input to the running program");
                    }

                    if let Some((label, message)) = status.program_statuses.display() {
                        let response = ui.add(
                            egui::Label::new(
                                egui::RichText::new(label).monospace().color(colors.accent),
                            )
                            .truncate(),
                        );
                        if let Some(message) = message {
                            response.on_hover_text(message);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let history_len = command_history.len();
                        if history_len > 0 {
                            ui.label(
                                egui::RichText::new(format!("{} cmds", history_len))
                                    .font(egui::FontId::monospace(11.0))
                                    .color(colors.status_bar.text),
                            );
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
                        for (i, block) in command_history.iter().enumerate() {
                            if let Some((block_id, pos)) =
                                Self::render_single_command_block_static(ui, block, i, &colors)
                            {
                                if !active {
                                    continue;
                                }
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
                    0.5_f32,
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

    fn close_terminal(&self, terminal: Box<Terminal>) {
        if let Err(error) = self.async_tx.send(AsyncRequest::CloseTerminal(terminal)) {
            if let AsyncRequest::CloseTerminal(terminal) = error.0 {
                executor::block_on(terminal.close());
            }
        }
    }

    /// Install the completed launch in the pane that requested it.
    fn poll_async_results(&mut self) {
        while let Ok(result) = self.async_rx.try_recv() {
            match result {
                AsyncResult::ShellStarted { pane_id, result } => {
                    let active = pane_id == self.pane_tree.active_id();
                    let Some(pane) = self.pane_tree.find_pane_mut(&pane_id) else {
                        if let Ok(started) = result {
                            self.close_terminal(started.terminal);
                        }
                        continue;
                    };
                    let session = if active {
                        &mut self.pane
                    } else {
                        &mut pane.session
                    };
                    session.initializing = false;
                    let mut snapshot = None;
                    let old_terminal = match result {
                        Ok(started) => {
                            let terminal = started.terminal;
                            snapshot = started.snapshot;
                            if let Some(state) =
                                self.state_manager.get_session_mut(&pane.session_id)
                            {
                                state.working_directory =
                                    terminal.get_working_directory().to_path_buf();
                                state.shell_type = terminal.detected_shell();
                                state.pty_handle_id =
                                    terminal.pty_handle().map(|handle| handle.id.clone());
                            }
                            session.status_message = None;
                            session.pty_size = None;
                            session.shell_markers_ready = false;
                            // Wait for the first real prompt before recording command output.
                            // A shell that bypasses its startup hook can still use prompt detection.
                            session.shell_startup_deadline =
                                terminal.shell_integration().map(|_| {
                                    std::time::Instant::now() + std::time::Duration::from_secs(5)
                                });
                            session.reported_cwd = None;
                            session.terminal_events = TerminalEventParser::default();
                            session.program_statuses.clear();
                            session.terminal.replace(terminal)
                        }
                        Err(error) => {
                            session.status_message =
                                Some(format!("Failed to start shell: {error}"));
                            None
                        }
                    };
                    if let Some(terminal) = old_terminal {
                        self.close_terminal(terminal);
                    }
                    if let Some(snapshot) = snapshot {
                        self.apply_session_snapshot(&pane_id, snapshot);
                        self.save_session_layout();
                    }
                    if active {
                        self.stop_loading();
                        self.state_manager
                            .set_terminal_ready(self.pane.terminal.is_some());
                        self.state_manager
                            .set_status_message(self.pane.status_message.clone());
                        self.update_contexts();
                        self.update_prompt();
                    }
                }
                AsyncResult::SessionsListed(result) => self.finish_session_listing(result),
                AsyncResult::SessionEnded { session, result } => {
                    self.finish_session_end(&session, result)
                }
                AsyncResult::InterruptSent {
                    pane_id,
                    result: Err(error),
                } => {
                    let message = Some(format!("Failed to interrupt command: {error}"));
                    if pane_id == self.pane_tree.active_id() {
                        self.set_status_message(message);
                    } else if let Some(pane) = self.pane_tree.find_pane_mut(&pane_id) {
                        pane.session.status_message = message;
                    }
                }
                AsyncResult::InterruptSent { result: Ok(()), .. } => {}
            }
        }
    }

    fn poll_panes(&mut self) {
        self.poll_pane();
        let active_id = self.pane_tree.active_id().to_string();
        let active_session_id = self
            .pane_tree
            .active_pane()
            .expect("active pane")
            .session_id
            .clone();
        for id in self.pane_tree.all_pane_ids() {
            if id == active_id {
                continue;
            }
            let pane = self.pane_tree.find_pane_mut(&id).expect("pane in tree");
            if pane.session.terminal.is_none() {
                continue;
            }
            self.state_manager
                .set_active_session(pane.session_id.clone());
            std::mem::swap(&mut self.pane, &mut pane.session);
            self.poll_pane();
            let pane = self.pane_tree.find_pane_mut(&id).expect("pane in tree");
            std::mem::swap(&mut self.pane, &mut pane.session);
        }
        self.state_manager.set_active_session(active_session_id);
        self.state_manager
            .set_status_message(self.pane.status_message.clone());
    }

    fn poll_pane(&mut self) {
        let Some(handle) = self
            .pane
            .terminal
            .as_ref()
            .and_then(|terminal| terminal.pty_handle())
            .cloned()
        else {
            return;
        };

        // Bound per-frame work so a noisy process cannot starve other panes or the UI.
        for _ in 0..64 {
            match executor::block_on(self.pty_manager.try_read_output_now(&handle)) {
                Ok(data) if data.is_empty() => break,
                Ok(data) => self.process_pane_output(data),
                Err(
                    mosaicterm::error::Error::PtyStreamDisconnected
                    | mosaicterm::error::Error::PtyStreamsNotFound { .. },
                ) => {
                    let persistent = self.pane.persistent.is_some();
                    self.flush_pane_output();
                    if !persistent {
                        let exit_code = executor::block_on(self.pty_manager.exit_code(&handle));
                        self.finish_pending_block(exit_code, true);
                    }
                    self.end_ssh_session();
                    self.pane.program_statuses.clear();
                    if self.pane.tui_overlay.is_active() {
                        self.pane.tui_overlay.mark_exited();
                    }
                    if let Some(terminal) = self.pane.terminal.take() {
                        self.close_terminal(terminal);
                    }
                    if let Some(session) = self.state_manager.active_session_mut() {
                        session.pty_handle_id = None;
                    }
                    self.set_status_message(Some(if persistent {
                        "Disconnected. Reconnect from Sessions".to_string()
                    } else {
                        "Shell closed".to_string()
                    }));
                    return;
                }
                Err(error) => {
                    warn!("Could not read pane output: {error}");
                    return;
                }
            }
        }

        if self.pane.pending_block.is_some()
            && !self.integrated_shell_ready()
            && !self.pane.ssh_session_active
        {
            if let Some(pid) = handle.pid {
                let busy = mosaicterm::pty::shell_state::has_foreground_children(pid);
                if std::mem::replace(&mut self.pane.shell_had_children, busy) && !busy {
                    self.flush_pane_output();
                    let exit_code = self.fallback_exit_code();
                    self.finish_pending_block(exit_code, false);
                    self.sync_shell_state();
                }
            }
        }

        self.check_command_timeout(&handle);
    }

    fn integrated_shell_ready(&self) -> bool {
        self.pane.shell_markers_ready
    }

    fn accepts_shell_mark(&self, mark: &ShellMark) -> bool {
        match self
            .pane
            .terminal
            .as_ref()
            .and_then(|terminal| terminal.shell_integration())
        {
            Some(integration) => mark.token.as_deref() == Some(integration.token()),
            None => self.pane.ssh_session_command.is_none(),
        }
    }

    fn process_pane_output(&mut self, data: Vec<u8>) {
        let marks = self.pane.terminal_events.observe(&data);
        if marks.is_empty() {
            self.process_pane_output_chunk(data, false);
            return;
        }

        let mut start = 0;
        for (end, event) in marks {
            let completes_command = matches!(&event, TerminalEvent::Shell(mark)
                if matches!(mark.kind, ShellMarkKind::Finished(_)) && self.accepts_shell_mark(mark));
            self.process_pane_output_chunk(data[start..end].to_vec(), completes_command);
            self.apply_terminal_event(event);
            start = end;
        }
        if start < data.len() {
            self.process_pane_output_chunk(data[start..].to_vec(), false);
        }
    }

    fn apply_terminal_event(&mut self, event: TerminalEvent) {
        match event {
            TerminalEvent::Reset => self.pane.program_statuses.clear(),
            TerminalEvent::Program(ProgramEvent::Query) => {
                if let Some(handle) = self
                    .pane
                    .terminal
                    .as_ref()
                    .and_then(|terminal| terminal.pty_handle())
                {
                    let _ = executor::block_on(self.pty_manager.send_input(handle, FEATURE_REPLY));
                }
            }
            TerminalEvent::Program(event) => self.pane.program_statuses.apply(event),
            TerminalEvent::Directory(path) => {
                if self.pane.ssh_session_command.is_none() {
                    self.pane.reported_cwd = Some(path);
                    if self.pane.pending_block.is_none() {
                        self.sync_shell_state();
                    }
                }
            }
            TerminalEvent::Shell(mark) => {
                if matches!(mark.kind, ShellMarkKind::Prompt) {
                    self.pane.program_statuses.prompt_started();
                }
                if !self.accepts_shell_mark(&mark) {
                    return;
                }
                let first_marker = !self.pane.shell_markers_ready;
                self.pane.shell_markers_ready = true;
                self.pane.shell_startup_deadline = None;
                match mark.kind {
                    ShellMarkKind::Finished(code) => {
                        self.finish_pending_block(code, false);
                        self.end_ssh_session();
                        self.pane.ssh_session_command = None;
                        self.sync_shell_state();
                    }
                    ShellMarkKind::Prompt if first_marker => self.sync_shell_state(),
                    _ => {}
                }
            }
        }
    }

    fn process_pane_output_chunk(&mut self, data: Vec<u8>, completed: bool) {
        if self.pane.terminal.is_none() {
            return;
        }

        if self.pane.tui_overlay.is_active() {
            self.pane.tui_overlay.add_raw_output(&data);
            let reply = self.pane.tui_overlay.take_pty_replies();
            if !reply.is_empty() {
                if let Some(handle) = self
                    .pane
                    .terminal
                    .as_ref()
                    .and_then(|terminal| terminal.pty_handle())
                {
                    let _ = executor::block_on(self.pty_manager.send_input(handle, &reply));
                }
            }
            if completed {
                self.pane.tui_overlay.mark_exited();
            } else if !self.integrated_shell_ready() {
                if data.windows(8).any(|window| window == b"\x1b[?1049h")
                    || data.windows(6).any(|window| window == b"\x1b[?47h")
                {
                    self.pane.tui_overlay.note_alt_screen_enter();
                }
                if !self.pane.tui_overlay.in_grace_period()
                    && self.pane.tui_overlay.saw_alt_screen()
                    && (data.windows(8).any(|window| window == b"\x1b[?1049l")
                        || data.windows(6).any(|window| window == b"\x1b[?47l"))
                {
                    self.finish_pending_block(None, false);
                    self.pane.tui_overlay.mark_exited();
                }
            }
        } else {
            let text = String::from_utf8_lossy(&data);
            if !completed && self.handle_ssh_authentication(&text) {
                return;
            }
            let remote_prompt = if completed || self.pane.ssh_session_command.is_none() {
                None
            } else {
                Self::detect_remote_prompt_static(&text)
            };
            let remote_closed = self.pane.ssh_session_active
                && (Self::detect_ssh_session_end_static(&text)
                    || remote_prompt.as_ref().is_some_and(|prompt| {
                        Self::is_local_prompt_static(self.pane.ssh_remote_prompt.as_ref(), prompt)
                    }));
            if text.contains("\x1b[2J") {
                let pending = self.pane.pending_block.as_deref();
                if let Some(history) = self.state_manager.command_history_mut() {
                    history.retain(|block| Some(block.id.as_str()) == pending);
                    self.pane.clear_pending_block = history
                        .last()
                        .is_some_and(|block| block.command.trim() == "clear");
                }
            }

            let fallback = !self.integrated_shell_ready();
            let terminal = self
                .pane
                .terminal
                .as_mut()
                .expect("output belongs to a terminal");
            let mut lines = terminal
                .process_block_output(data, completed)
                .unwrap_or_default();
            let mut saw_prompt = false;
            let is_prompt = |text: &str| {
                remote_prompt
                    .as_ref()
                    .is_some_and(|prompt| text.trim() == prompt.trim())
                    || fallback && terminal.is_shell_prompt(text)
            };
            if lines.last().is_some_and(|line| is_prompt(&line.text)) {
                lines.pop();
                saw_prompt = true;
            }
            if terminal.peek_partial_line().is_some_and(is_prompt) {
                terminal.clear_pending_output();
                saw_prompt = true;
            }
            self.append_output_lines(lines);

            if remote_closed {
                // Integrated shells will report the local ssh command's status.
                self.end_ssh_session();
            } else if let Some(prompt) = remote_prompt {
                self.pane.ssh_session_active = true;
                self.pane.ssh_remote_prompt = Some(prompt);
                self.pane.ssh_prompt_buffer.clear();
                self.finish_pending_block(None, false);
                self.update_prompt();
            } else if saw_prompt {
                let exit_code = self.fallback_exit_code();
                self.finish_pending_block(exit_code, false);
                self.sync_shell_state();
            }
        }
    }

    /// Returns true when the output was consumed by the authentication overlay.
    fn handle_ssh_authentication(&mut self, text: &str) -> bool {
        if self.pane.ssh_session_command.is_none() {
            return false;
        }

        self.pane.ssh_prompt_buffer.push_str(text);
        trim_output_tail(&mut self.pane.ssh_prompt_buffer);
        if !self.pane.ssh_prompt_overlay.is_active() {
            if let Some((prompt_type, message)) =
                mosaicterm::ui::SshPromptOverlay::detect_ssh_prompt(&self.pane.ssh_prompt_buffer)
            {
                self.pane.ssh_prompt_overlay.show(prompt_type, message);
                self.pane.ssh_prompt_buffer.clear();
                return true;
            }
        }

        false
    }

    fn flush_pane_output(&mut self) {
        if let Some(terminal) = self.pane.terminal.as_mut() {
            let lines = terminal.flush_output();
            self.append_output_lines(lines);
        }
    }

    fn append_output_lines(&mut self, mut lines: Vec<mosaicterm::models::OutputLine>) {
        let Some(pending) = self.pane.pending_block.as_deref() else {
            return;
        };
        let block = self
            .state_manager
            .command_history_mut()
            .and_then(|history| history.iter_mut().rev().find(|block| block.id == pending));
        let Some(block) = block else {
            return;
        };

        if !lines.is_empty()
            && std::mem::take(&mut self.pane.expect_command_echo)
            && lines[0].text.trim() == block.command.trim()
        {
            lines.remove(0);
        }
        for line in &mut lines {
            truncate_output_line(&mut line.text);
        }
        let count = lines.len();
        block.add_output_lines(lines);
        if block.output.len() > MAX_OUTPUT_LINES_PER_COMMAND {
            let removed = block.output.len() - MAX_OUTPUT_LINES_PER_COMMAND + 1000;
            block.output.drain(0..removed);
            block.output.insert(
                0,
                mosaicterm::models::OutputLine::new(format!(
                    "... [truncated {removed} lines due to size limit] ..."
                )),
            );
        }
        self.state_manager.statistics_mut().total_output_lines += count;
    }

    fn fallback_exit_code(&self) -> Option<i32> {
        let pending = self.pane.pending_block.as_deref()?;
        let block = self
            .state_manager
            .get_command_history()
            .iter()
            .rev()
            .find(|block| block.id == pending)?;
        block.output.iter().find_map(|line| {
            let text = line.text.to_ascii_lowercase();
            if text.contains("command not found")
                || text.contains("no such file or directory")
                || text.contains("is not recognized")
            {
                Some(127)
            } else if text.contains("permission denied") || text.ends_with("is a directory") {
                Some(126)
            } else if text.contains("syntax error") {
                Some(2)
            } else {
                None
            }
        })
    }

    fn finish_pending_block(&mut self, exit_code: Option<i32>, shell_closed: bool) {
        let Some(id) = self.pane.pending_block.take() else {
            return;
        };
        let elapsed = self
            .state_manager
            .last_command_time()
            .map(|time| time.elapsed())
            .unwrap_or_default();
        self.state_manager.clear_last_command_time();
        self.pane.expect_command_echo = false;
        self.pane.shell_had_children = false;
        let interrupted = std::mem::take(&mut self.pane.interrupted);
        self.set_status_message(None);

        if let Some(history) = self.state_manager.command_history_mut() {
            if std::mem::take(&mut self.pane.clear_pending_block) {
                history.retain(|block| block.id != id);
            }
            if let Some(block) = history.iter_mut().rev().find(|block| block.id == id) {
                if !matches!(
                    block.status,
                    ExecutionStatus::Running | ExecutionStatus::TuiMode
                ) {
                    block.exit_code = exit_code;
                    return;
                }
                if interrupted && matches!(exit_code, None | Some(130)) {
                    block.mark_cancelled();
                    block.exit_code = exit_code;
                    block.execution_time = Some(elapsed);
                } else if let Some(code) = exit_code {
                    block.mark_completed_with_code(elapsed, code);
                } else {
                    block.status = if shell_closed {
                        ExecutionStatus::Failed
                    } else {
                        ExecutionStatus::Completed
                    };
                    block.execution_time = Some(elapsed);
                }
                if !self.window_has_focus && elapsed.as_secs() >= 10 {
                    let command = &block.command;
                    let short = command.floor_char_boundary(40);
                    let suffix = if short < command.len() { "..." } else { "" };
                    let status = if block.status == ExecutionStatus::Completed {
                        "completed"
                    } else {
                        "failed"
                    };
                    send_system_notification(
                        "MosaicTerm",
                        &format!(
                            "Command {status} ({}s): {}{suffix}",
                            elapsed.as_secs(),
                            &command[..short]
                        ),
                    );
                }
            }
        }
    }

    fn check_command_timeout(&mut self, handle: &PtyHandle) {
        let Some(elapsed) = self
            .state_manager
            .last_command_time()
            .map(|time| time.elapsed())
        else {
            return;
        };
        let Some(id) = self.pane.pending_block.as_deref() else {
            return;
        };
        let config = &self.runtime_config.config().terminal.timeout;
        let Some(block) = self
            .state_manager
            .get_command_history()
            .iter()
            .rev()
            .find(|block| block.id == id)
        else {
            return;
        };
        let interactive = self.is_interactive_command(&block.command)
            || self.is_ssh_command(&block.command)
            || block.status == ExecutionStatus::TuiMode
            || [" | ", " > ", " >> "]
                .iter()
                .any(|separator| block.command.contains(separator));
        let seconds = if interactive {
            config.interactive_command_timeout_secs
        } else {
            config.regular_command_timeout_secs
        };
        if seconds == 0 || elapsed.as_secs() < seconds {
            return;
        }
        let kill_on_timeout = config.kill_on_timeout && !self.pane.ssh_session_active;
        let id = id.to_string();
        self.state_manager.clear_last_command_time();
        self.state_manager.add_output_line(
            &id,
            mosaicterm::models::OutputLine::new(format!(
                "[Timeout: Command exceeded {seconds}s limit]"
            )),
        );
        self.state_manager
            .update_command_block_status(&id, ExecutionStatus::Failed);
        if kill_on_timeout {
            let _ = executor::block_on(self.pty_manager.send_input(handle, &[3]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = MosaicTermApp::new();
        assert!(app.pane.terminal.is_none()); // Terminal starts as None
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
    #[test]
    fn output_caps_keep_utf8_character_boundaries() {
        let mut prompt = format!("é{}", "x".repeat(2047));
        trim_output_tail(&mut prompt);
        assert_eq!(prompt, "x".repeat(2047));

        let mut line = format!("{}éending", "x".repeat(MAX_LINE_LENGTH - 1));
        truncate_output_line(&mut line);
        assert_eq!(
            line,
            format!("{}... [truncated]", "x".repeat(MAX_LINE_LENGTH - 1))
        );
    }

    #[cfg(unix)]
    fn poll_until(app: &mut MosaicTermApp, done: impl Fn(&MosaicTermApp) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done(app) {
            app.poll_async_results();
            app.poll_panes();
            assert!(
                std::time::Instant::now() < deadline,
                "timed out: {:?}",
                app.pane.status_message
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[cfg(unix)]
    fn run_command(app: &mut MosaicTermApp, command: &str) {
        assert!(app.can_submit_command());
        executor::block_on(app.handle_command_input(command.to_string())).unwrap();
        poll_until(app, |app| app.pane.pending_block.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn prompt_commands_keep_shell_syntax_and_panes_keep_separate_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("name with space");
        std::fs::create_dir(&subdir).unwrap();
        let mut app = MosaicTermApp::new();
        app.history_manager =
            mosaicterm::history::HistoryManager::with_path(dir.path().join("history")).unwrap();
        let config = &mut app.runtime_config.config_mut().terminal;
        config.shell_path = PathBuf::from("bash");
        config.shell_args = vec!["--noprofile".into(), "--noediting".into()];
        app.request_shell(dir.path().to_path_buf());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.pane.terminal.is_none() {
            app.poll_async_results();
            assert!(
                std::time::Instant::now() < deadline,
                "{:?}",
                app.pane.status_message
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        app.process_pane_output(b"startup output ending with a prompt-like $ ".to_vec());
        assert!(
            !app.can_submit_command(),
            "the shell has not sent its first prompt marker"
        );
        poll_until(&mut app, |app| app.can_submit_command());
        assert!(app.pane.shell_markers_ready);

        run_command(&mut app, "echo 'a b' | wc -w");
        let block = app.state_manager.get_command_history().last().unwrap();
        assert_eq!(block.exit_code, Some(0));
        // macOS bash prints a "default interactive shell is now zsh" banner on startup,
        // so the command's own output is the last non-empty line rather than the first.
        let last_output = block
            .output
            .iter()
            .rev()
            .map(|line| line.text.trim())
            .find(|text| !text.is_empty());
        assert_eq!(last_output, Some("2"));
        run_command(&mut app, "cd 'name with space'");
        assert_eq!(
            app.pane.terminal.as_ref().unwrap().get_working_directory(),
            subdir.canonicalize().unwrap()
        );
        run_command(&mut app, "false");
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .exit_code,
            Some(1)
        );
        run_command(
            &mut app,
            "alias echo='printf \"alias:%s\"'; LOCAL_VALUE=kept",
        );
        run_command(&mut app, "echo \"$LOCAL_VALUE\" > redirected");
        assert_eq!(
            std::fs::read_to_string(subdir.join("redirected")).unwrap(),
            "alias:kept"
        );
        run_command(&mut app, "printf 'command not found\\n'");
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .exit_code,
            Some(0)
        );

        executor::block_on(app.handle_command_input("printf 'started\\n'; sleep 30".into()))
            .unwrap();
        poll_until(&mut app, |app| {
            let block = app.state_manager.get_command_history().last().unwrap();
            block.output.iter().any(|line| line.text == "started")
        });
        app.handle_interrupt_command();
        poll_until(&mut app, |app| app.can_submit_command());
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .status,
            ExecutionStatus::Cancelled
        );

        let first_pane = app.pane_tree.active_id().to_owned();
        let first_session = app.pane_tree.active_pane().unwrap().session_id.clone();
        let first_handle = app
            .pane
            .terminal
            .as_ref()
            .unwrap()
            .pty_handle()
            .unwrap()
            .id
            .clone();
        executor::block_on(app.handle_command_input(
            "printf '\\033]7501;state=working:app=build\\a'; read -r reply; printf original".into(),
        ))
        .unwrap();
        poll_until(&mut app, |app| {
            app.pane.program_statuses.display().is_some()
        });
        app.handle_split_right();
        poll_until(&mut app, |app| app.can_submit_command());
        assert_ne!(
            app.pane.terminal.as_ref().unwrap().pty_handle().unwrap().id,
            first_handle
        );
        assert!(app.state_manager.get_command_history().is_empty());
        run_command(&mut app, "printf '%s' \"$LOCAL_VALUE\"");
        assert!(app
            .state_manager
            .get_command_history()
            .last()
            .unwrap()
            .output
            .is_empty());
        let second_pane = app.pane_tree.active_id().to_owned();
        app.resize_pane(&first_pane, (23, 68));
        app.resize_pane(&second_pane, (37, 106));
        run_command(&mut app, "stty size");
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output[0]
                .text,
            "37 106"
        );

        run_command(
            &mut app,
            "printf '\\033]133;D;77\\a\\033]7501;state=error:app=task\\a'; true",
        );
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .exit_code,
            Some(0)
        );
        assert_eq!(
            app.pane.program_statuses.display().unwrap().0,
            "task · Failed"
        );
        assert_eq!(
            app.pane_tree
                .find_pane(&first_pane)
                .unwrap()
                .session
                .program_statuses
                .display()
                .unwrap()
                .0,
            "build · Working"
        );

        app.activate_pane(&first_pane);
        executor::block_on(app.send_plain_input("continue")).unwrap();
        app.activate_pane(&second_pane);
        poll_until(&mut app, |app| {
            app.state_manager
                .get_session(&first_session)
                .unwrap()
                .command_history
                .last()
                .unwrap()
                .exit_code
                .is_some()
        });
        let background = app
            .state_manager
            .get_session(&first_session)
            .unwrap()
            .command_history
            .last()
            .unwrap();
        assert!(background.output.iter().any(|line| line.text == "original"));
        assert!(app
            .pane_tree
            .find_pane(&first_pane)
            .unwrap()
            .session
            .program_statuses
            .display()
            .is_none());
        app.handle_close_pane();
        assert_eq!(app.pane_tree.active_id(), first_pane);
        assert_eq!(
            app.pane.terminal.as_ref().unwrap().pty_handle().unwrap().id,
            first_handle
        );
        run_command(&mut app, "stty size");
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output[0]
                .text,
            "23 68"
        );
        poll_until(&mut app, |app| {
            executor::block_on(app.pty_manager.active_count()) == 1
        });
        executor::block_on(app.pane.terminal.take().unwrap().close());
    }
    #[cfg(unix)]
    #[test]
    fn persistent_layout_reattaches_two_live_shells_without_restarting_them() {
        if std::env::var_os("MOSAICTERM_TEST_PERSISTENCE").is_none() {
            return;
        }

        let root = tempfile::tempdir().unwrap();
        let layout_path = root.path().join("pane-layout.json");
        let mut app = MosaicTermApp::new();
        app.sessions.layout_path = Some(layout_path.clone());
        app.history_manager =
            mosaicterm::history::HistoryManager::with_path(root.path().join("history")).unwrap();
        let config = app.runtime_config.config_mut();
        config.session.persistence = true;
        config.session.auto_restore = true;
        config.terminal.shell_path = PathBuf::from("bash");
        config.terminal.shell_args.clear();
        config.terminal.working_directory = Some(root.path().to_path_buf());

        app.initialize_session_layout();
        poll_until(&mut app, |app| app.can_submit_command());
        run_command(
            &mut app,
            "mkdir pane-one; cd pane-one; printf 'one:%s\\n' \"$$\"",
        );
        let first_id = app.pane.persistent.clone().unwrap();
        let first_output = app
            .state_manager
            .get_command_history()
            .last()
            .unwrap()
            .output[0]
            .text
            .clone();
        executor::block_on(app.handle_command_input(
            "printf 'started\\n'; sleep 0.4; printf 'offline-output\\n'".into(),
        ))
        .unwrap();
        poll_until(&mut app, |app| {
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output
                .iter()
                .any(|line| line.text == "started")
        });

        app.handle_split_right();
        poll_until(&mut app, |app| app.can_submit_command());
        run_command(
            &mut app,
            "mkdir ../pane-two; cd ../pane-two; printf 'two:%s\\n' \"$$\"",
        );
        let second_id = app.pane.persistent.clone().unwrap();
        let second_output = app
            .state_manager
            .get_command_history()
            .last()
            .unwrap()
            .output[0]
            .text
            .clone();
        eframe::App::on_exit(&mut app, None);
        drop(app);

        let mut restored = MosaicTermApp::new();
        restored.sessions.layout_path = Some(layout_path);
        restored.history_manager =
            mosaicterm::history::HistoryManager::with_path(root.path().join("history")).unwrap();
        let config = restored.runtime_config.config_mut();
        config.session.persistence = true;
        config.session.auto_restore = true;
        config.terminal.shell_path = PathBuf::from("bash");
        config.terminal.shell_args.clear();
        std::thread::sleep(std::time::Duration::from_millis(500));
        restored.initialize_session_layout();
        poll_until(&mut restored, |app| app.can_submit_command());
        assert_eq!(restored.pane_tree.pane_count(), 2);
        assert_eq!(restored.pane.persistent.as_ref(), Some(&second_id));
        assert_eq!(
            restored
                .pane
                .terminal
                .as_ref()
                .unwrap()
                .get_working_directory(),
            root.path().join("pane-two")
        );
        run_command(&mut restored, "printf 'two:%s\\n' \"$$\"");
        assert_eq!(
            restored
                .state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output[0]
                .text,
            second_output
        );

        restored.handle_navigate_pane(pane_tree::Direction::Left);
        poll_until(&mut restored, |app| app.can_submit_command());
        assert_eq!(restored.pane.persistent.as_ref(), Some(&first_id));
        assert_eq!(
            restored
                .pane
                .terminal
                .as_ref()
                .unwrap()
                .get_working_directory(),
            root.path().join("pane-one")
        );
        assert!(restored
            .state_manager
            .get_command_history()
            .iter()
            .flat_map(|block| block.output.iter())
            .any(|line| line.text == "offline-output"));
        run_command(&mut restored, "printf 'one:%s\\n' \"$$\"");
        assert_eq!(
            restored
                .state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output[0]
                .text,
            first_output
        );
        eframe::App::on_exit(&mut restored, None);
        first_id.end().unwrap();
        second_id.end().unwrap();
        let sessions = SessionHost::default().list().unwrap();
        assert!(!sessions.contains(&first_id) && !sessions.contains(&second_id));
    }
    #[cfg(unix)]
    #[test]
    fn ssh_reconnect_keeps_the_shell_and_recovers_the_exit_status() {
        let Some(host) = std::env::var_os("MOSAICTERM_TEST_SSH_HOST") else {
            return;
        };
        let drop_file = std::env::var_os("MOSAICTERM_TEST_SSH_DROP").unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut app = MosaicTermApp::new();
        app.history_manager =
            mosaicterm::history::HistoryManager::with_path(root.path().join("history")).unwrap();
        app.runtime_config.config_mut().session.persistence = true;
        let session = PersistentSession::new(SessionHost {
            ssh: Some(host.into_string().unwrap()),
        });
        app.launch_shell(
            root.path().to_path_buf(),
            ShellLaunch::Persistent {
                session: session.clone(),
                create: true,
            },
        );
        poll_until(&mut app, |app| app.can_submit_command());
        assert!(session.host.list().unwrap().contains(&session));

        run_command(
            &mut app,
            "mkdir -p mosaicterm-fixture; cd mosaicterm-fixture; printf 'shell:%s\\n' \"$$\"",
        );
        let pid = app
            .state_manager
            .get_command_history()
            .last()
            .unwrap()
            .output[0]
            .text
            .clone();
        executor::block_on(app.handle_command_input(
            "printf 'starting\\n'; sleep 0.6; printf 'finished offline\\n'; false".into(),
        ))
        .unwrap();
        poll_until(&mut app, |app| {
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output
                .iter()
                .any(|line| line.text == "starting")
        });
        std::fs::write(&drop_file, b"disconnect").unwrap();
        poll_until(&mut app, |app| app.pane.terminal.is_none());
        assert!(app
            .pane
            .status_message
            .as_deref()
            .unwrap()
            .starts_with("Disconnected"));
        std::thread::sleep(std::time::Duration::from_millis(700));
        app.request_shell(root.path().to_path_buf());
        poll_until(&mut app, |app| !app.pane.initializing);
        assert!(
            app.can_submit_command(),
            "reconnect: {:?}, pending: {:?}, last blocks: {:?}",
            app.pane.status_message,
            app.pane.pending_block,
            app.state_manager
                .get_command_history()
                .iter()
                .rev()
                .take(3)
                .map(|block| (&block.command, block.exit_code))
                .collect::<Vec<_>>()
        );
        assert!(app
            .pane
            .terminal
            .as_ref()
            .unwrap()
            .get_working_directory()
            .ends_with("mosaicterm-fixture"));
        let history = app.state_manager.get_command_history();
        assert!(history
            .iter()
            .any(|block| block.exit_code == Some(1) && block.command.ends_with("false")));
        assert!(history
            .iter()
            .flat_map(|block| block.output.iter())
            .any(|line| line.text == "finished offline"));
        run_command(&mut app, "printf 'shell:%s\\n' \"$$\"");
        assert_eq!(
            app.state_manager
                .get_command_history()
                .last()
                .unwrap()
                .output[0]
                .text,
            pid
        );

        session.end().unwrap();
        poll_until(&mut app, |app| app.pane.terminal.is_none());
        app.request_shell(root.path().to_path_buf());
        poll_until(&mut app, |app| !app.pane.initializing);
        assert!(app.pane.terminal.is_none());
        assert!(
            app.pane
                .status_message
                .as_deref()
                .unwrap()
                .contains("Session ended"),
            "{:?}",
            app.pane.status_message
        );
    }
    #[cfg(unix)]
    #[test]
    fn ghostty_overlay_resizes_vim_and_returns_to_the_same_history() {
        if std::env::var_os("MOSAICTERM_GHOSTTY_VT").is_none() || !command_exists("vim") {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("sample.txt");
        let ready = root.path().join("ready");
        let size = root.path().join("size");
        std::fs::write(&source, "Unicode: 界 e\u{301}\n").unwrap();
        let mut app = MosaicTermApp::new();
        app.history_manager =
            mosaicterm::history::HistoryManager::with_path(root.path().join("history")).unwrap();
        let config = app.runtime_config.config_mut();
        config.terminal.shell_path = PathBuf::from("bash");
        config.terminal.shell_args.clear();
        config.tui_apps.fullscreen_commands.push("vim".into());
        app.request_shell(root.path().to_path_buf());
        poll_until(&mut app, |app| app.can_submit_command());
        run_command(&mut app, "printf 'history anchor\\n'");
        let anchor = app
            .state_manager
            .get_command_history()
            .last()
            .unwrap()
            .id
            .clone();
        executor::block_on(app.handle_command_input(format!(
            "vim -Nu NONE -i NONE -n -c \"call writefile(['ready'], '{}')\" '{}'",
            ready.display(),
            source.display()
        )))
        .unwrap();
        poll_until(&mut app, |_| ready.exists());
        assert!(app.pane.tui_overlay.is_active());

        let ctx = egui::Context::default();
        for area in [egui::vec2(640.0, 420.0), egui::vec2(900.0, 660.0)] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, area)),
                    ..Default::default()
                },
                |ui| {
                    let ctx = ui.ctx().clone();
                    app.pane.tui_overlay.render(&ctx, ui);
                },
            );
            let new_size = app.pane.tui_overlay.take_pending_resize().unwrap();
            MosaicTermApp::resize_session(&app.pty_manager, &mut app.pane, new_size);
        }
        let (rows, cols) = app.pane.pty_size.unwrap();
        let handle = app
            .pane
            .terminal
            .as_ref()
            .unwrap()
            .pty_handle()
            .unwrap()
            .clone();
        executor::block_on(
            app.pty_manager.send_input(
                &handle,
                format!(
                    ":call writefile([&lines . 'x' . &columns], '{}')\r",
                    size.display()
                )
                .as_bytes(),
            ),
        )
        .unwrap();
        poll_until(&mut app, |_| {
            std::fs::read_to_string(&size).is_ok_and(|text| text.trim() == format!("{rows}x{cols}"))
        });
        executor::block_on(app.pty_manager.send_input(&handle, b":q!\r")).unwrap();
        poll_until(&mut app, |app| app.pane.pending_block.is_none());
        assert!(app.pane.tui_overlay.has_exited());
        let history = app.state_manager.get_command_history();
        assert_eq!(history[history.len() - 2].id, anchor);
        assert_eq!(history.last().unwrap().exit_code, Some(0));
        executor::block_on(app.pane.terminal.take().unwrap().close());
    }
}
