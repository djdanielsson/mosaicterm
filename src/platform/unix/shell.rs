//! Unix shell operations

use crate::platform::traits::ShellOps;
use std::env;
use std::path::PathBuf;

pub struct UnixShell;

impl UnixShell {
    pub fn new() -> Self {
        Self
    }
}

impl ShellOps for UnixShell {
    fn default_shell(&self) -> PathBuf {
        // Try to get from SHELL environment variable
        if let Ok(shell) = env::var("SHELL") {
            PathBuf::from(shell)
        } else {
            // Default to bash
            PathBuf::from("/bin/bash")
        }
    }

    fn detect_shells(&self) -> Vec<(String, PathBuf)> {
        let mut shells = Vec::new();

        // Common shell paths
        let shell_paths = vec![
            ("bash", "/bin/bash"),
            ("zsh", "/bin/zsh"),
            ("fish", "/usr/bin/fish"),
            ("dash", "/bin/dash"),
            ("sh", "/bin/sh"),
        ];

        for (name, path) in shell_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                shells.push((name.to_string(), path_buf));
            }
        }

        // Also check if SHELL env var points to something not in the list
        if let Ok(shell_env) = env::var("SHELL") {
            let shell_path = PathBuf::from(&shell_env);
            if shell_path.exists() {
                // Extract shell name from path
                let shell_name = shell_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("shell")
                    .to_string();

                // Only add if not already in list
                if !shells.iter().any(|(_, p)| p == &shell_path) {
                    shells.push((shell_name, shell_path));
                }
            }
        }

        shells
    }
}
