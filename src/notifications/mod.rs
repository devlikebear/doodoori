//! Notification system for task completion alerts
//!
//! Supports multiple notification channels:
//! - Slack (via webhook)
//! - Discord (via webhook)
//! - Generic HTTP webhooks

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Notification event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    /// Task started
    Started,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Error,
    /// Budget exceeded
    BudgetExceeded,
    /// Max iterations reached
    MaxIterations,
}

impl NotificationEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationEvent::Started => "started",
            NotificationEvent::Completed => "completed",
            NotificationEvent::Error => "error",
            NotificationEvent::BudgetExceeded => "budget_exceeded",
            NotificationEvent::MaxIterations => "max_iterations",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            NotificationEvent::Started => "🚀",
            NotificationEvent::Completed => "✅",
            NotificationEvent::Error => "❌",
            NotificationEvent::BudgetExceeded => "💸",
            NotificationEvent::MaxIterations => "⚠️",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            NotificationEvent::Started => "#3498db",    // Blue
            NotificationEvent::Completed => "#2ecc71",  // Green
            NotificationEvent::Error => "#e74c3c",      // Red
            NotificationEvent::BudgetExceeded => "#f39c12", // Orange
            NotificationEvent::MaxIterations => "#f1c40f",  // Yellow
        }
    }

    /// Discord color as integer
    pub fn discord_color(&self) -> u32 {
        match self {
            NotificationEvent::Started => 0x3498db,
            NotificationEvent::Completed => 0x2ecc71,
            NotificationEvent::Error => 0xe74c3c,
            NotificationEvent::BudgetExceeded => 0xf39c12,
            NotificationEvent::MaxIterations => 0xf1c40f,
        }
    }
}

impl std::fmt::Display for NotificationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Notification payload containing task information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    /// Event type
    pub event: NotificationEvent,
    /// Task ID
    pub task_id: String,
    /// Task prompt/description (truncated)
    pub prompt: String,
    /// Model used
    pub model: String,
    /// Number of iterations
    pub iterations: u32,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message (if any)
    pub error: Option<String>,
    /// Timestamp
    pub timestamp: String,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl NotificationPayload {
    pub fn new(event: NotificationEvent, task_id: impl Into<String>) -> Self {
        Self {
            event,
            task_id: task_id.into(),
            prompt: String::new(),
            model: String::new(),
            iterations: 0,
            cost_usd: 0.0,
            duration_ms: 0,
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        let prompt = prompt.into();
        self.prompt = if prompt.len() > 100 {
            format!("{}...", &prompt[..97])
        } else {
            prompt
        };
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_usd = cost;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Format duration as human-readable string
    pub fn duration_string(&self) -> String {
        let secs = self.duration_ms / 1000;
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Format cost as string
    pub fn cost_string(&self) -> String {
        format!("${:.4}", self.cost_usd)
    }
}

/// Slack webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// Channel override (optional)
    pub channel: Option<String>,
    /// Username override (optional)
    pub username: Option<String>,
    /// Icon emoji (optional)
    pub icon_emoji: Option<String>,
    /// Events to notify on
    #[serde(default = "default_events")]
    pub events: Vec<NotificationEvent>,
}

/// Discord webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Webhook URL
    pub webhook_url: String,
    /// Username override (optional)
    pub username: Option<String>,
    /// Avatar URL (optional)
    pub avatar_url: Option<String>,
    /// Events to notify on
    #[serde(default = "default_events")]
    pub events: Vec<NotificationEvent>,
}

/// Generic HTTP webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// HTTP method (default: POST)
    #[serde(default = "default_method")]
    pub method: String,
    /// Custom headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Events to notify on
    #[serde(default = "default_events")]
    pub events: Vec<NotificationEvent>,
    /// Request timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_events() -> Vec<NotificationEvent> {
    vec![NotificationEvent::Completed, NotificationEvent::Error]
}

fn default_method() -> String {
    "POST".to_string()
}

fn default_timeout_secs() -> u64 {
    30
}

/// Main notifications configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationsConfig {
    /// Enable notifications globally
    #[serde(default)]
    pub enabled: bool,
    /// Slack configuration
    pub slack: Option<SlackConfig>,
    /// Discord configuration
    pub discord: Option<DiscordConfig>,
    /// Generic webhook configurations
    #[serde(default)]
    pub webhooks: Vec<WebhookConfig>,
}

