//! Command Detection and Classification
//!
//! Provides utilities for detecting and classifying different types of commands:
//! - TUI commands (fullscreen interactive apps like vim, htop, top)
//! - Directory change commands (cd, pushd, popd)
//! - Interactive commands (REPLs like python, node)
//! - Exit commands (exit, logout, quit)

use mosaicterm::config::Config;

/// Extract the command name from a command line
///
/// Returns the first word of the command, which is typically the program name.
pub fn get_command_name(command: &str) -> String {
    command.split_whitespace().next().unwrap_or("").to_string()
}

/// Check if a command is a TUI app that should open in fullscreen overlay
///
/// TUI apps are interactive terminal applications that need full screen control
/// (vim, htop, top, less, etc.)
pub fn is_tui_command(command: &str, config: &Config) -> bool {
    let cmd_name = get_command_name(command);
    config
        .tui_apps
        .fullscreen_commands
        .iter()
        .any(|prog| prog == &cmd_name)
}

/// Check if command is a directory change command
///
/// These commands modify the current working directory and require
/// special handling to keep the terminal state in sync.
pub fn is_cd_command(command: &str) -> bool {
    let cmd_name = get_command_name(command);
    cmd_name == "cd" || cmd_name == "pushd" || cmd_name == "popd"
}

/// Check if a command is interactive (TUI-based) and may not work well in block mode
///
/// These are REPL-style programs that expect continuous input.
/// Note: SSH is handled separately in the SSH module.
pub fn is_interactive_command(command: &str) -> bool {
    let cmd_name = get_command_name(command);

    // List of known interactive TUI programs (not handled by TUI overlay)
    let interactive_programs = [
        "python", "python3", "node", "irb", "ruby", // REPLs (SSH handled separately)
    ];

    interactive_programs.iter().any(|&prog| cmd_name == prog)
}

/// Check if a command is an exit/logout command
///
/// These commands terminate the current shell session.
pub fn is_exit_command(command: &str) -> bool {
    let trimmed = command.trim();
    trimmed == "exit" || trimmed == "logout" || trimmed == "quit"
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- get_command_name tests ----

    #[test]
    fn test_get_command_name_simple() {
        assert_eq!(get_command_name("ls -la"), "ls");
        assert_eq!(get_command_name("vim file.txt"), "vim");
        assert_eq!(get_command_name("cd /home/user"), "cd");
    }

    #[test]
    fn test_get_command_name_empty() {
        assert_eq!(get_command_name(""), "");
    }

    #[test]
    fn test_get_command_name_whitespace() {
        assert_eq!(get_command_name("   top   "), "top");
        assert_eq!(get_command_name("\t\thtop"), "htop");
    }

    #[test]
    fn test_get_command_name_path() {
        assert_eq!(
            get_command_name("/usr/bin/python3 script.py"),
            "/usr/bin/python3"
        );
        assert_eq!(get_command_name("./script.sh"), "./script.sh");
    }

    #[test]
    fn test_get_command_name_with_env_var() {
        assert_eq!(get_command_name("VAR=value command"), "VAR=value");
    }

    // ---- is_cd_command tests ----

    #[test]
    fn test_is_cd_command_basic() {
        assert!(is_cd_command("cd /home"));
        assert!(is_cd_command("cd"));
        assert!(is_cd_command("cd ~"));
        assert!(is_cd_command("cd .."));
        assert!(is_cd_command("cd -"));
    }

    #[test]
    fn test_is_cd_command_pushd_popd() {
        assert!(is_cd_command("pushd /tmp"));
        assert!(is_cd_command("popd"));
        assert!(is_cd_command("pushd +1"));
    }

    #[test]
    fn test_is_cd_command_false_positives() {
        assert!(!is_cd_command("ls"));
        assert!(!is_cd_command("echo cd"));
        assert!(!is_cd_command("cdd"));
        assert!(!is_cd_command("cdrom"));
    }

    #[test]
    fn test_is_cd_command_with_whitespace() {
        assert!(is_cd_command("  cd /home  "));
        assert!(is_cd_command("\tcd\t"));
    }

    // ---- is_interactive_command tests ----

    #[test]
    fn test_is_interactive_command_python() {
        assert!(is_interactive_command("python"));
        assert!(is_interactive_command("python3"));
        assert!(is_interactive_command("python -i script.py"));
    }

    #[test]
    fn test_is_interactive_command_other_repls() {
        assert!(is_interactive_command("node"));
        assert!(is_interactive_command("irb"));
        assert!(is_interactive_command("ruby"));
    }

    #[test]
    fn test_is_interactive_command_not_interactive() {
        assert!(!is_interactive_command("ls"));
        assert!(!is_interactive_command("cat file.py"));
        assert!(!is_interactive_command("npm install"));
        // Note: "python3 script.py" is detected as interactive since we check the command name
        // This is intentional - the command name "python3" matches interactive programs
    }

    // ---- is_exit_command tests ----

    #[test]
    fn test_is_exit_command_basic() {
        assert!(is_exit_command("exit"));
        assert!(is_exit_command("logout"));
        assert!(is_exit_command("quit"));
    }

    #[test]
    fn test_is_exit_command_with_whitespace() {
        assert!(is_exit_command("  exit  "));
        assert!(is_exit_command("\tlogout\t"));
    }

    #[test]
    fn test_is_exit_command_not_exit() {
        assert!(!is_exit_command("exit 0"));
        assert!(!is_exit_command("exit 1"));
        assert!(!is_exit_command("echo exit"));
        assert!(!is_exit_command("exitcode"));
    }

    // ---- is_tui_command tests ----

    #[test]
    fn test_is_tui_command_default_apps() {
        let config = Config::default();
        // Default config should include vim, htop, top, less, etc.
        assert!(is_tui_command("vim file.txt", &config));
        assert!(is_tui_command("htop", &config));
        assert!(is_tui_command("top", &config));
        assert!(is_tui_command("less file.txt", &config));
        assert!(is_tui_command("nano file.txt", &config));
    }

    #[test]
    fn test_is_tui_command_not_tui() {
        let config = Config::default();
        assert!(!is_tui_command("ls -la", &config));
        assert!(!is_tui_command("echo hello", &config));
        assert!(!is_tui_command("cat file.txt", &config));
        assert!(!is_tui_command("grep pattern file", &config));
    }

    #[test]
    fn test_is_tui_command_with_args() {
        let config = Config::default();
        assert!(is_tui_command("vim -R readonly.txt", &config));
        assert!(is_tui_command("top -d 1", &config));
    }

    #[test]
    fn test_is_tui_command_empty() {
        let config = Config::default();
        assert!(!is_tui_command("", &config));
        assert!(!is_tui_command("   ", &config));
    }
}
