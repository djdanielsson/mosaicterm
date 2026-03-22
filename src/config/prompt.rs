use std::env;
use std::path::Path;

use eframe::egui;

use crate::models::config::{PromptSegmentConfig, PromptStyle};

/// A rendered prompt segment with styling info
#[derive(Debug, Clone)]
pub struct PromptSegment {
    pub text: String,
    pub fg: egui::Color32,
    pub bg: Option<egui::Color32>,
    pub bold: bool,
    pub separator: Option<char>,
}

/// Git status for prompt display
#[derive(Debug, Clone, Default)]
pub struct GitPromptStatus {
    pub branch: String,
    pub staged: usize,
    pub modified: usize,
    pub untracked: usize,
    pub ahead: usize,
    pub behind: usize,
    pub detached: bool,
}

impl GitPromptStatus {
    /// Parse a context string like "main +2 !3 ?1" or "main *" back into a GitPromptStatus
    pub fn from_context_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }
        let branch = parts[0].to_string();
        let mut status = Self {
            branch,
            ..Default::default()
        };
        for part in &parts[1..] {
            if let Some(n) = part.strip_prefix('+') {
                status.staged = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix('!') {
                status.modified = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix('?') {
                status.untracked = n.parse().unwrap_or(0);
            } else if part.contains('\u{2191}') || part.contains('\u{2193}') {
                // ahead/behind arrows
            } else if *part == "*" {
                status.modified = 1;
            }
        }
        Some(status)
    }

    pub fn format_compact(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.branch.clone());
        if self.staged > 0 {
            parts.push(format!("+{}", self.staged));
        }
        if self.modified > 0 {
            parts.push(format!("!{}", self.modified));
        }
        if self.untracked > 0 {
            parts.push(format!("?{}", self.untracked));
        }
        if self.ahead > 0 || self.behind > 0 {
            let mut sync = String::new();
            if self.ahead > 0 {
                sync.push_str(&format!("\u{2191}{}", self.ahead));
            }
            if self.behind > 0 {
                sync.push_str(&format!("\u{2193}{}", self.behind));
            }
            parts.push(sync);
        }
        parts.join(" ")
    }
}

/// Environment context for prompt display
#[derive(Debug, Clone)]
pub struct EnvPromptContext {
    pub name: String,
    pub value: String,
}

/// Prompt formatter that handles variable substitution and segment rendering
#[derive(Debug, Clone)]
pub struct PromptFormatter {
    format: String,
    style: PromptStyle,
    custom_segments: Vec<PromptSegmentConfig>,
}

impl PromptFormatter {
    pub fn new(format: String) -> Self {
        Self {
            format,
            style: PromptStyle::default(),
            custom_segments: Vec::new(),
        }
    }