impl NotificationsConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_slack(mut self, config: SlackConfig) -> Self {
        self.slack = Some(config);
        self.enabled = true;
        self
    }

    pub fn with_discord(mut self, config: DiscordConfig) -> Self {
        self.discord = Some(config);
        self.enabled = true;
        self
    }

    pub fn with_webhook(mut self, config: WebhookConfig) -> Self {
        self.webhooks.push(config);
        self.enabled = true;
        self
    }

    pub fn has_any_notifiers(&self) -> bool {
        self.slack.is_some() || self.discord.is_some() || !self.webhooks.is_empty()
    }
}

/// Notifier trait for different notification channels
#[async_trait::async_trait]
pub trait Notifier: Send + Sync {
    /// Send a notification
    async fn notify(&self, payload: &NotificationPayload) -> Result<()>;

    /// Check if this notifier should handle the given event
    fn should_notify(&self, event: NotificationEvent) -> bool;

    /// Get notifier name for logging
    fn name(&self) -> &str;
}

/// Slack notifier implementation
pub struct SlackNotifier {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackNotifier {
    pub fn new(config: SlackConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    fn build_message(&self, payload: &NotificationPayload) -> serde_json::Value {
        let title = format!(
            "{} Doodoori Task {}",
            payload.event.emoji(),
            payload.event.as_str().replace('_', " ").to_uppercase()
        );

        let mut fields = vec![
            serde_json::json!({
                "title": "Task ID",
                "value": &payload.task_id[..8.min(payload.task_id.len())],
                "short": true
            }),
            serde_json::json!({
                "title": "Model",
                "value": &payload.model,
                "short": true
            }),
            serde_json::json!({
                "title": "Iterations",
                "value": payload.iterations.to_string(),
                "short": true
            }),
            serde_json::json!({
                "title": "Cost",
                "value": payload.cost_string(),
                "short": true
            }),
            serde_json::json!({
                "title": "Duration",
                "value": payload.duration_string(),
                "short": true
            }),
        ];

        if !payload.prompt.is_empty() {
            fields.insert(0, serde_json::json!({
                "title": "Task",
                "value": &payload.prompt,
                "short": false
            }));
        }

        if let Some(ref error) = payload.error {
            fields.push(serde_json::json!({
                "title": "Error",
                "value": error,
                "short": false
            }));
        }

        let mut message = serde_json::json!({
            "attachments": [{
                "color": payload.event.color(),
                "title": title,
                "fields": fields,
                "footer": "Doodoori",
                "ts": chrono::Utc::now().timestamp()
            }]
        });

        if let Some(ref channel) = self.config.channel {
            message["channel"] = serde_json::json!(channel);
        }
        if let Some(ref username) = self.config.username {
            message["username"] = serde_json::json!(username);
        }
        if let Some(ref icon) = self.config.icon_emoji {
            message["icon_emoji"] = serde_json::json!(icon);
        }

        message
    }
}

#[async_trait::async_trait]
impl Notifier for SlackNotifier {
    async fn notify(&self, payload: &NotificationPayload) -> Result<()> {
        let message = self.build_message(payload);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&message)
            .send()
            .await
            .context("Failed to send Slack notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Slack webhook returned {}: {}", status, body);
        }

        tracing::info!("Slack notification sent for event: {}", payload.event);
        Ok(())
    }

    fn should_notify(&self, event: NotificationEvent) -> bool {
        self.config.events.contains(&event)
    }

    fn name(&self) -> &str {
        "Slack"
    }
}

/// Discord notifier implementation
pub struct DiscordNotifier {
    config: DiscordConfig,
    client: reqwest::Client,
}

impl DiscordNotifier {
    pub fn new(config: DiscordConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    fn build_message(&self, payload: &NotificationPayload) -> serde_json::Value {
        let title = format!(
            "{} Task {}",
            payload.event.emoji(),
            payload.event.as_str().replace('_', " ").to_uppercase()
        );

        let mut fields = vec![
            serde_json::json!({
                "name": "Task ID",
                "value": &payload.task_id[..8.min(payload.task_id.len())],
                "inline": true
            }),
            serde_json::json!({
                "name": "Model",
                "value": &payload.model,
                "inline": true
            }),
            serde_json::json!({
                "name": "Iterations",
                "value": payload.iterations.to_string(),
                "inline": true
            }),
            serde_json::json!({
                "name": "Cost",
                "value": payload.cost_string(),
                "inline": true
            }),
            serde_json::json!({
                "name": "Duration",
                "value": payload.duration_string(),
                "inline": true
            }),
        ];

        if !payload.prompt.is_empty() {
            fields.insert(0, serde_json::json!({
                "name": "Task",
                "value": &payload.prompt,
                "inline": false
            }));
        }

        if let Some(ref error) = payload.error {
            fields.push(serde_json::json!({
                "name": "Error",
                "value": error,
                "inline": false
            }));
        }

        let mut message = serde_json::json!({
            "embeds": [{
                "title": title,
                "color": payload.event.discord_color(),
                "fields": fields,
                "footer": {
                    "text": "Doodoori"
                },
                "timestamp": &payload.timestamp
            }]
        });

        if let Some(ref username) = self.config.username {
            message["username"] = serde_json::json!(username);
        }
        if let Some(ref avatar) = self.config.avatar_url {
            message["avatar_url"] = serde_json::json!(avatar);
        }

        message
    }
}

#[async_trait::async_trait]
impl Notifier for DiscordNotifier {
    async fn notify(&self, payload: &NotificationPayload) -> Result<()> {
        let message = self.build_message(payload);

        let response = self
            .client
            .post(&self.config.webhook_url)
            .json(&message)
            .send()
            .await
            .context("Failed to send Discord notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Discord webhook returned {}: {}", status, body);
        }

