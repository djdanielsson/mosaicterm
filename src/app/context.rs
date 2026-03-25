//! Context and Environment Management
//!
//! This module handles detection and display of environment contexts
//! in the terminal prompt, including:
//!
//! - **Python environments**: virtualenv, conda
//! - **Node.js version managers**: nvm
//! - **Git repository information**: branch name, dirty state
//!
//! ## Usage
//!
//! Environment contexts are detected from shell environment variables
//! and displayed in the prompt to help users understand their current
//! working context.
//!
//! ```text
//! (venv:myproject) ~/code$ [main *]
//!  ^^^^^^^^^^^^^^^          ^^^^^^^^
//!  env contexts             git context
//! ```

use mosaicterm::config::prompt::GitPromptStatus;
use mosaicterm::context::ContextDetector;
use mosaicterm::state_manager::StateManager;
use mosaicterm::terminal::Terminal;
use std::collections::HashMap;
use tracing::{debug, info};

/// Detect git context from the filesystem (simple string for backward compat)
pub fn detect_git_context(terminal: Option<&Terminal>) -> Option<String> {
    detect_git_status(terminal).map(|s| s.format_compact())
}

/// Detect detailed git status for prompt rendering
pub fn detect_git_status(terminal: Option<&Terminal>) -> Option<GitPromptStatus> {
    let working_dir = terminal?.get_working_directory();

    let repo = git2::Repository::discover(working_dir).ok()?;

    let head = repo.head().ok();
    let detached = head.as_ref().map(|h| !h.is_branch()).unwrap_or(true);
    let branch_name = head
        .as_ref()
        .and_then(|h| h.shorthand())
        .unwrap_or("HEAD")
        .to_string();

    let mut staged = 0usize;
    let mut modified = 0usize;
    let mut untracked = 0usize;

    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false)
        .include_unmodified(false);

    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for entry in statuses.iter() {
            let s = entry.status();
            if s.intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            ) {
                staged += 1;
            }
            if s.intersects(
                git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_RENAMED
                    | git2::Status::WT_TYPECHANGE,
            ) {
                modified += 1;
            }
            if s.intersects(git2::Status::WT_NEW) {
                untracked += 1;
            }
        }
    }

    let (ahead, behind) = (|| -> Option<(usize, usize)> {
        let local_oid = repo.head().ok()?.target()?;
        let branch = repo.head().ok()?;
        let branch_name = branch.shorthand()?;
        let upstream_name = format!("refs/remotes/origin/{}", branch_name);
        let upstream_ref = repo.find_reference(&upstream_name).ok()?;
        let upstream_oid = upstream_ref.target()?;
        repo.graph_ahead_behind(local_oid, upstream_oid).ok()
    })()
    .unwrap_or((0, 0));

    Some(GitPromptStatus {
        branch: branch_name,
        staged,
        modified,
        untracked,
        ahead,
        behind,
        detached,
    })
}

/// Parse environment variable output and detect active contexts
///
/// Parses shell output in "KEY=value" format and uses the ContextDetector
/// to identify active development environments.
///
/// # Arguments
///
/// * `output` - Raw environment variable output from shell (one KEY=value per line)
/// * `context_detector` - The context detector to use for environment detection
///
/// # Returns
///
/// A vector of formatted context strings suitable for display in the prompt.
pub fn parse_env_and_detect_contexts(
    output: &str,
    context_detector: &ContextDetector,
    working_dir: Option<&std::path::Path>,
) -> Vec<String> {
    debug!("Parsing environment output ({} bytes)", output.len());
    let env = parse_env_output(output);

    // Detect contexts from environment, passing working dir for project-aware filtering
    let contexts = context_detector.detect_contexts_with_dir(&env, working_dir);
    info!("Detected {} contexts: {:?}", contexts.len(), contexts);

    // Format contexts for display (venv/conda/nvm only - git handled separately)
    contexts.iter().map(|c| c.format_short()).collect()
}

/// Parse environment output into a HashMap
///
/// Converts lines of "KEY=value" format into a HashMap, skipping
/// empty lines and entries with empty values.
fn parse_env_output(output: &str) -> HashMap<String, String> {
    let mut env = HashMap::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            // Only add non-empty values
            if !value.is_empty() {
                debug!("Found env var: {}", key);
                env.insert(key.to_string(), value.to_string());
            } else {
                debug!("Skipping empty env var: {}", key);
            }
        }
    }

    env
}

