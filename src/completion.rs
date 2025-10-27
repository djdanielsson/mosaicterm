//! Command and Path Completion
//!
//! Provides intelligent auto-completion for commands, arguments, and file paths.

use crate::error::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Completion provider for shell commands and paths
#[derive(Debug, Clone)]
pub struct CompletionProvider {
    /// Cached list of executable commands in PATH
    command_cache: Vec<String>,
    /// Last time the command cache was updated
    cache_updated: Option<std::time::Instant>,
    /// Cache timeout duration (5 minutes)
    cache_timeout: std::time::Duration,
}

/// Completion result containing suggestions
#[derive(Debug, Clone)]
pub struct CompletionResult {
    /// List of completion suggestions
    pub suggestions: Vec<CompletionItem>,
    /// The prefix that was matched
    pub prefix: String,
    /// Type of completion
    pub completion_type: CompletionType,
}

/// Individual completion item
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The completion text
    pub text: String,
    /// Display label (may include extra info)
    pub label: String,
    /// Type of completion item
    pub item_type: CompletionItemType,
    /// Description or additional info
    pub description: Option<String>,
}

/// Type of completion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionType {
    /// Command completion
    Command,
    /// File/directory path completion
    Path,
    /// Argument completion
    Argument,
    /// Mixed completion
    Mixed,
}

/// Type of completion item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemType {
    /// Executable command
    Command,
    /// Directory
    Directory,
    /// File
    File,
    /// Symlink
    Symlink,
    /// Special item (e.g., "..", "~")
    Special,
}

impl CompletionProvider {
    /// Create a new completion provider
    pub fn new() -> Self {
        Self {
            command_cache: Vec::new(),
            cache_updated: None,
            cache_timeout: std::time::Duration::from_secs(300), // 5 minutes
        }
    }

    /// Get completions for the given input
    pub fn get_completions(&mut self, input: &str, working_dir: &Path) -> Result<CompletionResult> {
        let input_trimmed = input.trim();

        // Parse the input to determine what to complete
        let parts: Vec<&str> = input_trimmed.split_whitespace().collect();

        if parts.is_empty() {
            // No input - suggest common commands
            return Ok(CompletionResult {
                suggestions: self.get_common_commands(),
                prefix: String::new(),
                completion_type: CompletionType::Command,
            });
        }

        // Check if we're completing the command or an argument
        if parts.len() == 1 && !input_trimmed.ends_with(char::is_whitespace) {
            // Completing the command itself
            self.complete_command(parts[0])
        } else {
            // Completing an argument (likely a path)
            // Get the last word even if input doesn't end with space
            // This handles "cd D" where "D" is what we want to complete
            let last_arg = if input_trimmed.ends_with(char::is_whitespace) {
                ""
            } else {
                parts.last().unwrap_or(&"")
            };
            self.complete_path(last_arg, working_dir)
        }
    }

    /// Complete a command name
    fn complete_command(&mut self, prefix: &str) -> Result<CompletionResult> {
        self.refresh_command_cache_if_needed()?;

        let suggestions: Vec<CompletionItem> = self
            .command_cache
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .take(50) // Limit to 50 suggestions
            .map(|cmd| CompletionItem {
                text: cmd.clone(),
                label: cmd.clone(),
                item_type: CompletionItemType::Command,
                description: None,
            })
            .collect();

        Ok(CompletionResult {
            suggestions,
            prefix: prefix.to_string(),
            completion_type: CompletionType::Command,
        })
    }

