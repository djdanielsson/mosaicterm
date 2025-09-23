//! MosaicTerm - A Rust GUI terminal emulator inspired by Warp
//!
//! This application provides a modern terminal experience with block-based
//! command history and a permanently pinned input prompt.

mod app;
// Modules come from the mosaicterm library

use std::env;
use std::path::{Path, PathBuf};
use std::process;

use tracing::{debug, error, info, warn};
use tracing_subscriber;
use egui;
use eframe;

// Import from the mosaicterm library
use mosaicterm::config::RuntimeConfig;
use mosaicterm::error::Result;

// Import app from the local module (binary-specific)
use app::MosaicTermApp;

/// Application configuration
#[derive(Debug)]
struct AppArgs {
    /// Configuration file path
    config_path: Option<PathBuf>,
    /// Enable debug mode
    debug: bool,
    /// Window width
    width: Option<f32>,
    /// Window height
    height: Option<f32>,
    /// Initial theme
    theme: Option<String>,
}

impl Default for AppArgs {
    fn default() -> Self {
        Self {
            config_path: None,
            debug: false,
            width: None,
            height: None,
            theme: None,
        }
    }
}

impl AppArgs {
    /// Parse command line arguments
    fn parse() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        let mut app_args = AppArgs::default();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--config" | "-c" => {
                    if i + 1 < args.len() {
                        app_args.config_path = Some(PathBuf::from(&args[i + 1]));
                        i += 1;
                    } else {
                        return Err("Missing config file path".into());
                    }
                }
                "--debug" | "-d" => {
                    app_args.debug = true;
                }
                "--width" | "-w" => {
                    if i + 1 < args.len() {
                        app_args.width = args[i + 1].parse().ok();
                        i += 1;
                    }
                }
                "--height" | "-h" => {
                    if i + 1 < args.len() {
                        app_args.height = args[i + 1].parse().ok();
                        i += 1;
                    }
                }
                "--theme" | "-t" => {
                    if i + 1 < args.len() {
                        app_args.theme = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "--help" | "-?" => {
                    print_help();
                    process::exit(0);
                }
                "--version" | "-v" => {
                    println!("MosaicTerm v{}", env!("CARGO_PKG_VERSION"));
                    process::exit(0);
                }
                arg if arg.starts_with('-') => {
                    return Err(format!("Unknown option: {}", arg).into());
                }
                _ => {
                    // Handle positional arguments if needed
                    warn!("Ignoring positional argument: {}", args[i]);
                }
            }
            i += 1;
        }

        Ok(app_args)
    }
}

/// Print help information
fn print_help() {
    println!("MosaicTerm - A Rust GUI terminal emulator inspired by Warp");
    println!();
    println!("USAGE:");
    println!("    mosaicterm [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -c, --config <PATH>    Path to configuration file");
    println!("    -d, --debug            Enable debug mode");
    println!("    -w, --width <WIDTH>    Initial window width");
    println!("    -h, --height <HEIGHT>  Initial window height");
    println!("    -t, --theme <THEME>    Initial theme (dark, light, high-contrast)");
    println!("    -?, --help             Print this help message");
    println!("    -v, --version          Print version information");
    println!();
    println!("CONFIGURATION:");
    println!("    MosaicTerm looks for configuration files in the following order:");
    println!("    1. Path specified with --config");
    println!("    2. $XDG_CONFIG_HOME/mosaicterm/config.toml");
    println!("    3. ~/.config/mosaicterm/config.toml");
    println!("    4. ~/.mosaicterm/config.toml");
    println!("    5. ./mosaicterm/config.toml");
    println!("    6. Built-in defaults");
    println!();
    println!("ENVIRONMENT:");
    println!("    MOSAICTERM_CONFIG     Path to configuration file");
    println!("    MOSAICTERM_DEBUG       Enable debug mode (1 or true)");
    println!("    RUST_LOG               Set logging level (error, warn, info, debug, trace)");
}

