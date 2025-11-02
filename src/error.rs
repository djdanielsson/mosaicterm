//! Error types and Result aliases for MosaicTerm

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// Result type alias for MosaicTerm operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for MosaicTerm
#[derive(Debug)]
pub enum Error {
    // === PTY-related errors ===
    /// Failed to create PTY
    PtyCreationFailed {
        command: String,
        reason: String,
    },

    /// Failed to spawn command in PTY
    CommandSpawnFailed {
        command: String,
        reason: String,
    },

    /// PTY handle not found
    PtyHandleNotFound {
        handle_id: String,
    },

    /// Failed to clone PTY reader
    PtyReaderCloneFailed {
        reason: String,
    },

    /// Failed to take PTY writer
    PtyWriterTakeFailed {
        reason: String,
    },

    /// Failed to send input to PTY
    PtyInputSendFailed {
        reason: String,
    },

    /// Failed to read from PTY
    PtyReadFailed {
        reason: String,
    },

    /// PTY streams not found
    PtyStreamsNotFound {
        handle_id: String,
    },

    /// Process not registered with signal manager
    ProcessNotRegistered {
        handle_id: String,
    },

    /// Failed to send signal to process
    SignalSendFailed {
        signal: String,
        reason: String,
    },

    /// Signal handling not supported on platform
    SignalNotSupported {
        signal: String,
        platform: String,
    },

    /// PTY handle is invalid
    InvalidPtyHandle,

    /// No PID available for PTY
    NoPidAvailable {
        handle_id: String,
    },

    // === Command errors ===
    /// Command not found in PATH
    CommandNotFound {
        command: String,
    },

    /// Command validation failed
    CommandValidationFailed {
        command: String,
        reason: String,
    },

    /// Command timeout
    CommandTimeout {
        command: String,
        duration: Duration,
    },

    /// Empty command
    EmptyCommand,

    /// No previous command in history
    NoPreviousCommand,

    // === Configuration errors ===
    /// Failed to load configuration file
    ConfigLoadFailed {
        path: PathBuf,
        reason: String,
    },

    /// Failed to save configuration file
    ConfigSaveFailed {
        path: PathBuf,
        reason: String,
    },

    /// Configuration file not found
    ConfigNotFound,

    /// Configuration validation failed
    ConfigValidationFailed {
        field: String,
        reason: String,
    },

    /// Failed to serialize configuration
    ConfigSerializationFailed {
        format: String,
        reason: String,
    },

    /// Failed to parse configuration
    ConfigParseFailed {
        format: String,
        reason: String,
    },

    /// Shell configuration not found
    ShellConfigNotFound {
        shell_type: String,
    },

    /// Theme not found
    ThemeNotFound {
        theme_name: String,
    },

    /// Theme already exists
    ThemeAlreadyExists {
        theme_name: String,
    },

    /// Cannot remove built-in theme
    CannotRemoveBuiltInTheme {
        theme_name: String,
    },

    /// Failed to export theme
    ThemeExportFailed {
        theme_name: String,
        reason: String,
    },

    /// Failed to import theme
    ThemeImportFailed {
        reason: String,
    },

    /// Unknown component
    UnknownComponent {
        component: String,
    },

    /// Unknown color scheme
    UnknownColorScheme {
        scheme: String,
    },

    // === Terminal errors ===
    /// No PTY handle available
    NoPtyHandleAvailable,

    /// Output buffer full
    OutputBufferFull {
        command: String,
        size: usize,
    },

    // === I/O and serialization errors (kept for compatibility) ===
    /// I/O errors
    Io(std::io::Error),

    /// Serialization errors
    Serde(serde_json::Error),

    /// TOML parsing errors
    Toml(toml::de::Error),

    /// Regex compilation errors
    Regex(regex::Error),

