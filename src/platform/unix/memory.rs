//! Unix memory operations

use crate::error::{Error, Result};
use crate::platform::traits::MemoryOps;

pub struct UnixMemory;

impl UnixMemory {
    pub fn new() -> Self {
        Self
    }
}

impl MemoryOps for UnixMemory {
    fn get_current_memory(&self) -> Result<usize> {
        #[cfg(target_os = "macos")]
        {
            // Get memory usage on macOS using ps
            let output = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .map_err(|e| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to run ps: {}", e),
                    ))
                })?;

            let memory_str = String::from_utf8(output.stdout).map_err(|e| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse ps output: {}", e),
                ))
            })?;

            let memory_kb = memory_str.trim().parse::<usize>().map_err(|e| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to parse memory value: {}", e),
                ))
            })?;

            Ok(memory_kb * 1024)
        }

        #[cfg(target_os = "linux")]
        {
            // Read from /proc/self/status on Linux
            let status = std::fs::read_to_string("/proc/self/status").map_err(Error::Io)?;

            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        let memory_kb = value.parse::<usize>().map_err(|e| {
                            Error::Io(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("Failed to parse memory value: {}", e),
                            ))
                        })?;
                        return Ok(memory_kb * 1024);
                    }
                }
            }

            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "VmRSS not found in /proc/self/status",
            )))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            // Fallback for other Unix systems
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Memory tracking not implemented for this Unix variant",
            )))
        }
    }

    fn get_peak_memory(&self) -> Result<usize> {
        // Peak memory tracking would require maintaining state
        // For now, return current memory as peak
        self.get_current_memory()
    }
}