    pub fn with_style(mut self, style: PromptStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_custom_segments(mut self, segments: Vec<PromptSegmentConfig>) -> Self {
        self.custom_segments = segments;
        self
    }

    pub fn style(&self) -> &PromptStyle {
        &self.style
    }

    pub fn render(&self, working_dir: &Path) -> String {
        self.substitute_vars(&self.format, working_dir, None, &[])
    }

    pub fn render_segments(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        match self.style {
            PromptStyle::Classic => self.render_classic(working_dir, git_status, env_contexts),
            PromptStyle::Minimal => self.render_minimal(working_dir, git_status, env_contexts),
            PromptStyle::Powerline => self.render_powerline(working_dir, git_status, env_contexts),
            PromptStyle::Starship => self.render_starship(working_dir, git_status, env_contexts),
            PromptStyle::OhMyZsh => self.render_ohmyzsh(working_dir, git_status, env_contexts),
            PromptStyle::Custom => {
                self.render_custom_segments(working_dir, git_status, env_contexts)
            }
        }
    }

    fn render_classic(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();

        for ctx in env_contexts {
            segments.push(PromptSegment {
                text: format!("({}:{}) ", ctx.name, ctx.value),
                fg: egui::Color32::from_rgb(255, 200, 100),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        let user = get_user();
        let hostname = get_hostname();
        let pwd = format_pwd(working_dir);
        segments.push(PromptSegment {
            text: format!("{}@{}:{}$ ", user, hostname, pwd),
            fg: egui::Color32::from_rgb(100, 200, 100),
            bg: None,
            bold: true,
            separator: None,
        });

        if let Some(git) = git_status {
            segments.push(PromptSegment {
                text: format!("[{}] ", git.format_compact()),
                fg: egui::Color32::from_rgb(200, 200, 255),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        segments
    }

    fn render_minimal(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();

        for ctx in env_contexts {
            segments.push(PromptSegment {
                text: format!("({}) ", ctx.value),
                fg: egui::Color32::from_rgb(255, 200, 100),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        let pwd = format_pwd(working_dir);
        segments.push(PromptSegment {
            text: format!("{} ", pwd),
            fg: egui::Color32::from_rgb(100, 180, 255),
            bg: None,
            bold: true,
            separator: None,
        });

        if let Some(git) = git_status {
            segments.push(PromptSegment {
                text: format!("{} ", git.format_compact()),
                fg: egui::Color32::from_rgb(150, 220, 150),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        segments.push(PromptSegment {
            text: "> ".to_string(),
            fg: egui::Color32::from_rgb(180, 180, 200),
            bg: None,
            bold: false,
            separator: None,
        });

        segments
    }

    fn render_powerline(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();
        let arrow = '\u{E0B0}';

        for ctx in env_contexts {
            segments.push(PromptSegment {
                text: format!(" {} ", ctx.value),
                fg: egui::Color32::from_rgb(255, 255, 255),
                bg: Some(egui::Color32::from_rgb(60, 60, 100)),
                bold: false,
                separator: Some(arrow),
            });
        }

        let pwd = format_pwd(working_dir);
        segments.push(PromptSegment {
            text: format!(" {} ", pwd),
            fg: egui::Color32::from_rgb(255, 255, 255),
            bg: Some(egui::Color32::from_rgb(36, 114, 200)),
            bold: true,
            separator: Some(arrow),
        });

        if let Some(git) = git_status {
            let bg = if git.modified > 0 || git.untracked > 0 {
                egui::Color32::from_rgb(180, 120, 30)
            } else {
                egui::Color32::from_rgb(40, 140, 60)
            };
            segments.push(PromptSegment {
                text: format!(" {} ", git.format_compact()),
                fg: egui::Color32::from_rgb(255, 255, 255),
                bg: Some(bg),
                bold: false,
                separator: Some(arrow),
            });
        }

        segments.push(PromptSegment {
            text: " ".to_string(),
            fg: egui::Color32::from_rgb(180, 180, 200),
            bg: None,
            bold: false,
            separator: None,
        });

        segments
    }

    fn render_starship(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();

        let pwd = format_pwd(working_dir);
        segments.push(PromptSegment {
            text: format!("{} ", pwd),
            fg: egui::Color32::from_rgb(80, 180, 255),
            bg: None,
            bold: true,
            separator: None,
        });

        if let Some(git) = git_status {
            let color = if git.modified > 0 || git.untracked > 0 {
                egui::Color32::from_rgb(255, 100, 100)
            } else {
                egui::Color32::from_rgb(100, 220, 100)
            };
            segments.push(PromptSegment {
                text: format!("{} ", git.format_compact()),
                fg: color,
                bg: None,
                bold: false,
                separator: None,
            });
        }

        for ctx in env_contexts {
            let (icon, color) = match ctx.name.as_str() {
                "venv" | "conda" => ("\u{1F40D}", egui::Color32::from_rgb(80, 180, 80)),
                "node" => ("\u{2B22}", egui::Color32::from_rgb(80, 180, 80)),
                "ruby" => ("\u{1F48E}", egui::Color32::from_rgb(200, 50, 50)),
                "rust" => ("\u{1F980}", egui::Color32::from_rgb(220, 120, 50)),
                "go" => ("Go", egui::Color32::from_rgb(0, 173, 216)),
                "java" => ("J", egui::Color32::from_rgb(200, 50, 50)),
                "docker" => ("\u{1F433}", egui::Color32::from_rgb(0, 130, 200)),
                "k8s" => ("\u{2638}", egui::Color32::from_rgb(50, 100, 220)),
                "aws" => ("AWS", egui::Color32::from_rgb(255, 153, 0)),
                "terraform" => ("TF", egui::Color32::from_rgb(100, 70, 200)),
                _ => ("", egui::Color32::from_rgb(180, 180, 200)),
            };
            let text = if icon.is_empty() {
                format!("{} ", ctx.value)
            } else {
                format!("{} {} ", icon, ctx.value)
            };
            segments.push(PromptSegment {
                text,
                fg: color,
                bg: None,
                bold: false,
                separator: None,
            });
        }

        segments.push(PromptSegment {
            text: "\u{276F} ".to_string(),
            fg: egui::Color32::from_rgb(100, 220, 100),
            bg: None,
            bold: true,
            separator: None,
        });

        segments
    }

    fn render_ohmyzsh(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();

        for ctx in env_contexts {
            segments.push(PromptSegment {
                text: format!("({}) ", ctx.value),
                fg: egui::Color32::from_rgb(255, 200, 100),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        let user = get_user();
        let hostname = get_hostname();
        segments.push(PromptSegment {
            text: format!("{} ", user),
            fg: egui::Color32::from_rgb(0, 210, 210),
            bg: None,
            bold: true,
            separator: None,
        });

        segments.push(PromptSegment {
            text: format!("at {} ", hostname),
            fg: egui::Color32::from_rgb(180, 180, 200),
            bg: None,
            bold: false,
            separator: None,
        });

        let pwd = format_pwd(working_dir);
        segments.push(PromptSegment {
            text: format!("in {} ", pwd),
            fg: egui::Color32::from_rgb(80, 180, 255),
            bg: None,
            bold: true,
            separator: None,
        });

        if let Some(git) = git_status {
            let dirty_marker = if git.modified > 0 || git.untracked > 0 {
                " \u{2717}"
            } else {
                " \u{2713}"
            };
            segments.push(PromptSegment {
                text: format!("git:({}{}) ", git.branch, dirty_marker),
                fg: egui::Color32::from_rgb(200, 200, 255),
                bg: None,
                bold: false,
                separator: None,
            });
        }

        segments.push(PromptSegment {
            text: "\u{279C} ".to_string(),
            fg: egui::Color32::from_rgb(100, 220, 100),
            bg: None,
            bold: true,
            separator: None,
        });

        segments
    }

    fn render_custom_segments(
        &self,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> Vec<PromptSegment> {
        let mut segments = Vec::new();

        for seg_config in &self.custom_segments {
            if let Some(ref cond) = seg_config.condition {
                let should_show = match cond.as_str() {
                    "git" => git_status.is_some(),
                    "venv" => env_contexts.iter().any(|c| c.name == "venv"),
                    "conda" => env_contexts.iter().any(|c| c.name == "conda"),
                    "node" => env_contexts.iter().any(|c| c.name == "node"),
                    "docker" => env_contexts.iter().any(|c| c.name == "docker"),
                    _ => true,
                };
                if !should_show {
                    continue;
                }
            }

            let text =
                self.substitute_vars(&seg_config.content, working_dir, git_status, env_contexts);

            let fg = seg_config
                .fg
                .as_ref()
                .and_then(|c| parse_color_string(c))
                .unwrap_or(egui::Color32::from_rgb(200, 200, 220));
            let bg = seg_config.bg.as_ref().and_then(|c| parse_color_string(c));

            segments.push(PromptSegment {
                text: format!("{} ", text),
                fg,
                bg,
                bold: seg_config.bold,
                separator: None,
            });
        }

        if segments.is_empty() {
            return self.render_minimal(working_dir, git_status, env_contexts);
        }

        segments
    }

    fn substitute_vars(
        &self,
        template: &str,
        working_dir: &Path,
        git_status: Option<&GitPromptStatus>,
        env_contexts: &[EnvPromptContext],
    ) -> String {
        let mut result = template.to_string();

        let user = get_user();
        let hostname = get_hostname();
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/".to_string());
        let pwd = format_pwd(working_dir);
        let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());

        // Escape sequences
        result = result.replace("$$USER", "\x00USER\x00");
        result = result.replace("$$HOSTNAME", "\x00HOSTNAME\x00");
        result = result.replace("$$PWD", "\x00PWD\x00");
        result = result.replace("$$HOME", "\x00HOME\x00");
        result = result.replace("$$SHELL", "\x00SHELL\x00");

        result = result.replace("$USER", &user);
        result = result.replace("$HOSTNAME", &hostname);
        result = result.replace("$PWD", &pwd);
        result = result.replace("$HOME", &home);
        result = result.replace("$SHELL", &shell);

        // New template variables
        if let Some(git) = git_status {
            result = result.replace("$GIT_BRANCH", &git.branch);
            result = result.replace("$GIT_STATUS", &git.format_compact());
        } else {
            result = result.replace("$GIT_BRANCH", "");
            result = result.replace("$GIT_STATUS", "");
        }

        let venv = env_contexts
            .iter()
            .find(|c| c.name == "venv" || c.name == "conda")
            .map(|c| c.value.as_str())
            .unwrap_or("");
        result = result.replace("$VENV", venv);

        let node = env_contexts
            .iter()
            .find(|c| c.name == "node")
            .map(|c| c.value.as_str())
            .unwrap_or("");
        result = result.replace("$NODE_VERSION", node);

        let ruby = env_contexts
            .iter()
            .find(|c| c.name == "ruby")
            .map(|c| c.value.as_str())
            .unwrap_or("");
        result = result.replace("$RUBY_VERSION", ruby);

        let docker = env_contexts
            .iter()
            .find(|c| c.name == "docker")
            .map(|c| c.value.as_str())
            .unwrap_or("");
        result = result.replace("$DOCKER", docker);

        let kube = env_contexts
            .iter()
            .find(|c| c.name == "k8s")
            .map(|c| c.value.as_str())
            .unwrap_or("");
        result = result.replace("$KUBE", kube);

        // Restore escaped
        result = result.replace("\x00USER\x00", "$USER");
        result = result.replace("\x00HOSTNAME\x00", "$HOSTNAME");
        result = result.replace("\x00PWD\x00", "$PWD");
        result = result.replace("\x00HOME\x00", "$HOME");
        result = result.replace("\x00SHELL\x00", "$SHELL");

        result
    }

    pub fn set_format(&mut self, format: String) {
        self.format = format;
    }

    pub fn format(&self) -> &str {
        &self.format
    }
}

impl Default for PromptFormatter {
    fn default() -> Self {
        Self::new("$USER@$HOSTNAME:$PWD$ ".to_string())
    }
}

fn get_user() -> String {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_string())
}

fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string())
}

fn format_pwd(working_dir: &Path) -> String {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/".to_string());

    if working_dir == Path::new(&home) {
        return "~".to_string();
    }

    if let Ok(stripped) = working_dir.strip_prefix(&home) {
        format!("~/{}", stripped.display())
            .trim_end_matches('/')
            .to_string()
    } else {
        working_dir.display().to_string()
    }
}

fn parse_color_string(s: &str) -> Option<egui::Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(egui::Color32::from_rgb(r, g, b))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_formatting() {
        let formatter = PromptFormatter::new("$USER> ".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains(">"));
    }

    #[test]
    fn test_pwd_substitution() {
        let formatter = PromptFormatter::new("[$PWD]$ ".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains("/tmp") || result.contains("~"));
    }

    #[test]
    fn test_hostname_substitution() {
        let formatter = PromptFormatter::new("$HOSTNAME:".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(!result.contains("$HOSTNAME"));
    }

    #[test]
    fn test_multiple_variables() {
        let formatter = PromptFormatter::new("$USER@$HOSTNAME:$PWD$ ".to_string());
        let result = formatter.render(Path::new("/usr"));
        assert!(result.contains("@"));
        assert!(result.contains(":"));
        assert!(result.ends_with("$ "));
    }

    #[test]
    fn test_home_directory_tilde() {
        if let Ok(home) = env::var("HOME") {
            let formatter = PromptFormatter::new("$PWD> ".to_string());
            let result = formatter.render(Path::new(&home));
            assert!(result.starts_with("~>") || result.starts_with("~/>"));
        }
    }

    #[test]
    fn test_escaped_variables() {
        let formatter = PromptFormatter::new("$$USER is $USER".to_string());
        let result = formatter.render(Path::new("/tmp"));
        assert!(result.contains("$USER is "));
    }

    #[test]
    fn test_set_format() {
        let mut formatter = PromptFormatter::new("old".to_string());
        formatter.set_format("new".to_string());
        assert_eq!(formatter.format(), "new");
    }

    #[test]
    fn test_default_format() {
        let formatter = PromptFormatter::default();
        assert_eq!(formatter.format(), "$USER@$HOSTNAME:$PWD$ ");
    }

    #[test]
    fn test_git_prompt_status_format() {
        let status = GitPromptStatus {
            branch: "main".to_string(),
            staged: 2,
            modified: 3,
            untracked: 1,
            ahead: 2,
            behind: 1,
            detached: false,
        };
        let formatted = status.format_compact();
        assert!(formatted.contains("main"));
        assert!(formatted.contains("+2"));
        assert!(formatted.contains("!3"));
        assert!(formatted.contains("?1"));
    }

    #[test]
    fn test_render_segments_minimal() {
        let formatter =
            PromptFormatter::new("$PWD > ".to_string()).with_style(PromptStyle::Minimal);
        let segments = formatter.render_segments(Path::new("/tmp"), None, &[]);
        assert!(!segments.is_empty());
    }

    #[test]
    fn test_render_segments_powerline() {
        let formatter =
            PromptFormatter::new("$PWD > ".to_string()).with_style(PromptStyle::Powerline);
        let git = GitPromptStatus {
            branch: "main".to_string(),
            ..Default::default()
        };
        let segments = formatter.render_segments(Path::new("/tmp"), Some(&git), &[]);
        assert!(segments.len() >= 2);
    }
}
