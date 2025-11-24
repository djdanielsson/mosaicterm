//! Unix signal operations

use crate::error::{Error, Result};
use crate::platform::traits::SignalOps;
use nix::sys::signal::{kill, Signal as NixSignal};
use nix::unistd::Pid;

pub struct UnixSignals;

impl UnixSignals {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SignalOps for UnixSignals {
    async fn send_interrupt(&self, pid: u32) -> Result<()> {
        kill(Pid::from_raw(pid as i32), NixSignal::SIGINT).map_err(|e| Error::SignalSendFailed {
            signal: "SIGINT".to_string(),
            reason: e.to_string(),
        })
    }

    async fn send_terminate(&self, pid: u32) -> Result<()> {
        kill(Pid::from_raw(pid as i32), NixSignal::SIGTERM).map_err(|e| Error::SignalSendFailed {
            signal: "SIGTERM".to_string(),
            reason: e.to_string(),
        })
    }

    async fn send_kill(&self, pid: u32) -> Result<()> {
        kill(Pid::from_raw(pid as i32), NixSignal::SIGKILL).map_err(|e| Error::SignalSendFailed {
            signal: "SIGKILL".to_string(),
            reason: e.to_string(),
        })
    }

    fn is_process_running(&self, pid: u32) -> bool {
        // Check if process exists by sending signal 0 (SIGCONT)
        kill(Pid::from_raw(pid as i32), NixSignal::SIGCONT).is_ok()
    }
}
