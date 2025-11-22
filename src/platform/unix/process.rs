//! Unix process tree operations

use crate::error::{Error, Result};
use crate::platform::traits::ProcessTreeOps;
use nix::sys::signal::{kill, Signal as NixSignal};
use nix::unistd::Pid;
use std::collections::HashSet;
#[cfg(target_os = "linux")]
use std::fs;

pub struct UnixProcessTree;

impl UnixProcessTree {
    pub fn new() -> Self {
        Self
    }
}

impl ProcessTreeOps for UnixProcessTree {
    fn get_child_pids(&self, parent_pid: u32) -> Result<Vec<u32>> {
        #[cfg(target_os = "linux")]
        {
            self.get_child_pids_linux(parent_pid)
        }

        #[cfg(target_os = "macos")]
        {
            self.get_child_pids_macos(parent_pid)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Fallback for other Unix systems - try Linux method first
            self.get_child_pids_linux(parent_pid)
        }
    }

    fn kill_process_tree(&self, root_pid: u32) -> Result<()> {
        // Get all descendants recursively
        let descendants = self.get_all_descendant_pids(root_pid)?;

        // Kill all descendants first (children before parents)
        for pid in descendants.iter().rev() {
            let _ = kill(Pid::from_raw(*pid as i32), NixSignal::SIGTERM);
        }

        // Finally kill the root process
        let _ = kill(Pid::from_raw(root_pid as i32), NixSignal::SIGTERM);

        Ok(())
    }
}

impl UnixProcessTree {
    /// Get child PIDs on Linux using /proc filesystem
    #[cfg(target_os = "linux")]
    fn get_child_pids_linux(&self, parent_pid: u32) -> Result<Vec<u32>> {
        let mut children = Vec::new();

        // Read /proc to find all child processes
        let proc_dir = fs::read_dir("/proc").map_err(Error::Io)?;

        for entry in proc_dir.flatten() {
            // Check if this is a PID directory (all digits)
            if let Ok(file_name) = entry.file_name().into_string() {
                if let Ok(pid) = file_name.parse::<u32>() {
                    // Read the stat file to get parent PID
                    let stat_path = format!("/proc/{}/stat", pid);
                    if let Ok(stat_content) = fs::read_to_string(&stat_path) {
                        // Parse parent PID from stat file
                        // Format: pid (comm) state ppid ...
                        if let Some(ppid) = parse_ppid_from_stat(&stat_content) {
                            if ppid == parent_pid {
                                children.push(pid);
                            }
                        }
                    }
                }
            }
        }

        Ok(children)
    }

    /// Get child PIDs on macOS using ps command
    #[cfg(target_os = "macos")]
    fn get_child_pids_macos(&self, parent_pid: u32) -> Result<Vec<u32>> {
        use std::process::Command;

        let mut children = Vec::new();

        // Use ps command to get all processes with their parent PIDs
        // ps -eo pid,ppid - outputs: PID PPID
        let output = Command::new("ps")
            .args(["-eo", "pid,ppid"])
            .output()
            .map_err(|e| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to run ps: {}", e),
                ))
            })?;

        if !output.status.success() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ps command failed",
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse ps output - skip header line, then parse PID and PPID
        for line in output_str.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let (Ok(pid), Ok(ppid)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                    if ppid == parent_pid {
                        children.push(pid);
                    }
                }
            }
        }

        Ok(children)
    }

    /// Recursively get all descendant PIDs (children, grandchildren, etc.)
    fn get_all_descendant_pids(&self, parent_pid: u32) -> Result<Vec<u32>> {
        let mut all_descendants = Vec::new();
        let mut to_check = vec![parent_pid];
        let mut checked = HashSet::new();

        while let Some(pid) = to_check.pop() {
            if checked.contains(&pid) {
                continue;
            }
            checked.insert(pid);

            if let Ok(children) = self.get_child_pids(pid) {
                for child in children {
                    all_descendants.push(child);
                    to_check.push(child);
                }
            }
        }

        Ok(all_descendants)
    }
}

/// Parse parent PID from /proc/[pid]/stat content
#[cfg(target_os = "linux")]
fn parse_ppid_from_stat(stat_content: &str) -> Option<u32> {
    // The format is: pid (comm) state ppid ...
    // We need to skip the comm field which can contain spaces and parentheses

    // Find the last ')' which closes the comm field
    let close_paren = stat_content.rfind(')')?;

    // Split the rest by whitespace and get the second field (ppid)
    let after_comm = &stat_content[close_paren + 1..];
    let parts: Vec<&str> = after_comm.split_whitespace().collect();

    // parts[0] is state, parts[1] is ppid
    if parts.len() > 1 {
        parts[1].parse::<u32>().ok()
    } else {
        None
    }
}
