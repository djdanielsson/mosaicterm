//! Windows filesystem operations

use crate::error::{Error, Result};
use crate::platform::traits::FilesystemOps;
use std::env;
use std::path::{Path, PathBuf};

pub struct WindowsFilesystem;

impl WindowsFilesystem {
    pub fn new() -> Self {
        Self
    }
}

impl FilesystemOps for WindowsFilesystem {
    fn is_executable(&self, path: &Path) -> bool {
        // On Windows, check for common executable extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            matches!(
                ext.to_lowercase().as_str(),
                "exe" | "bat" | "cmd" | "ps1" | "com"
            )
        } else {
            false
        }
    }

    fn find_command(&self, command: &str) -> Result<Option<PathBuf>> {
        use std::process::Command;
        use std::process::Stdio;

        // First, try using 'where' command (Windows equivalent of 'which')
        let output = Command::new("where")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let path_str = String::from_utf8(output.stdout).map_err(|e| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to parse where output: {}", e),
                    ))
                })?;

                // 'where' can return multiple paths, take the first one
                let first_line = path_str.lines().next().unwrap_or("").trim();
                if !first_line.is_empty() {
                    let path = PathBuf::from(first_line);
                    if path.exists() {
                        return Ok(Some(path));
                    }
                }
            }
            _ => {
                // 'where' failed, try manual PATH scanning
            }
        }

        // Fallback: manually scan PATH
        if let Ok(path_env) = env::var("PATH") {
            let executable_extensions = ["", ".exe", ".bat", ".cmd", ".ps1", ".com"];

            for path_dir in env::split_paths(&path_env) {
                for ext in &executable_extensions {
                    let full_path = path_dir.join(format!("{}{}", command, ext));
                    if full_path.exists() && self.is_executable(&full_path) {
                        return Ok(Some(full_path));
                    }
                }
            }
        }

        Ok(None)
    }
}
