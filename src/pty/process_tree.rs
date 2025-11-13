//! Process Tree Management
//!
//! Utilities for tracking and managing child processes in a PTY session.

use crate::error::{Error, Result};
use std::collections::HashSet;

#[cfg(unix)]
use std::fs;

/// Get all child process IDs of a given parent PID
#[cfg(unix)]
pub fn get_child_pids(parent_pid: u32) -> Result<Vec<u32>> {
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

#[cfg(not(unix))]
pub fn get_child_pids(_parent_pid: u32) -> Result<Vec<u32>> {
    // Windows implementation would use different API
    Ok(Vec::new())
}

/// Parse parent PID from /proc/[pid]/stat content
#[cfg(unix)]
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

/// Recursively get all descendant PIDs (children, grandchildren, etc.)
pub fn get_all_descendant_pids(parent_pid: u32) -> Result<Vec<u32>> {
    let mut all_descendants = Vec::new();
    let mut to_check = vec![parent_pid];
    let mut checked = HashSet::new();
    
    while let Some(pid) = to_check.pop() {
        if checked.contains(&pid) {
            continue;
        }
        checked.insert(pid);
        
        if let Ok(children) = get_child_pids(pid) {
            for child in children {
                all_descendants.push(child);
                to_check.push(child);
            }
        }
    }
    
    Ok(all_descendants)
}

/// Kill a process and all its descendants
#[cfg(unix)]
pub fn kill_process_tree(root_pid: u32, signal: nix::sys::signal::Signal) -> Result<()> {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    
    // Get all descendants
    let descendants = get_all_descendant_pids(root_pid)?;
    
    // Kill all descendants first (children before parents)
    for pid in descendants.iter().rev() {
        let _ = kill(Pid::from_raw(*pid as i32), signal);
    }
    
    // Finally kill the root process
    let _ = kill(Pid::from_raw(root_pid as i32), signal);
    
    Ok(())
}

#[cfg(not(unix))]
pub fn kill_process_tree(_root_pid: u32, _signal: ()) -> Result<()> {
    // Windows implementation would go here
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(unix)]
    fn test_parse_ppid_from_stat() {
        let stat = "12345 (bash) S 1234 12345 12345 0 -1 4194560 1234 0 0 0 0 0 0 0 20 0 1 0 123456 12345678 1234 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        assert_eq!(parse_ppid_from_stat(stat), Some(1234));
        
        // Test with spaces in command name
        let stat_spaces = "12345 (my shell) S 1234 12345 12345 0 -1 4194560 1234 0 0 0 0 0 0 0 20 0 1 0 123456 12345678 1234 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        assert_eq!(parse_ppid_from_stat(stat_spaces), Some(1234));
    }
    
    #[test]
    #[cfg(unix)]
    fn test_get_child_pids() {
        // Get children of init (should always exist)
        let result = get_child_pids(1);
        assert!(result.is_ok());
        // Init should have at least some children on most systems
    }
    
    #[test]
    fn test_get_all_descendant_pids() {
        // Test with current process (should return empty or just our children)
        let result = get_all_descendant_pids(std::process::id());
        assert!(result.is_ok());
    }
}

