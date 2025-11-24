//! Process Tree Management
//!
//! Utilities for tracking and managing child processes in a PTY session.
//! Uses platform abstraction for cross-platform process tree operations.

use crate::error::Result;
use crate::platform::Platform;
use std::collections::HashSet;

/// Get all child process IDs of a given parent PID
pub fn get_child_pids(parent_pid: u32) -> Result<Vec<u32>> {
    Platform::process_tree().get_child_pids(parent_pid)
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
pub fn kill_process_tree(root_pid: u32) -> Result<()> {
    Platform::process_tree().kill_process_tree(root_pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    #[ignore] // This test depends on system state and /proc availability
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
