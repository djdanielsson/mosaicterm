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
    GoVersion,
    JavaHome,
    RustToolchain,
    Docker,
    Kubernetes,
    AwsProfile,
    Terraform,
    Mise,
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
            ContextType::PythonVenv => format!("venv:{}", self.display_name),
            ContextType::Conda => format!("conda:{}", self.display_name),
            ContextType::NvmNode => format!("node:{}", self.display_name),
            ContextType::Rbenv | ContextType::Rvm => format!("ruby:{}", self.display_name),
            ContextType::Direnv => "direnv".to_string(),
            ContextType::GoVersion => format!("go:{}", self.display_name),
            ContextType::JavaHome => format!("java:{}", self.display_name),
            ContextType::RustToolchain => format!("rust:{}", self.display_name),
            ContextType::Docker => format!("docker:{}", self.display_name),
            ContextType::Kubernetes => format!("k8s:{}", self.display_name),
            ContextType::AwsProfile => format!("aws:{}", self.display_name),
            ContextType::Terraform => format!("tf:{}", self.display_name),
            ContextType::Mise => format!("mise:{}", self.display_name),
        }
    }

    /// Get a short format for use in prompts
    pub fn format_short(&self) -> String {
        self.format()
    }
}

pub struct ContextDetector {}

impl ContextDetector {
    pub fn new() -> Self {
        Self {}
    }

    /// Detect active contexts from environment variables.
    /// If `working_dir` is provided, directory-sensitive contexts (like Rust)
    /// are only shown when relevant project files exist.
    pub fn detect_contexts(&self, env: &HashMap<String, String>) -> Vec<EnvironmentContext> {
        self.detect_contexts_with_dir(env, None)
    }

    /// Detect active contexts with optional working directory awareness
    pub fn detect_contexts_with_dir(
        &self,
        env: &HashMap<String, String>,
        working_dir: Option<&Path>,
    ) -> Vec<EnvironmentContext> {
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

        // Go version - only show when inside a Go project
        if let Some(goversion) = env.get("GOVERSION") {
            if !goversion.is_empty() {
                let in_go_project = working_dir.map(is_go_project_dir).unwrap_or(true);
                if in_go_project {
                    contexts.push(EnvironmentContext {
                        context_type: ContextType::GoVersion,
                        display_name: goversion.trim_start_matches("go").to_string(),
                    });
                }
            }
        }

        // Java - only show when inside a Java project
        if let Some(java_home) = env.get("JAVA_HOME") {
            if !java_home.is_empty() {
                let in_java_project = working_dir.map(is_java_project_dir).unwrap_or(true);
                if in_java_project {
                    let version = Path::new(java_home)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("active")
                        .to_string();
                    contexts.push(EnvironmentContext {
                        context_type: ContextType::JavaHome,
                        display_name: version,
                    });
                }
            }
        }

        // Rust toolchain - only show when inside a Rust project directory
        if let Some(toolchain) = env.get("RUSTUP_TOOLCHAIN") {
            if !toolchain.is_empty() {
                let in_rust_project = working_dir.map(is_rust_project_dir).unwrap_or(true); // default to showing if no dir info
                if in_rust_project {
                    contexts.push(EnvironmentContext {
                        context_type: ContextType::RustToolchain,
                        display_name: toolchain.clone(),
                    });
                }
            }
        }

        // Docker
        if env.get("DOCKER_HOST").is_some() || env.get("DOCKER_CONTEXT").is_some() {
            let name = env
                .get("DOCKER_CONTEXT")
                .or(env.get("DOCKER_HOST"))
                .cloned()
                .unwrap_or_else(|| "active".to_string());
            contexts.push(EnvironmentContext {
                context_type: ContextType::Docker,
                display_name: name,
            });
        }

        // Kubernetes
        if let Some(kube) = env.get("KUBECONFIG") {
            if !kube.is_empty() {
                let ctx_name = Path::new(kube)
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("active")
                    .to_string();
                contexts.push(EnvironmentContext {
                    context_type: ContextType::Kubernetes,
                    display_name: ctx_name,
                });
            }
        }

        // AWS
        if let Some(profile) = env.get("AWS_PROFILE").or(env.get("AWS_DEFAULT_PROFILE")) {
            if !profile.is_empty() {
                contexts.push(EnvironmentContext {
                    context_type: ContextType::AwsProfile,
                    display_name: profile.clone(),
                });
            }
        }

        // Terraform
        if let Some(workspace) = env.get("TF_WORKSPACE") {
            if !workspace.is_empty() {
                contexts.push(EnvironmentContext {
                    context_type: ContextType::Terraform,
                    display_name: workspace.clone(),
                });
            }
        }

        // mise/asdf
        let mise_active = env.keys().any(|k| k.starts_with("MISE_"));
        let asdf_active = env.keys().any(|k| k.starts_with("ASDF_"));
        if mise_active || asdf_active {
            let name = if mise_active { "mise" } else { "asdf" };
            contexts.push(EnvironmentContext {
                context_type: ContextType::Mise,
                display_name: name.to_string(),
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

/// Check if the given directory (or any ancestor up to 5 levels) is a Rust project
fn is_rust_project_dir(dir: &Path) -> bool {
    let markers = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
    ];
    let mut current = Some(dir);
    let mut depth = 0;
    while let Some(d) = current {
        if depth > 5 {
            break;
        }
        for marker in &markers {
            if d.join(marker).exists() {
                return true;
            }
        }
        current = d.parent();
        depth += 1;
    }
    false
}

/// Check if the given directory (or any ancestor up to 5 levels) is a Go project
fn is_go_project_dir(dir: &Path) -> bool {
    let markers = ["go.mod", "go.sum"];
    let mut current = Some(dir);
    let mut depth = 0;
    while let Some(d) = current {
        if depth > 5 {
            break;
        }
        for marker in &markers {
            if d.join(marker).exists() {
                return true;
            }
        }
        current = d.parent();
        depth += 1;
    }
    false
}

/// Check if the given directory (or any ancestor up to 5 levels) is a Java project
fn is_java_project_dir(dir: &Path) -> bool {
    let markers = [
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        ".java-version",
    ];
    let mut current = Some(dir);
    let mut depth = 0;
    while let Some(d) = current {
        if depth > 5 {
            break;
        }
        for marker in &markers {
            if d.join(marker).exists() {
                return true;
            }
        }
        current = d.parent();
        depth += 1;
    }
    false
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
