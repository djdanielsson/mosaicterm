//! Windows memory operations

use crate::error::{Error, Result};
use crate::platform::traits::MemoryOps;

pub struct WindowsMemory;

impl WindowsMemory {
    pub fn new() -> Self {
        Self
    }
}

impl MemoryOps for WindowsMemory {
    fn get_current_memory(&self) -> Result<usize> {
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        unsafe {
            // Get handle to current process
            let process_handle = GetCurrentProcess();

            let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

            if GetProcessMemoryInfo(process_handle, &mut pmc, pmc.cb) == 0 {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "GetProcessMemoryInfo failed: {}",
                        windows_sys::Win32::Foundation::GetLastError()
                    ),
                )));
            }

            // Return WorkingSetSize (current memory usage) in bytes
            Ok(pmc.WorkingSetSize)
        }
    }

    fn get_peak_memory(&self) -> Result<usize> {
        use windows_sys::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        unsafe {
            // Get handle to current process
            let process_handle = GetCurrentProcess();

            let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

            if GetProcessMemoryInfo(process_handle, &mut pmc, pmc.cb) == 0 {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "GetProcessMemoryInfo failed: {}",
                        windows_sys::Win32::Foundation::GetLastError()
                    ),
                )));
            }

            // Return PeakWorkingSetSize (peak memory usage) in bytes
            Ok(pmc.PeakWorkingSetSize)
        }
    }
}
