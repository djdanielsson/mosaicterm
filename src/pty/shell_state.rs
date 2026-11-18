//! OS-level shell state reading
//!
//! Reads shell process state (CWD, children, environment) directly from the
//! operating system instead of injecting commands into the PTY output stream.
//!
//! ## Approach
//!
//! - **Linux** uses [`/proc`](https://man7.org/linux/man-pages/man5/proc.5.html):
//!   `readlink` on `/proc/{pid}/cwd`, `/proc/{pid}/task/{pid}/children` (with a
//!   fallback scan of `/proc/*/stat` for PPID), and parsing `/proc/{pid}/environ`.
//! - **macOS** shells out to **`lsof`** for CWD and **`pgrep -P`** for direct
//!   children; process environment is not exposed in a stable, unprivileged way,
//!   so [`read_environ_var`] returns [`None`].
//! - **Windows** (and other targets): not implemented here; all APIs return
//!   [`None`] / `false` so callers can treat the shell as idle / unknown.
//!
//! ## Shell integration files
//!
//! Private startup files install a shell hook that reports command status on
//! the PTY after the command's output and snapshots the shell's directory and
//! environment for the application's prompt.

use std::path::PathBuf;

/// Reads the process current working directory from the OS (no PTY I/O).
pub fn read_cwd(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        linux_read_cwd(pid)
    }
    #[cfg(target_os = "macos")]
    {
        macos_read_cwd(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

/// Returns `true` if the process has at least one child (direct descendant).
///
/// On failure or unsupported OS, returns `false` (assume idle).
pub fn has_foreground_children(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        linux_has_foreground_children(pid)
    }
    #[cfg(target_os = "macos")]
    {
        macos_has_foreground_children(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        false
    }
}

/// Reads a single environment variable from the process, if the OS exposes it.
///
/// On macOS and Windows this always returns [`None`].
pub fn read_environ_var(pid: u32, var_name: &str) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        linux_read_environ_var(pid, var_name)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, var_name);
        None
    }
}