    /// Complete a file or directory path
    fn complete_path(&self, prefix: &str, working_dir: &Path) -> Result<CompletionResult> {
        let (dir_path, file_prefix) = self.parse_path_prefix(prefix, working_dir);

        let mut suggestions = Vec::new();

        // Read directory contents
        if let Ok(entries) = fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                if let Ok(filename) = entry.file_name().into_string() {
                    // Skip hidden files unless prefix starts with '.'
                    if filename.starts_with('.') && !file_prefix.starts_with('.') {
                        continue;
                    }

                    // Check if filename matches prefix
                    if filename.starts_with(&file_prefix) {
                        let item_type = if let Ok(metadata) = entry.metadata() {
                            if metadata.is_dir() {
                                CompletionItemType::Directory
                            } else if metadata.is_symlink() {
                                CompletionItemType::Symlink
                            } else {
                                CompletionItemType::File
                            }
                        } else {
                            CompletionItemType::File
                        };

                        // Add trailing slash for directories
                        let display_name = if item_type == CompletionItemType::Directory {
                            format!("{}/", filename)
                        } else {
                            filename.clone()
                        };

                        suggestions.push(CompletionItem {
                            text: filename.clone(),
                            label: display_name,
                            item_type,
                            description: None,
                        });
                    }
                }
            }
        }

        // Sort: directories first, then files, alphabetically within each group
        suggestions.sort_by(|a, b| match (a.item_type, b.item_type) {
            (CompletionItemType::Directory, CompletionItemType::Directory)
            | (CompletionItemType::File, CompletionItemType::File)
            | (CompletionItemType::Symlink, CompletionItemType::Symlink) => {
                a.text.to_lowercase().cmp(&b.text.to_lowercase())
            }
            (CompletionItemType::Directory, _) => std::cmp::Ordering::Less,
            (_, CompletionItemType::Directory) => std::cmp::Ordering::Greater,
            _ => a.text.to_lowercase().cmp(&b.text.to_lowercase()),
        });

        // Limit suggestions
        suggestions.truncate(50);

        Ok(CompletionResult {
            suggestions,
            prefix: file_prefix.to_string(),
            completion_type: CompletionType::Path,
        })
    }

    /// Parse a path prefix into directory path and filename prefix
    fn parse_path_prefix(&self, prefix: &str, working_dir: &Path) -> (PathBuf, String) {
        if prefix.is_empty() {
            return (working_dir.to_path_buf(), String::new());
        }

        let path = Path::new(prefix);

        // Handle tilde expansion
        let expanded_path = if prefix.starts_with('~') {
            if let Some(home) = env::var_os("HOME") {
                PathBuf::from(home).join(&prefix[2..])
            } else {
                path.to_path_buf()
            }
        } else if path.is_absolute() {
            path.to_path_buf()
        } else {
            working_dir.join(path)
        };

        // Split into directory and filename parts
        if prefix.ends_with('/') || prefix.ends_with(std::path::MAIN_SEPARATOR) {
            // Already pointing to a directory
            (expanded_path, String::new())
        } else {
            // Split the last component
            if let Some(parent) = expanded_path.parent() {
                let filename = expanded_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                (parent.to_path_buf(), filename)
            } else {
                (expanded_path, String::new())
            }
        }
    }

    /// Refresh the command cache if it's stale
    fn refresh_command_cache_if_needed(&mut self) -> Result<()> {
        let should_refresh = self
            .cache_updated
            .map(|updated| updated.elapsed() > self.cache_timeout)
            .unwrap_or(true);

        if should_refresh {
            self.refresh_command_cache()?;
        }

        Ok(())
    }

    /// Refresh the command cache by scanning PATH
    fn refresh_command_cache(&mut self) -> Result<()> {
        let mut commands = Vec::new();

        if let Ok(path_env) = env::var("PATH") {
            for path_dir in env::split_paths(&path_env) {
                if let Ok(entries) = fs::read_dir(&path_dir) {
                    for entry in entries.flatten() {
                        if let Ok(filename) = entry.file_name().into_string() {
                            // Check if file is executable
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(metadata) = entry.metadata() {
                                    if metadata.is_file()
                                        && (metadata.permissions().mode() & 0o111 != 0)
                                        && !commands.contains(&filename)
                                    {
                                        commands.push(filename);
                                    }
                                }
                            }

                            #[cfg(windows)]
                            {
                                // On Windows, check for common executable extensions
                                if let Ok(metadata) = entry.metadata() {
                                    if metadata.is_file() {
                                        let is_executable = filename.ends_with(".exe")
                                            || filename.ends_with(".bat")
                                            || filename.ends_with(".cmd")
                                            || filename.ends_with(".ps1");

                                        if is_executable && !commands.contains(&filename) {
                                            commands.push(filename);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add common shell built-ins
        for builtin in Self::get_shell_builtins() {
            if !commands.contains(&builtin) {
                commands.push(builtin);
            }
        }

        commands.sort();
        self.command_cache = commands;
        self.cache_updated = Some(std::time::Instant::now());

        Ok(())
    }

    /// Get list of common shell built-in commands
    fn get_shell_builtins() -> Vec<String> {
        vec![
            "cd", "ls", "pwd", "echo", "cat", "grep", "sed", "awk", "find", "which", "whereis",
            "man", "help", "exit", "source", "alias", "unalias", "export", "set", "unset",
            "history", "pushd", "popd", "dirs", "fg", "bg", "jobs", "kill", "clear", "printf",
            "read", "test", "true", "false",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }

    /// Get list of common commands for empty input
    fn get_common_commands(&mut self) -> Vec<CompletionItem> {
        let common = vec![
            "ls", "cd", "pwd", "cat", "echo", "grep", "find", "mkdir", "rm", "cp", "mv", "touch",
            "chmod", "chown", "ps", "top", "git", "vim", "nano", "make", "curl", "wget", "ssh",
            "scp",
        ];

        common
            .into_iter()
            .map(|cmd| CompletionItem {
                text: cmd.to_string(),
                label: cmd.to_string(),
                item_type: CompletionItemType::Command,
                description: None,
            })
            .collect()
    }

    /// Get command-specific argument completions (for special cases like cd)
    pub fn get_argument_completions(
        &self,
        command: &str,
        arg: &str,
        working_dir: &Path,
    ) -> Result<CompletionResult> {
        match command {
            "cd" | "pushd" => {
                // Only show directories for cd
                let mut result = self.complete_path(arg, working_dir)?;
                result
                    .suggestions
                    .retain(|item| item.item_type == CompletionItemType::Directory);
                Ok(result)
            }
            _ => self.complete_path(arg, working_dir),
        }
    }
}

impl Default for CompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionResult {
    /// Check if there are any suggestions
    pub fn is_empty(&self) -> bool {
        self.suggestions.is_empty()
    }

    /// Get the number of suggestions
    pub fn len(&self) -> usize {
        self.suggestions.len()
    }

    /// Get the common prefix of all suggestions (for automatic completion)
    pub fn get_common_prefix(&self) -> Option<String> {
        if self.suggestions.is_empty() {
            return None;
        }

        if self.suggestions.len() == 1 {
            return Some(self.suggestions[0].text.clone());
        }

        // Find common prefix among all suggestions
        let first = &self.suggestions[0].text;
        let mut common = String::new();

        for (i, ch) in first.chars().enumerate() {
            if self
                .suggestions
                .iter()
                .all(|s| s.text.chars().nth(i) == Some(ch))
            {
                common.push(ch);
            } else {
                break;
            }
        }

        if common.len() > self.prefix.len() {
            Some(common)
        } else {
            None
        }
    }
}

impl CompletionItem {
    /// Get an icon/emoji for the completion item
    pub fn get_icon(&self) -> &str {
        match self.item_type {
            CompletionItemType::Command => "⚡",
            CompletionItemType::Directory => "📁",
            CompletionItemType::File => "📄",
            CompletionItemType::Symlink => "🔗",
            CompletionItemType::Special => "✨",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_completion_provider_creation() {
        let provider = CompletionProvider::new();
        assert!(provider.command_cache.is_empty());
        assert!(provider.cache_updated.is_none());
    }

    #[test]
    fn test_parse_path_prefix() {
        let provider = CompletionProvider::new();
        let working_dir = PathBuf::from("/home/user");

        let (dir, file) = provider.parse_path_prefix("test", &working_dir);
        assert_eq!(dir, working_dir);
        assert_eq!(file, "test");

        let (dir, file) = provider.parse_path_prefix("test/", &working_dir);
        assert_eq!(dir, working_dir.join("test"));
        assert_eq!(file, "");
    }

    #[test]
    fn test_common_prefix() {
        let result = CompletionResult {
            suggestions: vec![
                CompletionItem {
                    text: "test1".to_string(),
                    label: "test1".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
                CompletionItem {
                    text: "test2".to_string(),
                    label: "test2".to_string(),
                    item_type: CompletionItemType::File,
                    description: None,
                },
            ],
            prefix: "te".to_string(),
            completion_type: CompletionType::Path,
        };

        assert_eq!(result.get_common_prefix(), Some("test".to_string()));
    }
}
