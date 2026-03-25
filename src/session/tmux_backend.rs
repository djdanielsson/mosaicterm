use std::process::Command;

use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub name: String,
    pub windows: usize,
    pub created: String,
    pub attached: bool,
}

#[derive(Debug)]
pub struct TmuxSessionManager {
    session_prefix: String,
}

impl TmuxSessionManager {
    fn validate_session_name(name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err("Invalid tmux session name: contains disallowed characters".into());
        }
        if name.len() > 128 {
            return Err("Tmux session name too long".into());
        }
        Ok(())
    }

    pub fn new() -> Self {
        Self {
            session_prefix: "mosaicterm-".to_string(),
        }
    }

    pub fn is_available() -> bool {
        Command::new("tmux")
            .arg("-V")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn create_session(&self, name: &str) -> Result<String, Box<dyn std::error::Error>> {
        let session_name = format!("{}{}", self.session_prefix, name);
        let output = Command::new("tmux")
            .args(["new-session", "-d", "-s", &session_name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to create tmux session: {}", stderr).into());
        }

        info!("Created tmux session: {}", session_name);
        Ok(session_name)
    }

    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let output = match Command::new("tmux")
            .args([
                "list-sessions",
                "-F",
                "#{session_name}\t#{session_windows}\t#{session_created_string}\t#{session_attached}",
            ])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                debug!("tmux list-sessions failed: {}", e);
                return Vec::new();
            }
        };

        if !output.status.success() {
            return Vec::new();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter(|line| line.starts_with(&self.session_prefix))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 4 {
                    Some(SessionInfo {
                        name: parts[0].to_string(),
                        windows: parts[1].parse().unwrap_or(0),
                        created: parts[2].to_string(),
                        attached: parts[3] == "1",
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn list_mosaicterm_sessions(&self) -> Vec<SessionInfo> {
        self.list_sessions()
    }

    pub fn kill_session(&self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        Self::validate_session_name(name)?;
        let output = Command::new("tmux")
            .args(["kill-session", "-t", name])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to kill tmux session: {}", stderr).into());
        }

        info!("Killed tmux session: {}", name);
        Ok(())
    }

    pub fn kill_all_mosaicterm_sessions(&self) -> usize {
        let sessions = self.list_mosaicterm_sessions();
        let mut killed = 0;
        for session in &sessions {
            if self.kill_session(&session.name).is_ok() {
                killed += 1;
            }
        }
        killed
    }

    pub fn send_keys(&self, session: &str, keys: &str) -> Result<(), Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let output = Command::new("tmux")
            .args(["send-keys", "-t", session, keys, "Enter"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to send keys: {}", stderr).into());
        }
        Ok(())
    }

    pub fn capture_pane(&self, session: &str) -> Result<String, Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let output = Command::new("tmux")
            .args(["capture-pane", "-p", "-S", "-", "-t", session])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to capture pane: {}", stderr).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn set_history_limit(
        &self,
        session: &str,
        limit: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let output = Command::new("tmux")
            .args([
                "set-option",
                "-t",
                session,
                "history-limit",
                &limit.to_string(),
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to set history limit: {}", stderr);
        }
        Ok(())
    }

    pub fn split_window(
        &self,
        session: &str,
        vertical: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let mut args = vec!["split-window", "-t", session];
        if vertical {
            args.push("-v");
        } else {
            args.push("-h");
        }

        let output = Command::new("tmux").args(&args).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to split window: {}", stderr).into());
        }
        Ok(())
    }

    pub fn save_layout(&self, session: &str) -> Result<String, Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let output = Command::new("tmux")
            .args(["display-message", "-t", session, "-p", "#{window_layout}"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to save layout: {}", stderr).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn restore_layout(
        &self,
        session: &str,
        layout: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Self::validate_session_name(session)?;
        let output = Command::new("tmux")
            .args(["select-layout", "-t", session, layout])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to restore layout: {}", stderr).into());
        }
        Ok(())
    }

    pub fn session_prefix(&self) -> &str {
        &self.session_prefix
    }
}

impl Default for TmuxSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_creation() {
        let mgr = TmuxSessionManager::new();
        assert_eq!(mgr.session_prefix(), "mosaicterm-");
    }

    #[test]
    fn test_is_available_doesnt_panic() {
        let _ = TmuxSessionManager::is_available();
    }
}
