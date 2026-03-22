//! Prompt Rendering
//!
//! Handles the construction and updating of the terminal prompt display,
//! including environment contexts, git information, and SSH session state.

use mosaicterm::config::prompt::{EnvPromptContext, PromptFormatter, PromptSegment};
use mosaicterm::models::config::PromptStyle;
use mosaicterm::state_manager::StateManager;
use mosaicterm::terminal::Terminal;
use tracing::debug;

/// Build the rendered prompt as styled segments.
/// Falls back to a single-segment string for Classic/Minimal styles.
pub fn build_prompt_segments(
    terminal: Option<&Terminal>,
    state_manager: &StateManager,
    prompt_formatter: &PromptFormatter,
    ssh_session_active: bool,
    ssh_remote_prompt: Option<&str>,
    ssh_session_command: Option<&str>,
) -> Vec<PromptSegment> {
    use eframe::egui;

    if ssh_session_active {
        if let Some(remote_prompt) = ssh_remote_prompt {
            debug!("Using remote SSH prompt: '{}'", remote_prompt);
            return vec![PromptSegment {
                text: remote_prompt.to_string(),
                fg: egui::Color32::from_rgb(180, 180, 200),
                bg: None,
                bold: false,
                separator: None,
            }];
        } else if let Some(cmd) = ssh_session_command {
            let host = extract_ssh_host(cmd);
            return vec![PromptSegment {
                text: format!("[{}] $ ", host),
                fg: egui::Color32::from_rgb(180, 180, 200),
                bg: None,
                bold: false,
                separator: None,
            }];
        }
    }

    if let Some(terminal) = terminal {
        let working_dir = terminal.get_working_directory();

        let git_status = if let Some(session) = state_manager.active_session() {
            session.git_context.as_ref().and_then(|ctx| {
                mosaicterm::config::prompt::GitPromptStatus::from_context_string(ctx)
            })
        } else {
            None
        };

        let env_contexts: Vec<EnvPromptContext> = state_manager
            .active_session()
            .map(|session| {
                session
                    .active_contexts
                    .iter()
                    .filter_map(|ctx| {
                        let parts: Vec<&str> = ctx.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            Some(EnvPromptContext {
                                name: parts[0].to_string(),
                                value: parts[1].to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        match prompt_formatter.style() {
            PromptStyle::Classic | PromptStyle::Minimal => {
                let base_prompt = prompt_formatter.render(working_dir);
                let mut left_context = String::new();
                let mut right_context = String::new();

                if let Some(session) = state_manager.active_session() {
                    if !session.active_contexts.is_empty() {
                        let context_str = session.active_contexts.join(" ");
                        left_context = format!("({}) ", context_str);
                    }
                    if let Some(git) = &session.git_context {
                        right_context = format!(" [{}]", git);
                    }
                }

                vec![PromptSegment {
                    text: format!("{}{}{}", left_context, base_prompt, right_context),
                    fg: egui::Color32::from_rgb(180, 180, 200),
                    bg: None,
                    bold: false,
                    separator: None,
                }]
            }
            _ => {
                let segments = prompt_formatter.render_segments(
                    working_dir,
                    git_status.as_ref(),
                    &env_contexts,
                );

                let flat: String = segments.iter().map(|s| s.text.clone()).collect();
                debug!(
                    "Built prompt segments: '{}' (style: {:?})",
                    flat,
                    prompt_formatter.style()
                );

                segments
            }
        }
    } else {
        vec![PromptSegment {
            text: "$ ".to_string(),
            fg: egui::Color32::from_rgb(180, 180, 200),
            bg: None,
            bold: false,
            separator: None,
        }]
    }
}

/// Extract the host from an SSH command
///
/// Examples:
/// - `ssh user@host` -> `host`
/// - `ssh -p 22 host` -> `host`
/// - `ssh host` -> `host`
pub fn extract_ssh_host(ssh_command: &str) -> String {
    let parts: Vec<&str> = ssh_command.split_whitespace().collect();

    for (i, part) in parts.iter().enumerate() {
        // Skip "ssh" command itself
        if i == 0 && *part == "ssh" {
            continue;
        }

        // Skip options and their arguments
        if part.starts_with('-') {
            continue;
        }

        // Skip arguments to previous options (like -p 22, -i keyfile)
        if i > 0 {
            let prev = parts.get(i - 1);
            if let Some(p) = prev {
                if *p == "-p" || *p == "-i" || *p == "-l" || *p == "-o" {
                    continue;
                }
            }
        }

        // This should be the host or user@host
        if part.contains('@') {
            // user@host format
            if let Some(host) = part.split('@').nth(1) {
                return host.to_string();
            }
        } else {
            // Just host
            return (*part).to_string();
        }
    }

    "remote".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- extract_ssh_host tests ----

    #[test]
    fn test_extract_ssh_host_simple() {
        assert_eq!(extract_ssh_host("ssh myhost"), "myhost");
    }

    #[test]
    fn test_extract_ssh_host_with_user() {
        assert_eq!(extract_ssh_host("ssh user@myhost"), "myhost");
    }

    #[test]
    fn test_extract_ssh_host_with_port() {
        assert_eq!(extract_ssh_host("ssh -p 22 user@myhost"), "myhost");
    }

    #[test]
    fn test_extract_ssh_host_with_identity() {
        assert_eq!(extract_ssh_host("ssh -i ~/.ssh/key user@myhost"), "myhost");
    }

    #[test]
    fn test_extract_ssh_host_fallback() {
        assert_eq!(extract_ssh_host("ssh"), "remote");
    }

    #[test]
    fn test_extract_ssh_host_with_login_option() {
        assert_eq!(extract_ssh_host("ssh -l user myhost"), "myhost");
    }

    #[test]
    fn test_extract_ssh_host_with_config_option() {
        assert_eq!(
            extract_ssh_host("ssh -o StrictHostKeyChecking=no user@myhost"),
            "myhost"
        );
    }

    #[test]
    fn test_extract_ssh_host_complex_command() {
        assert_eq!(
            extract_ssh_host("ssh -p 2222 -i ~/.ssh/id_rsa -o StrictHostKeyChecking=no admin@production.example.com"),
            "production.example.com"
        );
    }

    #[test]
    fn test_extract_ssh_host_ipv4() {
        assert_eq!(extract_ssh_host("ssh root@192.168.1.100"), "192.168.1.100");
    }

    #[test]
    fn test_extract_ssh_host_without_user() {
        assert_eq!(extract_ssh_host("ssh -p 22 192.168.1.1"), "192.168.1.1");
    }

    fn flat(segments: &[mosaicterm::config::prompt::PromptSegment]) -> String {
        segments.iter().map(|s| s.text.clone()).collect()
    }

    #[test]
    fn test_build_prompt_no_terminal() {
        let state_manager = StateManager::new();
        let prompt_formatter = PromptFormatter::default();

        let segs =
            build_prompt_segments(None, &state_manager, &prompt_formatter, false, None, None);
        assert_eq!(flat(&segs), "$ ");
    }

    #[test]
    fn test_build_prompt_ssh_with_remote_prompt() {
        let state_manager = StateManager::new();
        let prompt_formatter = PromptFormatter::default();
        let remote_prompt = "user@remote:~$ ";

        let segs = build_prompt_segments(
            None,
            &state_manager,
            &prompt_formatter,
            true,
            Some(remote_prompt),
            Some("ssh user@remote"),
        );
        assert_eq!(flat(&segs), remote_prompt);
    }

    #[test]
    fn test_build_prompt_ssh_placeholder() {
        let state_manager = StateManager::new();
        let prompt_formatter = PromptFormatter::default();

        let segs = build_prompt_segments(
            None,
            &state_manager,
            &prompt_formatter,
            true,
            None,
            Some("ssh user@myserver.com"),
        );
        assert_eq!(flat(&segs), "[myserver.com] $ ");
    }

    #[test]
    fn test_build_prompt_ssh_simple_host() {
        let state_manager = StateManager::new();
        let prompt_formatter = PromptFormatter::default();

        let segs = build_prompt_segments(
            None,
            &state_manager,
            &prompt_formatter,
            true,
            None,
            Some("ssh devbox"),
        );
        assert_eq!(flat(&segs), "[devbox] $ ");
    }
}
