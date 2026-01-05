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

use mosaicterm::context::ContextDetector;
use mosaicterm::state_manager::StateManager;
use mosaicterm::terminal::Terminal;
use std::collections::HashMap;
use tracing::info;

/// Detect git context from the filesystem
///
/// Checks the current working directory for a git repository and returns
/// the current branch name with an optional dirty indicator (*).
///
/// # Arguments
///
/// * `terminal` - Optional reference to the terminal to get working directory
///
/// # Returns
///
/// * `Some(branch_name)` - The branch name, with " *" suffix if there are uncommitted changes
/// * `None` - If not in a git repository or no terminal is available
///
/// # Example
///
/// ```ignore
/// let context = detect_git_context(Some(&terminal));
/// // Returns Some("main *") if on main branch with uncommitted changes
/// ```
pub fn detect_git_context(terminal: Option<&Terminal>) -> Option<String> {
    let working_dir = terminal?.get_working_directory();

    let repo = git2::Repository::discover(working_dir).ok()?;
    let head = repo.head().ok()?;
    let branch_name = head.shorthand()?;

    let has_changes = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);

    if has_changes {
        Some(format!("{} *", branch_name))
    } else {
        Some(branch_name.to_string())
    }
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
) -> Vec<String> {
    info!("Parsing environment output: {}", output);
    let env = parse_env_output(output);

    // Detect contexts from environment
    let contexts = context_detector.detect_contexts(&env);
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
                info!("Found env var: {}={}", key, value);
                env.insert(key.to_string(), value.to_string());
            } else {
                info!("Skipping empty env var: {}", key);
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
