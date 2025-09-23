//! Error types and Result aliases for MosaicTerm

use std::fmt;

/// Result type alias for MosaicTerm operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for MosaicTerm
#[derive(Debug)]
pub enum Error {
    /// PTY-related errors
    Pty(String),

    /// Terminal emulation errors
    Terminal(String),

    /// Configuration errors
    Config(String),

    /// I/O errors
    Io(std::io::Error),

    /// Serialization errors
    Serde(serde_json::Error),

    /// TOML parsing errors
    Toml(toml::de::Error),

    /// Regex compilation errors
    Regex(regex::Error),

    /// Generic errors
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Pty(msg) => write!(f, "PTY error: {}", msg),
            Error::Terminal(msg) => write!(f, "Terminal error: {}", msg),
            Error::Config(msg) => write!(f, "Configuration error: {}", msg),
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::Serde(err) => write!(f, "Serialization error: {}", err),
            Error::Toml(err) => write!(f, "TOML parsing error: {}", err),
            Error::Regex(err) => write!(f, "Regex compilation error: {}", err),
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