    // === Generic fallback (use sparingly) ===
    /// Generic errors (for cases not yet categorized)
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // PTY errors
            Error::PtyCreationFailed { command, reason } => {
                write!(f, "Failed to create PTY for command '{}': {}", command, reason)
            }
            Error::CommandSpawnFailed { command, reason } => {
                write!(f, "Failed to spawn command '{}': {}", command, reason)
            }
            Error::PtyHandleNotFound { handle_id } => {
                write!(f, "PTY handle '{}' not found", handle_id)
            }
            Error::PtyReaderCloneFailed { reason } => {
                write!(f, "Failed to clone PTY reader: {}", reason)
            }
            Error::PtyWriterTakeFailed { reason } => {
                write!(f, "Failed to take PTY writer: {}", reason)
            }
            Error::PtyInputSendFailed { reason } => {
                write!(f, "Failed to send input to PTY: {}", reason)
            }
            Error::PtyReadFailed { reason } => {
                write!(f, "Failed to read from PTY: {}", reason)
            }
            Error::PtyStreamsNotFound { handle_id } => {
                write!(f, "PTY streams for '{}' not found", handle_id)
            }
            Error::ProcessNotRegistered { handle_id } => {
                write!(f, "Process '{}' not registered", handle_id)
            }
            Error::SignalSendFailed { signal, reason } => {
                write!(f, "Failed to send signal '{}': {}", signal, reason)
            }
            Error::SignalNotSupported { signal, platform } => {
                write!(f, "Signal '{}' not supported on {}", signal, platform)
            }
            Error::InvalidPtyHandle => {
                write!(f, "Invalid PTY handle")
            }
            Error::NoPidAvailable { handle_id } => {
                write!(f, "No PID available for PTY '{}'", handle_id)
            }

            // Command errors
            Error::CommandNotFound { command } => {
                write!(f, "Command '{}' not found in PATH", command)
            }
            Error::CommandValidationFailed { command, reason } => {
                write!(f, "Command validation failed for '{}': {}", command, reason)
            }
            Error::CommandTimeout { command, duration } => {
                write!(f, "Command '{}' timed out after {:?}", command, duration)
            }
            Error::EmptyCommand => {
                write!(f, "Command cannot be empty")
            }
            Error::NoPreviousCommand => {
                write!(f, "No previous command in history")
            }

            // Configuration errors
            Error::ConfigLoadFailed { path, reason } => {
                write!(f, "Failed to load config from '{}': {}", path.display(), reason)
            }
            Error::ConfigSaveFailed { path, reason } => {
                write!(f, "Failed to save config to '{}': {}", path.display(), reason)
            }
            Error::ConfigNotFound => {
                write!(f, "Configuration file not found")
            }
            Error::ConfigValidationFailed { field, reason } => {
                write!(f, "Configuration validation failed for '{}': {}", field, reason)
            }
            Error::ConfigSerializationFailed { format, reason } => {
                write!(f, "Failed to serialize config as {}: {}", format, reason)
            }
            Error::ConfigParseFailed { format, reason } => {
                write!(f, "Failed to parse {} config: {}", format, reason)
            }
            Error::ShellConfigNotFound { shell_type } => {
                write!(f, "Shell configuration not found for '{}'", shell_type)
            }
            Error::ThemeNotFound { theme_name } => {
                write!(f, "Theme '{}' not found", theme_name)
            }
            Error::ThemeAlreadyExists { theme_name } => {
                write!(f, "Theme '{}' already exists", theme_name)
            }
            Error::CannotRemoveBuiltInTheme { theme_name } => {
                write!(f, "Cannot remove built-in theme '{}'", theme_name)
            }
            Error::ThemeExportFailed { theme_name, reason } => {
                write!(f, "Failed to export theme '{}': {}", theme_name, reason)
            }
            Error::ThemeImportFailed { reason } => {
                write!(f, "Failed to import theme: {}", reason)
            }
            Error::UnknownComponent { component } => {
                write!(f, "Unknown component: '{}'", component)
            }
            Error::UnknownColorScheme { scheme } => {
                write!(f, "Unknown color scheme: '{}'", scheme)
            }

            // Terminal errors
            Error::NoPtyHandleAvailable => {
                write!(f, "No PTY handle available")
            }
            Error::OutputBufferFull { command, size } => {
                write!(f, "Output buffer full for command '{}' (size: {} bytes)", command, size)
            }

            // I/O and serialization errors
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::Serde(err) => write!(f, "Serialization error: {}", err),
            Error::Toml(err) => write!(f, "TOML parsing error: {}", err),
            Error::Regex(err) => write!(f, "Regex compilation error: {}", err),

            // Generic fallback
            Error::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Error::Toml(err)
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Other(err.to_string())
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::Regex(err)
    }
}

impl From<Box<dyn std::error::Error>> for Error {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Error::Other(err.to_string())
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Other(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error::Other(err.to_string())
    }
}
