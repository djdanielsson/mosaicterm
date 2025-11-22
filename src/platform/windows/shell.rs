//! Windows shell operations

use crate::platform::traits::ShellOps;
use std::path::PathBuf;

pub struct WindowsShell;

impl WindowsShell {
    pub fn new() -> Self {
        Self
    }
}

impl ShellOps for WindowsShell {
    fn default_shell(&self) -> PathBuf {
        // Try PowerShell first, then cmd.exe
        let powershell_paths = vec![
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
        ];

        for path in powershell_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                return path_buf;
            }
        }

        // Fallback to cmd.exe
        PathBuf::from(r"C:\Windows\System32\cmd.exe")
    }

    fn detect_shells(&self) -> Vec<(String, PathBuf)> {
        let mut shells = Vec::new();

        let shell_paths = vec![
            (
                "powershell",
                r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
            ),
            ("pwsh", r"C:\Program Files\PowerShell\7\pwsh.exe"),
            ("cmd", r"C:\Windows\System32\cmd.exe"),
        ];

        for (name, path) in shell_paths {
            let path_buf = PathBuf::from(path);
            if path_buf.exists() {
                shells.push((name.to_string(), path_buf));
            }
        }

        shells
    }
}
