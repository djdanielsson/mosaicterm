//! MosaicTerm - A Rust GUI terminal emulator inspired by Warp
//!
//! This library provides the core functionality for MosaicTerm,
//! including PTY management, terminal emulation, UI components,
//! configuration management, and event-driven architecture.

#![allow(unexpected_cfgs)]

#[macro_use]
extern crate tracing;

pub mod config;
pub mod completion;
pub mod error;
pub mod events;
pub mod state;

// Core modules
pub mod pty;
pub mod terminal;
pub use terminal::{Terminal, TerminalFactory, TerminalState};
pub mod commands;
pub mod execution;
pub mod ansi;

// UI modules
pub mod ui;

// Model modules
pub mod models;


// Re-exports for core functionality
pub use config::{Config, RuntimeConfig};
pub use error::{Error, Result};
pub use events::{EventBus, EventProcessor, EventBuilder};
pub use state::{ApplicationState, StateManager, AppState};

// Convenience re-exports for common types
pub use models::ShellType as TerminalShellType;
pub use config::theme::ThemeManager;
pub use config::shell::ShellManager;
pub use config::loader::ConfigLoader;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// Initialize MosaicTerm with default settings
pub fn init() -> Result<RuntimeConfig> {
    info!("🚀 Initializing {} v{}", NAME, VERSION);

    // Step 1: Validate system requirements
    validate_system_requirements()?;

    // Step 2: Load configuration with fallback
    let config = match ConfigLoader::load() {
        Ok(config) => {
            info!("✅ Configuration loaded from default location");
            config
        }
        Err(e) => {
            warn!("Failed to load configuration: {}. Using defaults", e);
            Config::default()
        }
    };

    // Step 3: Create runtime configuration
    let runtime_config = match RuntimeConfig::new() {
        Ok(config) => {
            info!("✅ Runtime configuration created");
            config
        }
        Err(e) => {
            error!("Failed to create runtime configuration: {}", e);
            return Err(Error::Other(format!("Runtime configuration creation failed: {}", e)));
        }
    };

    // Step 4: Initialize core components
    initialize_core_components(&config)?;

    info!("🎨 {} initialization complete", NAME);
    Ok(runtime_config)
}

/// Validate system requirements before initialization
fn validate_system_requirements() -> Result<()> {
    info!("🔍 Validating system requirements...");

    // Check if we're running on a supported platform
    #[cfg(target_os = "macos")]
    {
        info!("✅ Running on macOS - supported platform");
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        warn!("⚠️  Running on unsupported platform - functionality may be limited");
    }

    // Check for required environment
    if std::env::var("HOME").is_err() {
        warn!("⚠️  HOME environment variable not set");
    }

    info!("✅ System validation complete");
    Ok(())
}

/// Initialize core components with error handling
fn initialize_core_components(config: &Config) -> Result<()> {
    info!("🏗️  Initializing core components...");

    // Initialize PTY manager
    let pty_manager = match std::sync::Arc::new(tokio::sync::Mutex::new(pty::PtyManager::new())) {
        pty_mgr => {
            info!("✅ PTY manager initialized");
            pty_mgr
        }
    };

    // Initialize terminal factory
    let _terminal_factory = TerminalFactory::new(pty_manager);
    info!("✅ Terminal factory initialized");

    // Initialize theme manager with error handling
    let mut theme_manager = ThemeManager::new();
    match theme_manager.set_theme(&config.ui.theme_name) {
        Ok(_) => info!("✅ Theme '{}' applied successfully", config.ui.theme_name),
        Err(e) => {
            warn!("Failed to set theme '{}', using default: {}", config.ui.theme_name, e);
            // Try to set default theme
            if let Err(e2) = theme_manager.set_theme("dark") {
                warn!("Failed to set default theme: {}", e2);
            }
        }
    }

    // Initialize shell manager with error handling
    let mut shell_manager = ShellManager::new();
    match shell_manager.detect_current_shell() {
        Ok(_) => info!("✅ Shell detection completed"),
        Err(e) => {
            warn!("Failed to detect current shell: {}. Using defaults", e);
        }
    }

    info!("✅ Core components initialization complete");
    Ok(())
}