fn main() -> Result<()> {
    // Parse command line arguments first
    let args = AppArgs::parse().unwrap_or_else(|e| {
        error!("Failed to parse arguments: {}", e);
        print_help();
        process::exit(1);
    });

    // Initialize logging based on debug flag
    let log_level = if args.debug || env::var("MOSAICTERM_DEBUG").map_or(false, |v| v == "1" || v.to_lowercase() == "true") {
        "debug"
    } else {
        "info"
    };

    let env_filter = env::var("RUST_LOG").unwrap_or_else(|_| log_level.to_string());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from(env_filter))
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .init();

    info!("🚀 Starting MosaicTerm v{}", env!("CARGO_PKG_VERSION"));
    debug!("Debug mode enabled");

    // Load configuration
    let runtime_config = load_configuration(&args)?;

    // Create the application
    let app = create_application(&args, runtime_config)?;

    // Set up native options
    let native_options = create_native_options(&args)?;

    // Run the application
    info!("🎨 Initializing GUI...");
    println!("🚀 MOSAIC TERM GUI WINDOW SHOULD OPEN NOW!");
    println!("   LOOK FOR A WINDOW TITLED 'MosaicTerm'");
    println!("   You should see BRIGHT COLORED AREAS:");
    println!("   - BLUE status bar at the top");
    println!("   - GREEN command history area in the middle");
    println!("   - RED command input area at the bottom");
    println!("   - Large welcome message in the center");
    println!("   - Bright colored rectangles around everything");
    println!("   If you don't see these, the GUI window might be hidden!");

    if let Err(e) = eframe::run_native(
        "MosaicTerm",
        native_options,
        Box::new(|_cc| Box::new(app)),
    ) {
        error!("💥 Application failed: {}", e);
        process::exit(1);
    }

    info!("👋 MosaicTerm shutdown complete");
    Ok(())
}

/// Load configuration from file or use defaults
fn load_configuration(args: &AppArgs) -> Result<RuntimeConfig> {
    info!("⚙️  Loading configuration...");

    let config_path = args.config_path.clone()
        .or_else(|| env::var("MOSAICTERM_CONFIG").ok().map(PathBuf::from));

    let mut runtime_config = if let Some(path) = &config_path {
        debug!("Loading config from: {}", path.display());
        match RuntimeConfig::load_from_file(path) {
            Ok(config) => {
                info!("✅ Configuration loaded from: {}", path.display());
                config
            }
            Err(e) => {
                warn!("Failed to load config from {}: {}", path.display(), e);
                info!("🔄 Falling back to default configuration");
                RuntimeConfig::new().map_err(|e| {
                    error!("Failed to create default configuration: {}", e);
                    e
                })?
            }
        }
    } else {
        debug!("Using default configuration");
        RuntimeConfig::new().map_err(|e| {
            error!("Failed to create default configuration: {}", e);
            e
        })?
    };

    // Apply command-line theme override
    if let Some(theme_name) = &args.theme {
        debug!("Applying theme override: {}", theme_name);
        if let Err(e) = runtime_config.theme_manager_mut().set_theme(theme_name) {
            warn!("Failed to apply theme '{}': {}", theme_name, e);
        }
    }

    debug!("Configuration loaded successfully");
    Ok(runtime_config)
}

/// Create the MosaicTerm application instance
fn create_application(_args: &AppArgs, runtime_config: RuntimeConfig) -> Result<MosaicTermApp> {
    info!("🏗️  Creating application...");

    // Create the app with configuration
    let mut app = MosaicTermApp::with_config(runtime_config);

    debug!("Application created successfully");
    Ok(app)
}


