#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::claude::ModelAlias;
use crate::hooks::HooksConfig;
use crate::notifications::{NotificationEvent, NotificationsConfig};

/// Main configuration for Doodoori
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DoodooriConfig {
    /// Default model to use
    pub default_model: String,
    /// Maximum iterations for the loop engine
    pub max_iterations: u32,
    /// Default budget limit in USD (None = unlimited)
    pub budget_limit: Option<f64>,
    /// Enable YOLO mode by default
    pub yolo_mode: bool,
    /// Enable sandbox mode by default
    pub sandbox_mode: bool,
    /// Custom price.toml path
    pub price_file: Option<PathBuf>,
    /// Custom instructions file (like doodoori.md)
    pub instructions_file: Option<PathBuf>,
    /// Git workflow settings
    pub git: GitConfig,
    /// Logging settings
    pub logging: LoggingConfig,
    /// Parallel execution settings
    pub parallel: ParallelConfig,
    /// Hooks configuration
    pub hooks: HooksConfigFile,
    /// Notifications configuration
    pub notifications: NotificationsConfigFile,
}

impl Default for DoodooriConfig {
    fn default() -> Self {
        Self {
            default_model: "sonnet".to_string(),
            max_iterations: 50,
            budget_limit: None,
            yolo_mode: false,
            sandbox_mode: false,
            price_file: None,
            instructions_file: Some(PathBuf::from("doodoori.md")),
            git: GitConfig::default(),
            logging: LoggingConfig::default(),
            parallel: ParallelConfig::default(),
            hooks: HooksConfigFile::default(),
            notifications: NotificationsConfigFile::default(),
        }
    }
}

/// Git workflow configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GitConfig {
    /// Enable git workflow (branch, commit, PR)
    pub enabled: bool,
    /// Auto-create feature branches
    pub auto_branch: bool,
    /// Auto-commit on completion
    pub auto_commit: bool,
    /// Auto-create PR on completion
    pub auto_pr: bool,
    /// Auto-merge PR if CI passes
    pub auto_merge: bool,
    /// Branch prefix for feature branches
    pub branch_prefix: String,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_branch: true,
            auto_commit: true,
            auto_pr: false,
            auto_merge: false,
            branch_prefix: "doodoori/".to_string(),
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Log to file
    pub file: Option<PathBuf>,
    /// Show progress indicators
    pub progress: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            progress: true,
        }
    }
}

/// Parallel execution configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ParallelConfig {
    /// Default number of workers
    pub workers: usize,
    /// Isolate workspaces per task
    pub isolate_workspaces: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            workers: 3,
            isolate_workspaces: false,
        }
    }
}

/// Hooks configuration for TOML file (simple string paths)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HooksConfigFile {
    /// Enable hooks
    pub enabled: bool,
    /// Pre-run hook script path
    pub pre_run: Option<String>,
    /// Post-run hook script path
    pub post_run: Option<String>,
    /// On-error hook script path
    pub on_error: Option<String>,
    /// On-iteration hook script path
    pub on_iteration: Option<String>,
    /// On-complete hook script path
    pub on_complete: Option<String>,
    /// Default timeout for hooks in seconds
    pub timeout_secs: u64,
}

impl HooksConfigFile {
    /// Convert to HooksConfig for use with HookExecutor
    pub fn to_hooks_config(&self) -> HooksConfig {
        if !self.enabled {
            return HooksConfig::default().disabled();
        }

        HooksConfig::from_paths(
            self.pre_run.as_deref(),
            self.post_run.as_deref(),
            self.on_error.as_deref(),
            self.on_iteration.as_deref(),
            self.on_complete.as_deref(),
        )
    }
}

/// Notifications configuration for TOML file
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NotificationsConfigFile {
    /// Enable notifications globally
    pub enabled: bool,
    /// Slack webhook URL
    pub slack_webhook: Option<String>,
    /// Discord webhook URL
    pub discord_webhook: Option<String>,
    /// Generic webhook URL
    pub webhook_url: Option<String>,
    /// Events to notify on (completed, error, started, etc.)
    #[serde(default = "default_notification_events")]
    pub events: Vec<String>,
}

fn default_notification_events() -> Vec<String> {
    vec!["completed".to_string(), "error".to_string()]
}

impl NotificationsConfigFile {
    /// Convert to NotificationsConfig for use with NotificationManager
    pub fn to_notifications_config(&self) -> NotificationsConfig {
        use crate::notifications::{SlackConfig, DiscordConfig, WebhookConfig};

        if !self.enabled {
            return NotificationsConfig::default();
        }

        let events: Vec<NotificationEvent> = self.events
            .iter()
            .filter_map(|e| match e.as_str() {
                "started" => Some(NotificationEvent::Started),
                "completed" => Some(NotificationEvent::Completed),
                "error" => Some(NotificationEvent::Error),
                "budget_exceeded" => Some(NotificationEvent::BudgetExceeded),
                "max_iterations" => Some(NotificationEvent::MaxIterations),
                _ => None,
            })
            .collect();

        let mut config = NotificationsConfig {
            enabled: true,
            ..Default::default()
        };

        if let Some(ref url) = self.slack_webhook {
            config.slack = Some(SlackConfig {
                webhook_url: url.clone(),
                channel: None,
                username: None,
                icon_emoji: None,
                events: events.clone(),
            });
        }

        if let Some(ref url) = self.discord_webhook {
            config.discord = Some(DiscordConfig {
                webhook_url: url.clone(),
                username: None,
                avatar_url: None,
                events: events.clone(),
            });
        }

        if let Some(ref url) = self.webhook_url {
            config.webhooks.push(WebhookConfig {
                url: url.clone(),
                method: "POST".to_string(),
                headers: std::collections::HashMap::new(),
                events: events.clone(),
                timeout_secs: 30,
            });
        }

        config
    }
}