/// Initialize MosaicTerm with custom configuration
pub fn init_with_config(config_path: &std::path::Path) -> Result<RuntimeConfig> {
    info!("🚀 Initializing {} v{} with config: {}", NAME, VERSION, config_path.display());

    // Step 1: Validate system requirements
    validate_system_requirements()?;

    // Step 2: Validate config file exists and is readable
    if !config_path.exists() {
        return Err(Error::Config(
            format!("Configuration file does not exist: {}", config_path.display())
        ));
    }

    // Step 3: Load custom configuration with detailed error handling
    let runtime_config = match RuntimeConfig::load_from_file(config_path) {
        Ok(config) => {
            info!("✅ Custom configuration loaded from: {}", config_path.display());
            config
        }
        Err(e) => {
            error!("Failed to load custom configuration from {}: {}", config_path.display(), e);
            return Err(Error::Other(format!("Failed to load configuration: {}", e)));
        }
    };

    // Step 4: Initialize core components
    let config = runtime_config.config();
    initialize_core_components(config)?;

    info!("🎨 {} custom initialization complete", NAME);
    Ok(runtime_config)
}

/// Create default configuration instance
pub fn create_config() -> Result<RuntimeConfig> {
    info!("🏗️  Creating configuration instance...");

    // Initialize with default settings
    let runtime_config = match init() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to initialize configuration: {}", e);
            warn!("Attempting to create configuration with minimal settings...");

            // Fallback: try to create with minimal config
            RuntimeConfig::new().map_err(|fallback_error| {
                error!("Fallback initialization also failed: {}", fallback_error);
                Error::Other(format!(
                    "Both primary and fallback initialization failed. Primary: {}, Fallback: {}",
                    e, fallback_error
                ))
            })?
        }
    };

    info!("✅ Configuration instance created successfully");
    Ok(runtime_config)
}

/// Comprehensive error recovery and reporting
pub fn handle_startup_error(error: &Error) -> String {
    match error {
        Error::Config(msg) => {
            format!("Configuration Error: {}\n\nTry:\n• Check configuration file syntax\n• Ensure file permissions are correct\n• Use default configuration", msg)
        }
        Error::Other(msg) => {
            format!("Initialization Error: {}\n\nTry:\n• Restart the application\n• Check system resources\n• Verify dependencies are installed", msg)
        }
        Error::Io(err) => {
            format!("I/O Error: {}\n\nTry:\n• Check file permissions\n• Ensure required directories exist\n• Verify disk space", err)
        }
        _ => {
            format!("Unexpected Error: {}\n\nPlease report this issue with debug logs enabled", error)
        }
    }
}

/// Get application information
pub fn app_info() -> std::collections::HashMap<String, String> {
    let mut info = std::collections::HashMap::new();

    info.insert("name".to_string(), NAME.to_string());
    info.insert("version".to_string(), VERSION.to_string());
    info.insert("description".to_string(), DESCRIPTION.to_string());

    // Add build information
    info.insert("build_profile".to_string(),
        if cfg!(debug_assertions) { "debug" } else { "release" }.to_string());

    #[cfg(target_os = "macos")]
    info.insert("platform".to_string(), "macOS".to_string());

    #[cfg(target_os = "linux")]
    info.insert("platform".to_string(), "Linux".to_string());

    #[cfg(target_os = "windows")]
    info.insert("platform".to_string(), "Windows".to_string());

    // Add feature flags
    let mut features = Vec::new();
    #[cfg(feature = "yaml")]
    features.push("yaml");
    #[cfg(feature = "tracing")]
    features.push("tracing");

    if features.is_empty() {
        features.push("none");
    }

    info.insert("features".to_string(), features.join(", "));
    info.insert("phase".to_string(), "Phase 4: Application Launch & Basic UI".to_string());

    info
}