        tracing::info!("Discord notification sent for event: {}", payload.event);
        Ok(())
    }

    fn should_notify(&self, event: NotificationEvent) -> bool {
        self.config.events.contains(&event)
    }

    fn name(&self) -> &str {
        "Discord"
    }
}

/// Generic HTTP webhook notifier
pub struct WebhookNotifier {
    config: WebhookConfig,
    client: reqwest::Client,
}

impl WebhookNotifier {
    pub fn new(config: WebhookConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }
}

#[async_trait::async_trait]
impl Notifier for WebhookNotifier {
    async fn notify(&self, payload: &NotificationPayload) -> Result<()> {
        let mut request = match self.config.method.to_uppercase().as_str() {
            "GET" => self.client.get(&self.config.url),
            "PUT" => self.client.put(&self.config.url),
            "PATCH" => self.client.patch(&self.config.url),
            _ => self.client.post(&self.config.url),
        };

        // Add custom headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Send JSON payload
        let response = request
            .json(payload)
            .send()
            .await
            .context("Failed to send webhook notification")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Webhook returned {}: {}", status, body);
        }

        tracing::info!("Webhook notification sent to {}", self.config.url);
        Ok(())
    }

    fn should_notify(&self, event: NotificationEvent) -> bool {
        self.config.events.contains(&event)
    }

    fn name(&self) -> &str {
        "Webhook"
    }
}

/// Notification manager that handles multiple notifiers
pub struct NotificationManager {
    notifiers: Vec<Box<dyn Notifier>>,
    enabled: bool,
}

impl NotificationManager {
    pub fn new(config: NotificationsConfig) -> Self {
        let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();

        if let Some(slack_config) = config.slack {
            notifiers.push(Box::new(SlackNotifier::new(slack_config)));
        }

        if let Some(discord_config) = config.discord {
            notifiers.push(Box::new(DiscordNotifier::new(discord_config)));
        }

        for webhook_config in config.webhooks {
            notifiers.push(Box::new(WebhookNotifier::new(webhook_config)));
        }

        Self {
            notifiers,
            enabled: config.enabled,
        }
    }

    pub fn from_url(url: &str) -> Result<Self> {
        let config = if url.contains("hooks.slack.com") {
            NotificationsConfig::new().with_slack(SlackConfig {
                webhook_url: url.to_string(),
                channel: None,
                username: None,
                icon_emoji: None,
                events: default_events(),
            })
        } else if url.contains("discord.com/api/webhooks") {
            NotificationsConfig::new().with_discord(DiscordConfig {
                webhook_url: url.to_string(),
                username: None,
                avatar_url: None,
                events: default_events(),
            })
        } else {
            NotificationsConfig::new().with_webhook(WebhookConfig {
                url: url.to_string(),
                method: default_method(),
                headers: HashMap::new(),
                events: default_events(),
                timeout_secs: default_timeout_secs(),
            })
        };

        Ok(Self::new(config))
    }