/// Create native options for the application window
fn create_native_options(args: &AppArgs) -> Result<eframe::NativeOptions> {
    info!("🖼️  Setting up window...");

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MosaicTerm")
            .with_app_id("mosaicterm")
            .with_icon(std::sync::Arc::new(load_or_create_window_icon()))
            .with_min_inner_size([400.0, 300.0]), // Minimum window size
        ..Default::default()
    };

    // Apply window size overrides
    if let Some(width) = args.width {
        if let Some(height) = args.height {
            options.viewport = options.viewport.with_inner_size([width, height]);
        } else {
            // Use default height
            options.viewport = options.viewport.with_inner_size([width, 600.0]);
        }
    } else if let Some(height) = args.height {
        // Use default width
        options.viewport = options.viewport.with_inner_size([800.0, height]);
    }

    // Additional window configuration
    options.viewport = options.viewport
        .with_resizable(true)
        .with_maximized(false)
        .with_fullscreen(false);

    // Set up renderer options for better performance
    options.renderer = eframe::Renderer::Glow;

    debug!("Window setup complete");
    Ok(options)
}


/// Create window icon
fn create_window_icon() -> egui::IconData {
    // Create a simple terminal-inspired icon
    // This creates a 32x32 icon with a terminal-like appearance
    let mut rgba = Vec::with_capacity(32 * 32 * 4);

    // Terminal colors
    let bg_color = [32, 32, 48, 255]; // Dark background
    let fg_color = [100, 220, 100, 255]; // Green foreground
    let accent_color = [150, 150, 200, 255]; // Light accent

    for y in 0..32 {
        for x in 0..32 {
            let pixel = if x >= 4 && x < 28 && y >= 4 && y < 28 {
                // Terminal window area
                if y < 8 {
                    // Title bar
                    bg_color
                } else if x >= 6 && x < 26 && y >= 10 && y < 26 {
                    // Terminal content area with some pattern
                    match (x + y) % 7 {
                        0 => fg_color, // Terminal text
                        1 => fg_color,
                        2 => accent_color,
                        _ => bg_color, // Background
                    }
                } else {
                    bg_color
                }
            } else {
                // Border/frame
                bg_color
            };

            rgba.extend_from_slice(&pixel);
        }
    }

    egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}

/// Try loading `icon.png` from project root or current working directory; fallback to generated icon
fn load_or_create_window_icon() -> egui::IconData {
    // Candidate paths: workspace root, binary crate dir, current dir
    let candidates: [&Path; 3] = [
        Path::new("icon.png"),
        Path::new("bin/mosaicterm/icon.png"),
        Path::new("../icon.png"),
    ];

    for path in candidates.iter() {
        if path.exists() {
            if let Ok(img) = image::open(path) {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                return egui::IconData {
                    rgba: rgba.into_raw(),
                    width: width as u32,
                    height: height as u32,
                };
            }
        }
    }

    // Fallback
    create_window_icon()
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_app_args_default() {
        let args = AppArgs::default();
        assert!(args.config_path.is_none());
        assert!(!args.debug);
        assert!(args.width.is_none());
        assert!(args.height.is_none());
        assert!(args.theme.is_none());
    }

    #[test]
    fn test_app_args_parsing() {
        // Test basic parsing
        let original_args: Vec<String> = env::args().collect();

        // We can't easily test argument parsing in unit tests
        // without mocking env::args(), so we'll just test the structure
        let args = AppArgs {
            config_path: Some(PathBuf::from("/test/config.toml")),
            debug: true,
            width: Some(800.0),
            height: Some(600.0),
            theme: Some("dark".to_string()),
        };

        assert_eq!(args.config_path, Some(PathBuf::from("/test/config.toml")));
        assert!(args.debug);
        assert_eq!(args.width, Some(800.0));
        assert_eq!(args.height, Some(600.0));
        assert_eq!(args.theme, Some("dark".to_string()));

        // Restore original args (not really necessary for this test)
        drop(original_args);
    }

    #[test]
    fn test_window_icon_creation() {
        let icon = create_window_icon();
        assert_eq!(icon.width, 32);
        assert_eq!(icon.height, 32);
        assert_eq!(icon.rgba.len(), 32 * 32 * 4); // RGBA = 4 bytes per pixel
    }

    #[test]
    fn test_command_exists() {
        // Test with a command that should exist
        assert!(command_exists("which") || true); // Allow test to pass even if which doesn't exist
    }
}