/// Update git context in the state manager
///
/// Sets the git context for the active session, which will be displayed
/// in the terminal prompt.
pub fn update_state_git_context(state_manager: &mut StateManager, git_context: Option<String>) {
    if let Some(session) = state_manager.active_session_mut() {
        session.git_context = git_context;
    }
}

/// Update environment contexts in the state manager
///
/// Sets the active environment contexts (venv, conda, nvm) for the
/// active session, which will be displayed in the terminal prompt.
pub fn update_state_env_contexts(state_manager: &mut StateManager, contexts: Vec<String>) {
    if let Some(session) = state_manager.active_session_mut() {
        session.active_contexts = contexts.clone();
        info!("Updated session contexts to: {:?}", session.active_contexts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_output_simple() {
        let output = "VIRTUAL_ENV=/home/user/.venv/myproject";
        let env = parse_env_output(output);
        assert_eq!(
            env.get("VIRTUAL_ENV"),
            Some(&"/home/user/.venv/myproject".to_string())
        );
    }

    #[test]
    fn test_parse_env_output_multiple_lines() {
        let output = "VIRTUAL_ENV=/path/to/venv\nNVM_DIR=/home/user/.nvm\nPATH=/usr/bin";
        let env = parse_env_output(output);
        assert_eq!(env.len(), 3);
        assert!(env.contains_key("VIRTUAL_ENV"));
        assert!(env.contains_key("NVM_DIR"));
        assert!(env.contains_key("PATH"));
    }

    #[test]
    fn test_parse_env_output_empty_values_skipped() {
        let output = "EMPTY_VAR=\nGOOD_VAR=value";
        let env = parse_env_output(output);
        assert_eq!(env.len(), 1);
        assert!(!env.contains_key("EMPTY_VAR"));
        assert_eq!(env.get("GOOD_VAR"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_env_output_empty_lines_skipped() {
        let output = "VAR1=value1\n\n\nVAR2=value2\n";
        let env = parse_env_output(output);
        assert_eq!(env.len(), 2);
    }

    #[test]
    fn test_parse_env_output_whitespace_trimmed() {
        let output = "  VAR1=value1  \n  VAR2=value2  ";
        let env = parse_env_output(output);
        assert_eq!(env.len(), 2);
        assert!(env.contains_key("VAR1"));
        assert!(env.contains_key("VAR2"));
    }

    #[test]
    fn test_parse_env_output_value_with_equals() {
        let output = "URL=https://example.com?foo=bar";
        let env = parse_env_output(output);
        assert_eq!(
            env.get("URL"),
            Some(&"https://example.com?foo=bar".to_string())
        );
    }

    #[test]
    fn test_parse_env_output_malformed_lines() {
        let output = "GOOD=value\nno_equals_here";
        let env = parse_env_output(output);
        // Only GOOD=value should be parsed (no_equals_here has no '=')
        assert_eq!(env.len(), 1);
        assert!(env.contains_key("GOOD"));
    }

    #[test]
    fn test_parse_env_output_empty_key() {
        let output = "=value_with_empty_key";
        let env = parse_env_output(output);
        // Empty key with value is technically valid, but unusual
        assert_eq!(env.len(), 1);
        assert!(env.contains_key(""));
    }

    #[test]
    fn test_detect_git_context_no_terminal() {
        let result = detect_git_context(None);
        assert!(result.is_none());
    }

    #[test]
    fn test_update_state_git_context() {
        let mut state_manager = StateManager::new();
        update_state_git_context(&mut state_manager, Some("main *".to_string()));

        if let Some(session) = state_manager.active_session() {
            assert_eq!(session.git_context, Some("main *".to_string()));
        }
    }

    #[test]
    fn test_update_state_env_contexts() {
        let mut state_manager = StateManager::new();
        let contexts = vec!["venv:myenv".to_string(), "nvm:18.0".to_string()];
        update_state_env_contexts(&mut state_manager, contexts.clone());

        if let Some(session) = state_manager.active_session() {
            assert_eq!(session.active_contexts, contexts);
        }
    }
}
