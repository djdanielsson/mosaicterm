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
        use windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent;
        use windows_sys::Win32::System::Console::CTRL_C_EVENT;

        // GenerateConsoleCtrlEvent sends Ctrl+C to a process group
        // On Windows, we need to attach to the console of the target process
        // For now, we'll use TerminateProcess as a fallback for interrupt
        // Note: Proper Ctrl+C handling requires console attachment which is complex

        // Try to send Ctrl+C event (only works if process shares our console)
        unsafe {
            if GenerateConsoleCtrlEvent(CTRL_C_EVENT, pid) != 0 {
                return Ok(());
            }
        }

        // If that fails, fall back to termination
        // This is not ideal but Windows doesn't have Unix-style signals
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