#[cfg(target_os = "linux")]
fn linux_read_cwd(pid: u32) -> Option<PathBuf> {
    let path = format!("/proc/{pid}/cwd");
    match std::fs::read_link(&path) {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::debug!(pid, error = %e, %path, "read /proc cwd symlink failed");
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_read_cwd(pid: u32) -> Option<PathBuf> {
    use std::process::{Command, Stdio};

    let output = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .stderr(Stdio::null())
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            tracing::debug!(pid, error = %e, "lsof cwd spawn failed");
            return None;
        }
    };

    if !output.status.success() {
        tracing::debug!(pid, status = ?output.status, "lsof cwd query failed");
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    tracing::debug!(pid, "lsof cwd output had no n/ line");
    None
}

#[cfg(target_os = "linux")]
fn linux_has_foreground_children(pid: u32) -> bool {
    let task_path = format!("/proc/{pid}/task/{pid}/children");
    match std::fs::read_to_string(&task_path) {
        Ok(s) => !s.trim().is_empty(),
        Err(e) => {
            tracing::debug!(
                pid,
                error = %e,
                path = task_path,
                "read children file failed, falling back to /proc stat scan"
            );
            linux_has_children_via_proc_stat(pid)
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_has_children_via_proc_stat(parent: u32) -> bool {
    let entries = match std::fs::read_dir("/proc") {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!(error = %e, "read_dir /proc failed");
            return false;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.parse::<u32>().is_err() {
            continue;
        }
        let stat_path = format!("/proc/{name}/stat");
        let Ok(contents) = std::fs::read_to_string(&stat_path) else {
            continue;
        };
        if linux_parse_ppid_from_stat(&contents) == Some(parent) {
            return true;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn linux_parse_ppid_from_stat(stat: &str) -> Option<u32> {
    let rparen = stat.rfind(')')?;
    let rest = stat[rparen + 1..].trim_start();
    let mut fields = rest.split_whitespace();
    let _state = fields.next()?;
    fields.next()?.parse().ok()
}

#[cfg(target_os = "macos")]
fn macos_has_foreground_children(pid: u32) -> bool {
    use std::process::{Command, Stdio};

    let output = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                !out.stdout.iter().all(|b| b.is_ascii_whitespace())
            } else if out.status.code() == Some(1) {
                false
            } else {
                tracing::debug!(pid, status = ?out.status, "pgrep -P unexpected exit");
                false
            }
        }
        Err(e) => {
            tracing::debug!(pid, error = %e, "pgrep spawn failed");
            false
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_read_environ_var(pid: u32, var_name: &str) -> Option<String> {
    let path = format!("/proc/{pid}/environ");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!(pid, error = %e, %path, "read environ failed");
            return None;
        }
    };

    let prefix = format!("{var_name}=");
    for chunk in bytes.split(|&b| b == 0) {
        let s = std::str::from_utf8(chunk).ok()?;
        if let Some(rest) = s.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

const ENV_VARS: &[&str] = &[
    "VIRTUAL_ENV",
    "CONDA_DEFAULT_ENV",
    "CONDA_PREFIX",
    "NVM_BIN",
    "RBENV_VERSION",
    "rvm_ruby_string",
    "DIRENV_DIR",
    "GOVERSION",
    "JAVA_HOME",
    "RUSTUP_TOOLCHAIN",
    "DOCKER_HOST",
    "DOCKER_CONTEXT",
    "KUBECONFIG",
    "AWS_PROFILE",
    "AWS_DEFAULT_PROFILE",
    "TF_WORKSPACE",
];

/// A prompt boundary emitted by this session's shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
    Ready,
    Completed(i32),
}

/// Owns shell integration files and recognizes this session's PTY markers.
pub struct ShellIntegration {
    directory: tempfile::TempDir,
    marker: String,
    marker_tail: Vec<u8>,
    ready: bool,
}

impl ShellIntegration {
    /// Install integration for a Bash or Zsh interactive session.
    pub fn install(
        shell: crate::models::ShellType,
        args: &mut Vec<String>,
        environment: &mut std::collections::HashMap<String, String>,
    ) -> std::io::Result<Option<Self>> {
        use crate::models::ShellType;

        match shell {
            // Bash only reads --rcfile for interactive, non-login shells.
            // Preserve user-supplied startup and non-interactive options.
            ShellType::Bash
                if !args.iter().any(|arg| {
                    matches!(
                        arg.as_str(),
                        "--login" | "--norc" | "--rcfile" | "--init-file"
                    ) || (arg.starts_with('-')
                        && !arg.starts_with("--")
                        && (arg[1..].contains('l') || arg[1..].contains('c')))
                }) => {}
            ShellType::Zsh => {}
            _ => return Ok(None),
        }

        let directory = tempfile::Builder::new().prefix("mosaicterm-").tempdir()?;
        let marker = format!("\x1b]777;{};", uuid::Uuid::new_v4());
        let integration = Self {
            directory,
            marker,
            marker_tail: Vec::new(),
            ready: false,
        };
        let directory = integration.directory.path();
        let hook = integration.hook(shell);
        match shell {
            ShellType::Bash => {
                let rcfile = directory.join("bashrc");
                std::fs::write(
                    &rcfile,
                    format!(
                        r#"
[[ -f /etc/bash.bashrc ]] && source /etc/bash.bashrc
[[ -f "$HOME/.bashrc" ]] && source "$HOME/.bashrc"
{hook}
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
  PROMPT_COMMAND=(__mosaicterm_precmd "${{PROMPT_COMMAND[@]}}")
else
  PROMPT_COMMAND="__mosaicterm_precmd${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
fi
"#
                    ),
                )?;
                args.splice(
                    0..0,
                    [
                        "--rcfile".to_string(),
                        rcfile.to_string_lossy().into_owned(),
                    ],
                );
            }
            ShellType::Zsh => {
                let user_config_dir = environment
                    .get("ZDOTDIR")
                    .filter(|path| !path.is_empty())
                    .map(|path| shell_quote(path))
                    .unwrap_or_else(|| "\"$HOME\"".to_string());
                let restore_dir =
                    format!("ZDOTDIR={}\n", shell_quote(&directory.to_string_lossy()));
                for name in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
                    let source = format!(
                        "ZDOTDIR={user_config_dir}\n[[ -f \"$ZDOTDIR/{name}\" ]] && source \"$ZDOTDIR/{name}\"\n"
                    );
                    let script = match name {
                        ".zshrc" => format!(
                            "{source}unsetopt PROMPT_SP\n{hook}\nprecmd_functions=(__mosaicterm_precmd \"${{precmd_functions[@]}}\")\n[[ -o login ]] && {restore_dir}"
                        ),
                        ".zshenv" | ".zprofile" => format!("{source}{restore_dir}"),
                        _ => source,
                    };
                    std::fs::write(directory.join(name), script)?;
                }
                environment.insert("ZDOTDIR".into(), directory.to_string_lossy().into_owned());
            }
            _ => unreachable!(),
        }
        Ok(Some(integration))
    }

    fn hook(&self, shell: crate::models::ShellType) -> String {
        let state_path = shell_quote(&self.directory.path().join("state").to_string_lossy());
        let state_fields = ENV_VARS
            .iter()
            .map(|var| format!(" \"{var}=${{{var}-}}\""))
            .collect::<String>();
        let marker = self
            .marker
            .strip_prefix('\x1b')
            .expect("OSC marker starts with ESC");
        let return_status = if shell == crate::models::ShellType::Bash {
            "$__ec"
        } else {
            // Zsh skips subsequent precmd hooks when a hook returns nonzero.
            "0"
        };
        format!(
            r#"
# Provide directory jumping without overriding commands from the user's startup files.
if command -v zoxide >/dev/null 2>&1; then
  __mosaicterm_zoxide_jump() {{
    local target
    if (( $# == 0 )); then
      builtin cd
      return
    fi
    target=$(command zoxide query -- "$@") && builtin cd -- "$target"
  }}
  if ! type z >/dev/null 2>&1; then
    function z {{ __mosaicterm_zoxide_jump "$@"; }}
  fi
  if ! type zi >/dev/null 2>&1; then
    function zi {{ __mosaicterm_zoxide_jump "$@"; }}
  fi
fi

__mosaicterm_precmd() {{
  local __ec=$? __event
  if [[ -z "${{__mosaicterm_ready-}}" ]]; then
    __mosaicterm_ready=1
    __event=ready
  else
    __event=$__ec
  fi
  PS1='' PS2='' PS3='' PS4=''
  builtin printf '%s\0' "$PWD"{state_fields} 2>/dev/null > {state_path}
  builtin printf '\033{marker}%s\007' "$__event"
  return {return_status}
}}
"#
        )
    }

    /// True once the shell has advertised working integration.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Read the latest directory and environment snapshot.
    pub fn read_state(&self) -> Option<(PathBuf, String)> {
        let data = std::fs::read(self.directory.path().join("state")).ok()?;
        let data = String::from_utf8_lossy(&data);
        let mut fields = data.split_terminator('\0');
        Some((
            PathBuf::from(fields.next()?),
            fields.collect::<Vec<_>>().join("\n"),
        ))
    }

    /// Look for completion markers even when a PTY read splits the sequence.
    pub fn observe_output(&mut self, data: &[u8]) -> Option<ShellEvent> {
        let mut bytes = std::mem::take(&mut self.marker_tail);
        bytes.extend_from_slice(data);
        let prefix = self.marker.as_bytes();
        let mut event = None;
        let mut remaining = bytes.as_slice();
        while let Some(start) = remaining.windows(prefix.len()).position(|w| w == prefix) {
            let value = &remaining[start + prefix.len()..];
            if let Some(end) = value.iter().position(|&byte| byte == b'\x07') {
                if let Ok(value) = std::str::from_utf8(&value[..end]) {
                    if value == "ready" {
                        self.ready = true;
                        event = Some(ShellEvent::Ready);
                    } else if let Ok(code) = value.parse::<i32>() {
                        event = Some(ShellEvent::Completed(code));
                    }
                }
                remaining = &remaining[start + prefix.len() + end + 1..];
            } else if value.len() <= 5 {
                self.marker_tail.extend_from_slice(&remaining[start..]);
                return event;
            } else {
                remaining = value;
            }
        }
        let suffix_len = (1..prefix.len().min(remaining.len() + 1))
            .rev()
            .find(|&size| remaining.ends_with(&prefix[..size]))
            .unwrap_or(0);
        self.marker_tail
            .extend_from_slice(&remaining[remaining.len() - suffix_len..]);
        event
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ShellType;
    use std::collections::HashMap;

    #[test]
    fn startup_and_state_files_are_private_to_each_session() {
        let first = ShellIntegration::install(ShellType::Bash, &mut vec![], &mut HashMap::new())
            .unwrap()
            .unwrap();
        let second = ShellIntegration::install(ShellType::Zsh, &mut vec![], &mut HashMap::new())
            .unwrap()
            .unwrap();
        let directory = first.directory.path().to_owned();
        assert_ne!(directory, second.directory.path());
        assert!(directory.join("bashrc").is_file());
        assert!(second.directory.path().join(".zshrc").is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                directory.metadata().unwrap().permissions().mode() & 0o077,
                0
            );
        }
        drop(first);
        assert!(!directory.exists());
        assert!(second.directory.path().is_dir());
    }

    #[test]
    fn completion_marker_can_span_pty_reads() {
        let mut integration =
            ShellIntegration::install(ShellType::Bash, &mut vec![], &mut HashMap::new())
                .unwrap()
                .unwrap();
        let ready = format!("{}ready\x07", integration.marker);
        assert_eq!(
            integration.observe_output(ready.as_bytes()),
            Some(ShellEvent::Ready)
        );
        let completion = format!("output{}42\x07", integration.marker);
        let split_at = completion.len() - 2;
        let (first, second) = completion.as_bytes().split_at(split_at);
        assert_eq!(integration.observe_output(first), None);
        assert_eq!(
            integration.observe_output(second),
            Some(ShellEvent::Completed(42))
        );
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn zsh_reports_status_with_an_existing_precmd_hook() {
        use crate::pty::PtyManager;

        if crate::pty::validate_command("zsh").is_err() {
            return;
        }
        let user_config = tempfile::tempdir().unwrap();
        std::fs::write(user_config.path().join(".zshrc"), "precmd() { true; }\n").unwrap();
        let mut environment = HashMap::from([(
            "ZDOTDIR".to_string(),
            user_config.path().to_string_lossy().into_owned(),
        )]);
        let mut args = vec!["+Z".to_string(), "--login".to_string()];
        let mut integration =
            ShellIntegration::install(ShellType::Zsh, &mut args, &mut environment)
                .unwrap()
                .unwrap();
        let manager = PtyManager::new();
        let handle = manager
            .create_pty("zsh", &args, &environment, Some(user_config.path()))
            .await
            .unwrap();

        async fn next_event(
            manager: &PtyManager,
            handle: &crate::pty::PtyHandle,
            integration: &mut ShellIntegration,
        ) -> ShellEvent {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let bytes = manager.read_output(handle, 0).await.unwrap();
                    if let Some(event) = integration.observe_output(&bytes) {
                        return event;
                    }
                }
            })
            .await
            .expect("shell integration marker")
        }

        assert_eq!(
            next_event(&manager, &handle, &mut integration).await,
            ShellEvent::Ready
        );
        manager.send_input(&handle, b"false\n").await.unwrap();
        assert_eq!(
            next_event(&manager, &handle, &mut integration).await,
            ShellEvent::Completed(1)
        );
        manager.send_input(&handle, b"cd /\n").await.unwrap();
        assert_eq!(
            next_event(&manager, &handle, &mut integration).await,
            ShellEvent::Completed(0)
        );
        assert_eq!(integration.read_state().unwrap().0, PathBuf::from("/"));
        manager.terminate_pty(&handle).await.unwrap();
    }
}
