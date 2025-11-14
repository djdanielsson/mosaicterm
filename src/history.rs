//! Persistent command history management
//!
//! This module handles saving and loading command history from a file,
//! and provides search functionality (fuzzy or regex-based).

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Default history file name
const DEFAULT_HISTORY_FILE: &str = ".mosaicterm_history";

/// Maximum number of history entries to keep
const MAX_HISTORY_ENTRIES: usize = 10000;

/// Persistent command history manager
pub struct HistoryManager {
    /// Path to the history file
    history_file: PathBuf,
    /// In-memory history cache
    history: VecDeque<String>,
    /// Maximum history size
    max_size: usize,
}

impl HistoryManager {
    /// Create a new history manager
    pub fn new() -> Result<Self> {
        let history_file = Self::default_history_path()?;
        let mut manager = Self {
            history_file,
            history: VecDeque::new(),
            max_size: MAX_HISTORY_ENTRIES,
        };
        manager.load()?;
        Ok(manager)
    }

    /// Create with custom history file path
    pub fn with_path(path: PathBuf) -> Result<Self> {
        let mut manager = Self {
            history_file: path,
            history: VecDeque::new(),
            max_size: MAX_HISTORY_ENTRIES,
        };
        manager.load()?;
        Ok(manager)
    }

    /// Get the default history file path
    fn default_history_path() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| crate::error::Error::Other("Could not find home directory".to_string()))?;
        Ok(PathBuf::from(home).join(DEFAULT_HISTORY_FILE))
    }

    /// Load history from file
    pub fn load(&mut self) -> Result<()> {
        if !self.history_file.exists() {
            // Create empty history file
            File::create(&self.history_file)?;
            return Ok(());
        }

        let file = File::open(&self.history_file)?;
        let reader = BufReader::new(file);

        self.history.clear();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                self.history.push_back(line);
            }
        }

        // Trim to max size
        while self.history.len() > self.max_size {
            self.history.pop_front();
        }

        Ok(())
    }

    /// Save history to file
    pub fn save(&self) -> Result<()> {
        let mut file = File::create(&self.history_file)?;
        for entry in &self.history {
            writeln!(file, "{}", entry)?;
        }
        Ok(())
    }

    /// Add a command to history
    pub fn add(&mut self, command: String) -> Result<()> {
        if command.trim().is_empty() {
            return Ok(());
        }

        // Remove duplicates - keep only the most recent occurrence
        self.history.retain(|c| c != &command);
        
        // Add to end
        self.history.push_back(command.clone());

        // Trim to max size
        while self.history.len() > self.max_size {
            self.history.pop_front();
        }

        // Append to file (for persistence across sessions)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history_file)?;
        writeln!(file, "{}", command)?;

        Ok(())
    }

    /// Get all history entries
    pub fn entries(&self) -> &VecDeque<String> {
        &self.history
    }

    /// Search history with fuzzy matching
    pub fn search(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return self.history.iter().rev().cloned().collect();
        }

        let query_lower = query.to_lowercase();
        
        // Try fuzzy matching first
        let mut results: Vec<(usize, String)> = self.history
            .iter()
            .filter_map(|entry| {
                let entry_lower = entry.to_lowercase();
                
                // Simple fuzzy matching: check if all query chars appear in order
                let score = fuzzy_score(&query_lower, &entry_lower);
                if score > 0 {
                    Some((score, entry.clone()))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending) and recency (later entries are better)
        results.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        results.into_iter()
            .filter_map(|(_, entry)| {
                if seen.insert(entry.clone()) {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Search history with regex
    pub fn search_regex(&self, pattern: &str) -> Result<Vec<String>> {
        let re = regex::Regex::new(pattern)?;
        
        let mut results: Vec<String> = self.history
            .iter()
            .filter(|entry| re.is_match(entry))
            .cloned()
            .collect();
        
        // Reverse to show most recent first
        results.reverse();
        
        // Remove duplicates
        let mut seen = std::collections::HashSet::new();
        Ok(results.into_iter()
            .filter(|entry| seen.insert(entry.clone()))
            .collect())
    }

    /// Clear all history
    pub fn clear(&mut self) -> Result<()> {
        self.history.clear();
        File::create(&self.history_file)?;
        Ok(())
    }

    /// Get history file path
    pub fn history_file(&self) -> &Path {
        &self.history_file
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            history_file: PathBuf::from(".mosaicterm_history"),
            history: VecDeque::new(),
            max_size: MAX_HISTORY_ENTRIES,
        })
    }
}

/// Simple fuzzy scoring algorithm
/// Returns a score based on how well the query matches the target
fn fuzzy_score(query: &str, target: &str) -> usize {
    let query_chars: Vec<char> = query.chars().collect();
    let target_chars: Vec<char> = target.chars().collect();
    
    let mut query_idx = 0;
    let mut target_idx = 0;
    let mut score = 0;
    let mut consecutive = 0;
    
    while query_idx < query_chars.len() && target_idx < target_chars.len() {
        if query_chars[query_idx] == target_chars[target_idx] {
            score += 1 + consecutive * 5; // Bonus for consecutive matches
            consecutive += 1;
            query_idx += 1;
        } else {
            consecutive = 0;
        }
        target_idx += 1;
    }
    
    // Only count as a match if all query characters were found
    if query_idx == query_chars.len() {
        score
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_score() {
        assert!(fuzzy_score("ls", "ls -la") > 0);
        assert!(fuzzy_score("gst", "git status") > 0);
        assert!(fuzzy_score("gco", "git checkout") > 0);
        assert_eq!(fuzzy_score("xyz", "abc"), 0);
    }

    #[test]
    fn test_fuzzy_score_consecutive() {
        // Consecutive matches should score higher
        let score1 = fuzzy_score("test", "test_file.rs");
        let score2 = fuzzy_score("test", "t_e_s_t.rs");
        assert!(score1 > score2);
    }

    #[test]
    fn test_search_empty_query() {
        let mut manager = HistoryManager::default();
        manager.history.push_back("ls".to_string());
        manager.history.push_back("pwd".to_string());
        
        let results = manager.search("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_fuzzy() {
        let mut manager = HistoryManager::default();
        manager.history.push_back("git status".to_string());
        manager.history.push_back("git commit".to_string());
        manager.history.push_back("ls -la".to_string());
        
        let results = manager.search("gst");
        assert!(results.iter().any(|r| r.contains("git status")));
    }

    #[test]
    fn test_add_removes_duplicates() {
        let mut manager = HistoryManager::default();
        manager.add("ls".to_string()).unwrap();
        manager.add("pwd".to_string()).unwrap();
        manager.add("ls".to_string()).unwrap();
        
        // Should only have 2 entries, with "ls" at the end
        assert_eq!(manager.history.len(), 2);
        assert_eq!(manager.history.back().unwrap(), "ls");
    }
}

