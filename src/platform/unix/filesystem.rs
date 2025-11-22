//! Unix filesystem operations

use crate::error::{Error, Result};
use crate::platform::traits::FilesystemOps;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct UnixFilesystem;

impl UnixFilesystem {
    pub fn new() -> Self {
        Self
    }
}

impl FilesystemOps for UnixFilesystem {
    fn is_executable(&self, path: &Path) -> bool {
        if let Ok(metadata) = path.metadata() {
            if metadata.is_file() {
                // Check if file has executable permissions
                let permissions = metadata.permissions();
                return (permissions.mode() & 0o111) != 0;
            }
        }
        false
    }

    fn find_command(&self, command: &str) -> Result<Option<PathBuf>> {
        // Use 'which' command to find the command
        use std::process::Command;
        use std::process::Stdio;

        let output = Command::new("which")
            .arg(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to run which: {}", e),
                ))
            })?;

        if output.status.success() {
            let path_str = String::from_utf8(output.stdout).map_err(|e| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse which output: {}", e),
                ))
            })?;

            let path = PathBuf::from(path_str.trim());
            if path.exists() {
                Ok(Some(path))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
