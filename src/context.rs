//! Environment context detection
//!
//! Detects active environment contexts (venv, nvm, conda, etc.)
//! by examining environment variables.

use std::collections::HashMap;
use std::path::Path;

/// Detected environment context
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentContext {
    /// Context type
    pub context_type: ContextType,
    /// Display name (e.g., "myenv", "node-16.20.0")
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextType {
    PythonVenv,
    Conda,
    NvmNode,
    Rbenv,
    Rvm,
    Direnv,
}

impl EnvironmentContext {
    /// Create a new environment context
    pub fn new(context_type: ContextType, display_name: String) -> Self {
        Self {
            context_type,
            display_name,
        }
    }

    /// Format the context for display
    pub fn format(&self) -> String {
        match self.context_type {
            ContextType::PythonVenv => format!("🐍 venv:{}", self.display_name),
            ContextType::Conda => format!("🐍 conda:{}", self.display_name),
            ContextType::NvmNode => format!("📦 node:{}", self.display_name),
            ContextType::Rbenv => format!("💎 ruby:{}", self.display_name),
            ContextType::Rvm => format!("💎 ruby:{}", self.display_name),
            ContextType::Direnv => "📂 direnv".to_string(),
        }
    }

    /// Get a short format without emoji for use in prompts
    pub fn format_short(&self) -> String {
        match self.context_type {
            ContextType::PythonVenv => format!("venv:{}", self.display_name),
            ContextType::Conda => format!("conda:{}", self.display_name),
            ContextType::NvmNode => format!("node:{}", self.display_name),
            ContextType::Rbenv | ContextType::Rvm => format!("ruby:{}", self.display_name),
            ContextType::Direnv => "direnv".to_string(),
        }
    }
}

pub struct ContextDetector {}

impl ContextDetector {
    pub fn new() -> Self {
        Self {}
    }

    /// Detect active contexts from environment variables
    pub fn detect_contexts(&self, env: &HashMap<String, String>) -> Vec<EnvironmentContext> {
        let mut contexts = Vec::new();

        // Python venv - only if VIRTUAL_ENV is set to a non-empty value
        if let Some(venv_path) = env.get("VIRTUAL_ENV") {
            if !venv_path.is_empty() {
                let name = Path::new(venv_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("venv");
                contexts.push(EnvironmentContext {
                    context_type: ContextType::PythonVenv,
                    display_name: name.to_string(),
                });
            }
        }

        // Conda - only if CONDA_DEFAULT_ENV is set to a non-empty value
        if let Some(conda_env) = env.get("CONDA_DEFAULT_ENV") {
            if !conda_env.is_empty() && (conda_env != "base" || env.get("CONDA_PREFIX").is_some()) {
                contexts.push(EnvironmentContext {
                    context_type: ContextType::Conda,
                    display_name: conda_env.clone(),
                });
            }
        }

        // nvm (Node Version Manager) - only if NVM_BIN is non-empty
        if let Some(nvm_bin) = env.get("NVM_BIN") {
            if !nvm_bin.is_empty() {
                if let Some(version) = extract_node_version(nvm_bin) {
                    contexts.push(EnvironmentContext {
                        context_type: ContextType::NvmNode,
                        display_name: version,
                    });
                }
            }
        }

        // rbenv - only if RBENV_VERSION is non-empty
        if let Some(rbenv_version) = env.get("RBENV_VERSION") {
            if !rbenv_version.is_empty() {
                contexts.push(EnvironmentContext {
                    context_type: ContextType::Rbenv,
                    display_name: rbenv_version.clone(),
                });
            }
        }

        // rvm
        if let Some(rvm_ruby) = env.get("rvm_ruby_string") {
            // Only add if rbenv is not already active
            if !contexts
                .iter()
                .any(|c| matches!(c.context_type, ContextType::Rbenv))
            {
                contexts.push(EnvironmentContext {
                    context_type: ContextType::Rvm,
                    display_name: rvm_ruby.clone(),
                });
            }
        }

        // direnv (check if DIRENV_DIR is set)
        if env.get("DIRENV_DIR").is_some() {
            contexts.push(EnvironmentContext {
                context_type: ContextType::Direnv,
                display_name: "active".to_string(),
            });
        }

        contexts
    }

    /// Format contexts for display in UI (with emojis)
    pub fn format_contexts(&self, contexts: &[EnvironmentContext]) -> String {
        contexts
            .iter()
            .map(|c| c.format())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Format contexts for display in prompt (without emojis)
    pub fn format_contexts_short(&self, contexts: &[EnvironmentContext]) -> String {
        contexts
            .iter()
            .map(|c| c.format_short())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for ContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract Node version from NVM_BIN path
fn extract_node_version(nvm_bin: &str) -> Option<String> {
    // NVM_BIN typically looks like: /Users/user/.nvm/versions/node/v16.20.0/bin
    let path = Path::new(nvm_bin);
    path.parent()? // Remove /bin
        .file_name()?
        .to_str()?
        .strip_prefix('v')
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_python_venv() {
        let mut env = HashMap::new();
        env.insert(
            "VIRTUAL_ENV".to_string(),
            "/home/user/projects/myenv".to_string(),
        );

        let detector = ContextDetector::new();
        let contexts = detector.detect_contexts(&env);

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].context_type, ContextType::PythonVenv);
        assert_eq!(contexts[0].display_name, "myenv");
    }

    #[test]
    fn test_detect_nvm_node() {
        let mut env = HashMap::new();
        env.insert(
            "NVM_BIN".to_string(),
            "/Users/user/.nvm/versions/node/v16.20.0/bin".to_string(),
        );

        let detector = ContextDetector::new();
        let contexts = detector.detect_contexts(&env);

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].context_type, ContextType::NvmNode);
        assert_eq!(contexts[0].display_name, "16.20.0");
    }

    #[test]
    fn test_detect_conda() {
        let mut env = HashMap::new();
        env.insert("CONDA_DEFAULT_ENV".to_string(), "myenv".to_string());
        env.insert("CONDA_PREFIX".to_string(), "/path/to/conda".to_string());

        let detector = ContextDetector::new();
        let contexts = detector.detect_contexts(&env);

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].context_type, ContextType::Conda);
        assert_eq!(contexts[0].display_name, "myenv");
    }

    #[test]
    fn test_detect_multiple_contexts() {
        let mut env = HashMap::new();
        env.insert(
            "VIRTUAL_ENV".to_string(),
            "/home/user/projects/myenv".to_string(),
        );
        env.insert(
            "NVM_BIN".to_string(),
            "/Users/user/.nvm/versions/node/v16.20.0/bin".to_string(),
        );
        env.insert("DIRENV_DIR".to_string(), "/home/user/project".to_string());

        let detector = ContextDetector::new();
        let contexts = detector.detect_contexts(&env);

        assert_eq!(contexts.len(), 3);
        assert!(contexts
            .iter()
            .any(|c| matches!(c.context_type, ContextType::PythonVenv)));
        assert!(contexts
            .iter()
            .any(|c| matches!(c.context_type, ContextType::NvmNode)));
        assert!(contexts
            .iter()
            .any(|c| matches!(c.context_type, ContextType::Direnv)));
    }

    #[test]
    fn test_format_contexts() {
        let contexts = vec![
            EnvironmentContext::new(ContextType::PythonVenv, "myenv".to_string()),
            EnvironmentContext::new(ContextType::NvmNode, "16.20.0".to_string()),
        ];

        let detector = ContextDetector::new();
        let formatted = detector.format_contexts(&contexts);

        assert!(formatted.contains("venv:myenv"));
        assert!(formatted.contains("node:16.20.0"));
    }

    #[test]
    fn test_format_contexts_short() {
        let contexts = vec![
            EnvironmentContext::new(ContextType::PythonVenv, "myenv".to_string()),
            EnvironmentContext::new(ContextType::NvmNode, "16.20.0".to_string()),
        ];

        let detector = ContextDetector::new();
        let formatted = detector.format_contexts_short(&contexts);

        assert!(formatted.contains("venv:myenv"));
        assert!(formatted.contains("node:16.20.0"));
        // Short format should not contain emojis
        assert!(!formatted.contains("🐍"));
        assert!(!formatted.contains("📦"));
    }

    #[test]
    fn test_extract_node_version() {
        let nvm_bin = "/Users/user/.nvm/versions/node/v16.20.0/bin";
        let version = extract_node_version(nvm_bin);
        assert_eq!(version, Some("16.20.0".to_string()));

        // Test edge cases
        let invalid_path = "/usr/bin";
        assert_eq!(extract_node_version(invalid_path), None);
    }

    #[test]
    fn test_rbenv_and_rvm_not_both() {
        let mut env = HashMap::new();
        env.insert("RBENV_VERSION".to_string(), "3.2.0".to_string());
        env.insert("rvm_ruby_string".to_string(), "3.1.0".to_string());

        let detector = ContextDetector::new();
        let contexts = detector.detect_contexts(&env);

        // Should only have one Ruby context (rbenv takes precedence)
        let ruby_contexts: Vec<_> = contexts
            .iter()
            .filter(|c| matches!(c.context_type, ContextType::Rbenv | ContextType::Rvm))
            .collect();
        assert_eq!(ruby_contexts.len(), 1);
        assert_eq!(ruby_contexts[0].context_type, ContextType::Rbenv);
    }
}
