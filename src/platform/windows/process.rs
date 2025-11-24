//! Windows process tree operations

use crate::error::{Error, Result};
use crate::platform::traits::ProcessTreeOps;
use std::collections::HashSet;

pub struct WindowsProcessTree;

impl WindowsProcessTree {
    pub fn new() -> Self {
        Self
    }
}

impl ProcessTreeOps for WindowsProcessTree {
    fn get_child_pids(&self, parent_pid: u32) -> Result<Vec<u32>> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32,
            TH32CS_SNAPPROCESS,
        };

        let mut children = Vec::new();

        unsafe {
            // Create snapshot of all processes
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "CreateToolhelp32Snapshot failed: {}",
                        windows_sys::Win32::Foundation::GetLastError()
                    ),
                )));
            }

            let mut entry: PROCESSENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

            // Get first process
            if Process32First(snapshot, &mut entry) != 0 {
                loop {
                    // Check if this process's parent is the one we're looking for
                    if entry.th32ParentProcessID == parent_pid {
                        children.push(entry.th32ProcessID);
                    }

                    // Get next process
                    if Process32Next(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
        }

        Ok(children)
    }

    fn kill_process_tree(&self, root_pid: u32) -> Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
        use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess};

        // Get all descendants recursively
        let descendants = self.get_all_descendant_pids(root_pid)?;

        // Kill all descendants first (children before parents)
        for pid in descendants.iter().rev() {
            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, 0, *pid);
                if handle != 0 {
                    let _ = TerminateProcess(handle, 1);
                    CloseHandle(handle);
                }
            }
        }

        // Finally kill the root process
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, root_pid);
            if handle != 0 {
                let _ = TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }

        Ok(())
    }
}

impl WindowsProcessTree {
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