impl DoodooriConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            tracing::debug!("Config file not found: {}, using defaults", path.display());
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::from_str(&content)
    }

    /// Parse configuration from a TOML string
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse doodoori.toml")
    }

    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        Self::from_file(Path::new("doodoori.toml"))
    }

    /// Get the default model as ModelAlias
    pub fn default_model_alias(&self) -> ModelAlias {
        self.default_model
            .parse()
            .unwrap_or(ModelAlias::Sonnet)
    }

    /// Get the instructions file path if it exists
    pub fn get_instructions_file(&self) -> Option<PathBuf> {
        self.instructions_file.as_ref().and_then(|p| {
            if p.exists() {
                Some(p.clone())
            } else {
                None
            }
        })
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config to {}", path.display()))?;
        Ok(())
    }

    /// Generate a default config file content
    pub fn default_config_string() -> String {
        r#"# Doodoori Configuration
# This file configures default behavior for the doodoori CLI

# Default model: haiku, sonnet, or opus
default_model = "sonnet"

# Maximum iterations for the loop engine (default: 50)
max_iterations = 50

# Budget limit in USD (comment out for unlimited)
# budget_limit = 10.0

# Enable YOLO mode by default (skip all permissions)
yolo_mode = false

# Enable sandbox mode by default (Docker isolation)
sandbox_mode = false

# Custom price.toml path (optional)
# price_file = "custom-price.toml"

# Instructions file (like CLAUDE.md for doodoori)
instructions_file = "doodoori.md"

[git]
# Enable git workflow (branch, commit, PR)
enabled = true
# Auto-create feature branches
auto_branch = true
# Auto-commit on completion
auto_commit = true
# Auto-create PR on completion
auto_pr = false
# Auto-merge PR if CI passes
auto_merge = false
# Branch prefix for feature branches
branch_prefix = "doodoori/"

[logging]
# Log level: trace, debug, info, warn, error
level = "info"
# Log to file (optional)
# file = ".doodoori/logs/doodoori.log"
# Show progress indicators
progress = true

[parallel]
# Default number of parallel workers
workers = 3
# Isolate workspaces per task
isolate_workspaces = false

[hooks]
# Enable hooks
enabled = true
# Hook timeout in seconds (default: 60)
timeout_secs = 60
# Pre-run hook (before task execution)
# pre_run = "scripts/pre_run.sh"
# Post-run hook (after task execution)
# post_run = "scripts/post_run.sh"
# On-error hook (when an error occurs)
# on_error = "scripts/on_error.sh"
# On-iteration hook (after each loop iteration)
# on_iteration = "scripts/on_iteration.sh"
# On-complete hook (when task completes successfully)
# on_complete = "scripts/on_complete.sh"

[notifications]
# Enable notifications
enabled = false
# Slack webhook URL
# slack_webhook = "https://hooks.slack.com/services/..."
# Discord webhook URL
# discord_webhook = "https://discord.com/api/webhooks/..."
# Generic webhook URL
# webhook_url = "https://your-api.com/webhook"
# Events to notify on: started, completed, error, budget_exceeded, max_iterations
events = ["completed", "error"]
"#.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DoodooriConfig::default();
        assert_eq!(config.default_model, "sonnet");
        assert_eq!(config.max_iterations, 50);
        assert!(config.budget_limit.is_none());
        assert!(!config.yolo_mode);
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
default_model = "opus"
max_iterations = 100
budget_limit = 5.0
yolo_mode = true

[git]
enabled = false
auto_pr = true

[parallel]
workers = 8
"#;
        let config = DoodooriConfig::from_str(toml).unwrap();

        assert_eq!(config.default_model, "opus");
        assert_eq!(config.max_iterations, 100);
        assert_eq!(config.budget_limit, Some(5.0));
        assert!(config.yolo_mode);
        assert!(!config.git.enabled);
        assert!(config.git.auto_pr);
        assert_eq!(config.parallel.workers, 8);
    }

    #[test]
    fn test_partial_config() {
        let toml = r#"
default_model = "haiku"
"#;
        let config = DoodooriConfig::from_str(toml).unwrap();

        // Should use defaults for unspecified fields
        assert_eq!(config.default_model, "haiku");
        assert_eq!(config.max_iterations, 50);
        assert!(config.git.enabled);
    }

    #[test]
    fn test_default_model_alias() {
        let config = DoodooriConfig::default();
        assert_eq!(config.default_model_alias(), ModelAlias::Sonnet);

        let config = DoodooriConfig {
            default_model: "opus".to_string(),
            ..Default::default()
        };
        assert_eq!(config.default_model_alias(), ModelAlias::Opus);
    }

    #[test]
    fn test_git_config_defaults() {
        let config = GitConfig::default();
        assert!(config.enabled);
        assert!(config.auto_branch);
        assert!(config.auto_commit);
        assert!(!config.auto_pr);
        assert!(!config.auto_merge);
        assert_eq!(config.branch_prefix, "doodoori/");
    }

    #[test]
    fn test_logging_config_defaults() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert!(config.file.is_none());
        assert!(config.progress);
    }

    #[test]
    fn test_parallel_config_defaults() {
        let config = ParallelConfig::default();
        assert_eq!(config.workers, 3);
        assert!(!config.isolate_workspaces);
    }

    #[test]
    fn test_default_config_string() {
        let content = DoodooriConfig::default_config_string();
        assert!(content.contains("default_model"));
        assert!(content.contains("max_iterations"));
        assert!(content.contains("[git]"));
        assert!(content.contains("[logging]"));
        assert!(content.contains("[parallel]"));
    }
}
