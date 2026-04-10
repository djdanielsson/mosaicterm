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
//! [`create_zdotdir`] / [`create_bash_rcfile`] write temporary startup files
//! that install a `precmd` / `PROMPT_COMMAND` hook.  Because the hook is sourced
//! during shell startup (not typed into the PTY), it produces **zero** visible
//! output.

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

// ---------------------------------------------------------------------------
// State file (written by the precmd hook, read by Rust)
// ---------------------------------------------------------------------------

/// Path to the temp file where the shell's precmd hook writes state.
pub fn state_file_path(shell_pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("mosaicterm_state_{}", shell_pid))
}

/// Read the last exit code from the precmd state file.
pub fn read_exit_code(shell_pid: u32) -> Option<i32> {
    let path = state_file_path(shell_pid);
    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(shell_pid, error = %e, "read state file failed");
            return None;
        }
    };
    for line in contents.lines() {
        if let Some(code_str) = line.strip_prefix("EXIT:") {
            return code_str.trim().parse().ok();
        }
    }
    None
}

/// Read an environment variable from the precmd state file.
///
/// Falls back to [`read_environ_var`] (Linux `/proc`) if the key isn't in the
/// state file, so this works even before the first precmd fires.
pub fn read_state_env_var(shell_pid: u32, var_name: &str) -> Option<String> {
    let path = state_file_path(shell_pid);
    if let Ok(contents) = std::fs::read_to_string(&path) {
        let prefix = format!("{}=", var_name);
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix(&prefix) {
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
        }
    }
    read_environ_var(shell_pid, var_name)
}

/// Clean up the state file when the shell exits.
pub fn cleanup_state_file(shell_pid: u32) {
    let path = state_file_path(shell_pid);
    let _ = std::fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// Shell startup files (installed via ZDOTDIR / --rcfile, NOT via PTY input)
// ---------------------------------------------------------------------------

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

fn hook_body(state_path: &str) -> String {
    let env_writes: String = ENV_VARS
        .iter()
        .map(|v| format!("  echo \"{}=${{{}}}\"", v, v))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"__mosaicterm_precmd() {{
  local __ec=$?
  PS1='' PS2='' PS3='' PS4=''
  {{
    echo "EXIT:$__ec"
{env_writes}
  }} > "{state_path}" 2>/dev/null
  return $__ec
}}"#,
        env_writes = env_writes,
        state_path = state_path,
    )
}

/// Create a temporary ZDOTDIR with a `.zshrc` that sources the user's real
/// RC files and then installs the MosaicTerm precmd hook.
///
/// Returns the path to the temporary ZDOTDIR directory.  The caller should
/// set `ZDOTDIR=<returned path>` in the shell's environment **before**
/// spawning it.  This way nothing is ever typed into the PTY.
pub fn create_zdotdir(shell_pid: u32) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("mosaicterm_zdotdir_{}", shell_pid));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "failed to create ZDOTDIR");
        return None;
    }

    let state_path = state_file_path(shell_pid).display().to_string();
    let body = hook_body(&state_path);

    let user_home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .display()
        .to_string();

    // .zshenv: source the user's real .zshenv but do NOT reset ZDOTDIR yet
    // (ZDOTDIR must remain pointed here until .zshrc is loaded)
    let zshenv_contents = format!(
        r#"# MosaicTerm shell integration — .zshenv
# Source the user's real .zshenv for environment variables
[[ -f "{home}/.zshenv" ]] && source "{home}/.zshenv"
"#,
        home = user_home,
    );
    let _ = std::fs::write(dir.join(".zshenv"), zshenv_contents);

    let zshrc_contents = format!(
        r#"# MosaicTerm shell integration — sourced automatically via ZDOTDIR
# Source the real user config files (before restoring ZDOTDIR)
[[ -f "{home}/.zprofile" ]] && source "{home}/.zprofile"
[[ -f "{home}/.zshrc" ]] && source "{home}/.zshrc"

# Install MosaicTerm precmd hook (after user RC so it runs first in the chain)
{body}
precmd_functions=(__mosaicterm_precmd ${{precmd_functions[@]}})

# Restore ZDOTDIR so nested shells and tools see the real home
ZDOTDIR="{home}"
"#,
        home = user_home,
        body = body,
    );

    let zshrc_path = dir.join(".zshrc");
    if let Err(e) = std::fs::write(&zshrc_path, zshrc_contents) {
        tracing::warn!(error = %e, "failed to write ZDOTDIR/.zshrc");
        return None;
    }

    Some(dir)
}