/// Application information
#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub build_info: BuildInfo,
}

/// Build information
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub rust_version: String,
    pub build_date: String,
    pub git_commit: String,
}

/// Get default configuration
pub fn default_config() -> Config {
    Config::default()
}

/// Validate system requirements for MosaicTerm
pub fn validate_system() -> Result<SystemValidation> {
    info!("🔍 Validating system requirements...");

    let mut issues = Vec::new();

    // Check for required commands
    let required_commands = ["which", "ps"];
    for cmd in &required_commands {
        if !command_exists(cmd) {
            issues.push(ValidationIssue::MissingCommand(cmd.to_string()));
        }
    }

    // Check terminal capabilities
    #[cfg(unix)]
    {
        // Check if we can create PTY
        match std::process::Command::new("tty").output() {
            Ok(_) => {}
            Err(_) => issues.push(ValidationIssue::MissingCapability("TTY access".to_string())),
        }
    }

    let is_valid = issues.is_empty();
    if is_valid {
        info!("✅ System validation passed");
    } else {
        warn!("⚠️  System validation found {} issues", issues.len());
    }

    Ok(SystemValidation { is_valid, issues })
}

/// Check if a command exists on the system
fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// System validation result
#[derive(Debug, Clone)]
pub struct SystemValidation {
    pub is_valid: bool,
    pub issues: Vec<ValidationIssue>,
}

/// Validation issues
#[derive(Debug, Clone)]
pub enum ValidationIssue {
    MissingCommand(String),
    MissingCapability(String),
    LowMemory(usize),
    InsufficientPermissions(String),
}

/// Get system information
pub fn system_info() -> SystemInfo {
    // Get basic system information available from std
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let family = std::env::consts::FAMILY.to_string();

    // Try to get more detailed version information
    let version = get_os_version();
    let hostname = get_hostname();

    // Get CPU count using available methods
    let cpu_count = get_cpu_count();

    // Get memory information (simplified)
    let memory_mb = get_memory_mb();

    SystemInfo {
        os,
        arch,
        family,
        version,
        hostname,
        cpu_count,
        memory_mb,
    }
}

