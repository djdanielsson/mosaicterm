//! Windows signal operations

use crate::error::{Error, Result};
use crate::platform::traits::SignalOps;

pub struct WindowsSignals;

impl WindowsSignals {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SignalOps for WindowsSignals {
    async fn send_interrupt(&self, pid: u32) -> Result<()> {
        use windows_sys::Win32::System::Console::{
            AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, CTRL_C_EVENT,
        };

        // Proper Ctrl+C on Windows requires attaching to the target's console.
        // GenerateConsoleCtrlEvent's second arg is a process group ID, not a PID.
        // Using 0 sends to all processes attached to the current console.
        unsafe {
            // Detach from our own console, attach to the target's, send Ctrl+C, re-attach ours.
            let attached = AttachConsole(pid) != 0;
            if attached {
                let sent = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) != 0;
                FreeConsole();
                if sent {
                    return Ok(());
                }
            }
        }

        // Fallback: hard termination (Windows lacks Unix-style signals)
        self.send_terminate(pid).await
    }

    async fn send_terminate(&self, pid: u32) -> Result<()> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::PROCESS_TERMINATE;
        use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess};

        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if handle == 0 {
                return Err(Error::SignalSendFailed {
                    signal: "Terminate".to_string(),
                    reason: format!(
                        "OpenProcess failed: {}",
                        windows_sys::Win32::Foundation::GetLastError()
                    ),
                });
            }

            let result = TerminateProcess(handle, 1);
            CloseHandle(handle);

            if result == 0 {
                return Err(Error::SignalSendFailed {
                    signal: "Terminate".to_string(),
                    reason: format!(
                        "TerminateProcess failed: {}",
                        windows_sys::Win32::Foundation::GetLastError()
                    ),
                });
            }
        }

        Ok(())
    }

    async fn send_kill(&self, pid: u32) -> Result<()> {
        // On Windows, kill is the same as terminate (forceful)
        self.send_terminate(pid).await
    }

    fn is_process_running(&self, pid: u32) -> bool {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION};

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
            if handle == 0 {
                return false;
            }

            // Check if we can query the process (means it exists)
            use windows_sys::Win32::Foundation::CloseHandle;
            CloseHandle(handle);
            true
        }
    }
}