/// Create a temporary bash init file that sources the user's real bashrc
/// and installs the MosaicTerm PROMPT_COMMAND hook.
///
/// Returns the path to the init file.  The caller passes it to bash via
/// `--rcfile <path>` instead of the default `~/.bashrc`.
pub fn create_bash_rcfile(shell_pid: u32) -> Option<PathBuf> {
    let state_path = state_file_path(shell_pid).display().to_string();
    let body = hook_body(&state_path);

    let user_home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .display()
        .to_string();

    let contents = format!(
        r#"# MosaicTerm shell integration
[[ -f /etc/bash.bashrc ]] && source /etc/bash.bashrc
[[ -f "{home}/.bashrc" ]] && source "{home}/.bashrc"

{body}
PROMPT_COMMAND="__mosaicterm_precmd${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"

PS1='' PS2='' PS3='' PS4=''
"#,
        home = user_home,
        body = body,
    );

    let path = std::env::temp_dir().join(format!("mosaicterm_bashrc_{}", shell_pid));
    if let Err(e) = std::fs::write(&path, &contents) {
        tracing::warn!(error = %e, "failed to write bash rcfile");
        return None;
    }
    Some(path)
}

/// Clean up all temporary shell files.
pub fn cleanup_shell_files(shell_pid: u32) {
    cleanup_state_file(shell_pid);
    let zdotdir = std::env::temp_dir().join(format!("mosaicterm_zdotdir_{}", shell_pid));
    let _ = std::fs::remove_dir_all(&zdotdir);
    let bashrc = std::env::temp_dir().join(format!("mosaicterm_bashrc_{}", shell_pid));
    let _ = std::fs::remove_file(&bashrc);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_cwd_current_process_does_not_panic() {
        let pid = std::process::id();
        let _ = read_cwd(pid);
    }

    #[test]
    fn has_foreground_children_current_process_does_not_panic() {
        let pid = std::process::id();
        let _ = has_foreground_children(pid);
    }

    #[test]
    fn read_environ_var_current_process_does_not_panic() {
        let pid = std::process::id();
        let _ = read_environ_var(pid, "PATH");
    }

    #[test]
    fn state_file_path_contains_pid() {
        let path = state_file_path(12345);
        assert!(path.to_string_lossy().contains("mosaicterm_state_12345"));
    }

    #[test]
    fn create_zdotdir_writes_zshrc() {
        let pid = 88888;
        let dir = create_zdotdir(pid).expect("should create zdotdir");
        let zshrc = dir.join(".zshrc");
        assert!(zshrc.exists());
        let contents = std::fs::read_to_string(&zshrc).unwrap();
        assert!(contents.contains("__mosaicterm_precmd"));
        assert!(contents.contains("EXIT:$__ec"));
        assert!(contents.contains("VIRTUAL_ENV"));
        assert!(contents.contains("source"));
        assert!(contents.contains("PS1=''"));
        cleanup_shell_files(pid);
        assert!(!dir.exists());
    }

    #[test]
    fn create_bash_rcfile_writes_file() {
        let pid = 88887;
        let path = create_bash_rcfile(pid).expect("should create rcfile");
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("__mosaicterm_precmd"));
        assert!(contents.contains("PROMPT_COMMAND"));
        assert!(contents.contains(".bashrc"));
        cleanup_shell_files(pid);
        assert!(!path.exists());
    }

    #[test]
    fn read_exit_code_from_state_file() {
        let pid = 99999;
        let path = state_file_path(pid);
        std::fs::write(&path, "EXIT:42\nVIRTUAL_ENV=/some/path\n").unwrap();
        assert_eq!(read_exit_code(pid), Some(42));
        cleanup_state_file(pid);
        assert!(read_exit_code(pid).is_none());
    }

    #[test]
    fn read_state_env_var_from_file() {
        let pid = 99998;
        let path = state_file_path(pid);
        std::fs::write(&path, "EXIT:0\nVIRTUAL_ENV=/my/venv\nNVM_BIN=\n").unwrap();
        assert_eq!(
            read_state_env_var(pid, "VIRTUAL_ENV"),
            Some("/my/venv".to_string())
        );
        assert_eq!(read_state_env_var(pid, "NVM_BIN"), None);
        cleanup_state_file(pid);
    }
}