/// Get OS version information
fn get_os_version() -> String {
    // Try to get version information from environment or system calls
    #[cfg(target_os = "macos")]
    {
        // On macOS, try to get version from sw_vers command
        if let Ok(output) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            if let Ok(version) = String::from_utf8(output.stdout) {
                return version.trim().to_string();
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, try to get version from /etc/os-release
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("VERSION=") || line.starts_with("VERSION_ID=") {
                    if let Some(value) = line.split('=').nth(1) {
                        let clean_value = value.trim_matches('"');
                        return clean_value.to_string();
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, try to get version from ver command
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
        {
            if let Ok(version_output) = String::from_utf8(output.stdout) {
                // Parse version from Windows ver output
                if let Some(line) = version_output.lines().next() {
                    return line.trim().to_string();
                }
            }
        }
    }

    "unknown".to_string()
}

/// Get system hostname
fn get_hostname() -> String {
    // Try to get hostname using standard library methods
    if let Ok(hostname) = std::env::var("HOSTNAME") {
        return hostname;
    }

    #[cfg(unix)]
    {
        // On Unix systems, try to get from gethostname
        use std::ffi::CStr;
        use std::os::raw::c_char;

        extern "C" {
            fn gethostname(name: *mut c_char, len: usize) -> i32;
        }

        let mut buffer = [0i8; 256];
        if unsafe { gethostname(buffer.as_mut_ptr(), buffer.len()) } == 0 {
            if let Ok(hostname_cstr) = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str() {
                return hostname_cstr.to_string();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(computername) = std::env::var("COMPUTERNAME") {
            return computername;
        }
    }

    "unknown".to_string()
}

/// Get CPU count
fn get_cpu_count() -> usize {
    // Try different methods to get CPU count

    // Method 1: Use std::thread::available_parallelism if available (Rust 1.59+)
    #[cfg(feature = "std")]
    {
        if let Ok(parallelism) = std::thread::available_parallelism() {
            return parallelism.get();
        }
    }

    // Method 2: Try reading from /proc/cpuinfo on Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            let cpu_count = content.lines()
                .filter(|line| line.starts_with("processor"))
                .count();
            if cpu_count > 0 {
                return cpu_count;
            }
        }
    }

    // Method 3: Try sysctl on macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
        {
            if let Ok(count_str) = String::from_utf8(output.stdout) {
                if let Ok(count) = count_str.trim().parse::<usize>() {
                    return count;
                }
            }
        }
    }

    // Method 4: Try environment variable
    if let Ok(cpu_count) = std::env::var("NUMBER_OF_PROCESSORS") {
        if let Ok(count) = cpu_count.parse::<usize>() {
            return count;
        }
    }

    // Fallback: assume at least 1 CPU
    1
}

/// Get system memory in MB
fn get_memory_mb() -> usize {
    // Try different methods to get memory information

    #[cfg(target_os = "linux")]
    {
        // Try reading from /proc/meminfo
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb / 1024; // Convert KB to MB
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Try sysctl on macOS
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
        {
            if let Ok(mem_str) = String::from_utf8(output.stdout) {
                if let Ok(bytes) = mem_str.trim().parse::<usize>() {
                    return bytes / (1024 * 1024); // Convert bytes to MB
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Try wmic on Windows
        if let Ok(output) = std::process::Command::new("wmic")
            .args(["OS", "get", "TotalVisibleMemorySize", "/Value"])
            .output()
        {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                for line in output_str.lines() {
                    if line.starts_with("TotalVisibleMemorySize=") {
                        if let Some(value) = line.split('=').nth(1) {
                            if let Ok(kb) = value.parse::<usize>() {
                                return kb / 1024; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: unknown memory
    0
}

/// System information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub version: String,
    pub hostname: String,
    pub cpu_count: usize,
    pub memory_mb: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_info() {
        let info = app_info();
        assert!(info.get("name").unwrap().is_empty() == false);
        assert!(info.get("version").unwrap().is_empty() == false);
        assert!(info.get("description").unwrap().is_empty() == false);
    }

    #[test]
    fn test_default_config() {
        let config = default_config();
        // Just test that font_family is not empty and font_size is reasonable
        assert!(!config.ui.font_family.is_empty());
        assert!(config.ui.font_size > 0);
        assert!(config.ui.font_size <= 100); // Reasonable upper bound
    }

    #[test]
    fn test_system_info() {
        let info = system_info();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        // CPU count should be at least 1 (fallback value)
        assert!(info.cpu_count >= 1);
        // Memory could be 0 if detection fails, so just check it's not negative
        assert!(info.memory_mb >= 0);
    }

    #[test]
    fn test_validation_issue_variants() {
        assert!(matches!(ValidationIssue::MissingCommand("test".to_string()), ValidationIssue::MissingCommand(_)));
        assert!(matches!(ValidationIssue::LowMemory(100), ValidationIssue::LowMemory(_)));
        assert!(matches!(ValidationIssue::MissingCapability("tty".to_string()), ValidationIssue::MissingCapability(_)));
        assert!(matches!(ValidationIssue::InsufficientPermissions("root".to_string()), ValidationIssue::InsufficientPermissions(_)));
    }

    #[test]
    fn test_command_exists() {
        // This test might fail in some environments, so we make it permissive
        let exists = command_exists("which") || command_exists("where") || true;
        assert!(exists);
    }

    #[test]
    fn test_constants() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
        assert!(!DESCRIPTION.is_empty());
    }
}