    /// Check if notifications are enabled and configured
    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.notifiers.is_empty()
    }

    /// Send notification to all configured notifiers
    pub async fn notify(&self, payload: &NotificationPayload) -> Vec<Result<()>> {
        if !self.enabled {
            return vec![];
        }

        let mut results = Vec::new();

        for notifier in &self.notifiers {
            if notifier.should_notify(payload.event) {
                let result = notifier.notify(payload).await;
                if let Err(ref e) = result {
                    tracing::warn!("{} notification failed: {}", notifier.name(), e);
                }
                results.push(result);
            }
        }

        results
    }

    /// Send notification and ignore errors (for non-critical notifications)
    pub async fn notify_silent(&self, payload: &NotificationPayload) {
        let _ = self.notify(payload).await;
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self {
            notifiers: Vec::new(),
            enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_event() {
        assert_eq!(NotificationEvent::Completed.as_str(), "completed");
        assert_eq!(NotificationEvent::Error.emoji(), "❌");
        assert_eq!(NotificationEvent::Started.color(), "#3498db");
    }

    #[test]
    fn test_notification_payload() {
        let payload = NotificationPayload::new(NotificationEvent::Completed, "task-123")
            .with_prompt("Build a REST API")
            .with_model("sonnet")
            .with_iterations(5)
            .with_cost(1.25)
            .with_duration(60000);

        assert_eq!(payload.event, NotificationEvent::Completed);
        assert_eq!(payload.task_id, "task-123");
        assert_eq!(payload.iterations, 5);
        assert_eq!(payload.cost_string(), "$1.2500");
        assert_eq!(payload.duration_string(), "1m 0s");
    }

    #[test]
    fn test_notification_payload_long_prompt() {
        let long_prompt = "a".repeat(200);
        let payload = NotificationPayload::new(NotificationEvent::Completed, "task-123")
            .with_prompt(&long_prompt);

        assert_eq!(payload.prompt.len(), 100); // 97 + "..."
        assert!(payload.prompt.ends_with("..."));
    }

    #[test]
    fn test_duration_string() {
        let payload = NotificationPayload::new(NotificationEvent::Completed, "task-123")
            .with_duration(30000);
        assert_eq!(payload.duration_string(), "30s");

        let payload = payload.with_duration(90000);
        assert_eq!(payload.duration_string(), "1m 30s");

        let payload = payload.with_duration(3700000);
        assert_eq!(payload.duration_string(), "1h 1m");
    }

    #[test]
    fn test_notifications_config() {
        let config = NotificationsConfig::new()
            .with_slack(SlackConfig {
                webhook_url: "https://hooks.slack.com/test".to_string(),
                channel: Some("#general".to_string()),
                username: None,
                icon_emoji: None,
                events: vec![NotificationEvent::Completed],
            });

        assert!(config.enabled);
        assert!(config.has_any_notifiers());
        assert!(config.slack.is_some());
    }

    #[test]
    fn test_notification_manager_from_url() {
        // Slack URL
        let manager = NotificationManager::from_url("https://hooks.slack.com/services/xxx").unwrap();
        assert!(manager.is_enabled());

        // Discord URL
        let manager = NotificationManager::from_url("https://discord.com/api/webhooks/xxx").unwrap();
        assert!(manager.is_enabled());

        // Generic webhook
        let manager = NotificationManager::from_url("https://example.com/webhook").unwrap();
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_default_events() {
        let events = default_events();
        assert!(events.contains(&NotificationEvent::Completed));
        assert!(events.contains(&NotificationEvent::Error));
        assert!(!events.contains(&NotificationEvent::Started));
    }

    #[test]
    fn test_slack_message_builder() {
        let config = SlackConfig {
            webhook_url: "https://hooks.slack.com/test".to_string(),
            channel: Some("#general".to_string()),
            username: Some("Doodoori Bot".to_string()),
            icon_emoji: Some(":robot:".to_string()),
            events: default_events(),
        };

        let notifier = SlackNotifier::new(config);
        let payload = NotificationPayload::new(NotificationEvent::Completed, "task-123")
            .with_prompt("Build API")
            .with_model("sonnet")
            .with_iterations(5)
            .with_cost(1.25)
            .with_duration(60000);

        let message = notifier.build_message(&payload);

        assert!(message["attachments"].is_array());
        assert_eq!(message["channel"], "#general");
        assert_eq!(message["username"], "Doodoori Bot");
    }

    #[test]
    fn test_discord_message_builder() {
        let config = DiscordConfig {
            webhook_url: "https://discord.com/api/webhooks/test".to_string(),
            username: Some("Doodoori".to_string()),
            avatar_url: None,
            events: default_events(),
        };

        let notifier = DiscordNotifier::new(config);
        let payload = NotificationPayload::new(NotificationEvent::Error, "task-456")
            .with_prompt("Failing task")
            .with_error("Something went wrong");

        let message = notifier.build_message(&payload);

        assert!(message["embeds"].is_array());
        assert_eq!(message["username"], "Doodoori");
        assert_eq!(message["embeds"][0]["color"], NotificationEvent::Error.discord_color());
    }
}
