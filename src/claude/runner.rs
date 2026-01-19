use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ModelAlias;

/// Configuration for Claude Code execution
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// Model to use
    pub model: ModelAlias,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Working directory for execution
    pub working_dir: Option<PathBuf>,
    /// Allowed tools (e.g., "Read,Write,Edit")
    pub allowed_tools: Option<String>,
    /// Skip all permission prompts (YOLO mode)
    pub yolo_mode: bool,
    /// Custom system prompt file
    pub system_prompt: Option<PathBuf>,
    /// Read-only mode
    pub readonly: bool,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            model: ModelAlias::Sonnet,
            max_tokens: None,
            working_dir: None,
            allowed_tools: None,
            yolo_mode: false,
            system_prompt: None,
            readonly: false,
        }
    }
}

/// Events emitted during Claude execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeEvent {
    /// System initialization
    System(SystemEvent),
    /// Assistant message (text content)
    Assistant(AssistantEvent),
    /// Tool use event
    ToolUse(ToolUseEvent),
    /// Tool result event
    ToolResult(ToolResultEvent),
    /// Final result with usage stats
    Result(ResultEvent),
    /// Unknown event type
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub subtype: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantEvent {
    /// Message can be either a string (legacy) or an object (new format)
    #[serde(default)]
    pub message: Option<AssistantMessage>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Assistant message - can be a simple string or complex object
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssistantMessage {
    /// Simple text message
    Text(String),
    /// Complex message object from Claude API
    Object(AssistantMessageObject),
}

impl AssistantMessage {
    /// Extract text content from the message
    pub fn as_text(&self) -> String {
        match self {
            AssistantMessage::Text(s) => s.clone(),
            AssistantMessage::Object(obj) => {
                // Extract text from content blocks
                if let Some(content) = &obj.content {
                    content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            ContentBlock::ToolUse { name, .. } => Some(name.as_str()),
                            ContentBlock::Unknown => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                } else {
                    String::new()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageObject {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<Vec<ContentBlock>>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<MessageUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseEvent {
    pub tool_name: String,
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEvent {
    pub tool_name: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEvent {
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub duration_api_ms: Option<u64>,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub usage: Option<UsageStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

/// Accumulated usage from a Claude execution
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ExecutionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
}

impl ExecutionUsage {
    pub fn add(&mut self, other: &UsageStats) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_creation_tokens += other.cache_creation_input_tokens;
        self.cache_read_tokens += other.cache_read_input_tokens;
    }
}

/// Claude Code CLI runner
#[allow(dead_code)]
pub struct ClaudeRunner {
    config: ClaudeConfig,
    /// Optional task ID for logging
    task_id: Option<String>,
    /// Optional log directory override (for testing)
    log_dir: Option<PathBuf>,
}

#[allow(dead_code)]
impl ClaudeRunner {
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            task_id: None,
            log_dir: None,
        }
    }

    /// Set task ID for logging
    pub fn with_task_id(mut self, task_id: String) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Set custom log directory (mainly for testing)
    pub fn with_log_dir(mut self, log_dir: PathBuf) -> Self {
        self.log_dir = Some(log_dir);
        self
    }

    pub fn with_model(mut self, model: ModelAlias) -> Self {
        self.config.model = model;
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.config.working_dir = Some(dir);
        self
    }

    pub fn with_allowed_tools(mut self, tools: String) -> Self {
        self.config.allowed_tools = Some(tools);
        self
    }

    pub fn with_yolo_mode(mut self, enabled: bool) -> Self {
        self.config.yolo_mode = enabled;
        self
    }

    pub fn with_readonly(mut self, enabled: bool) -> Self {
        self.config.readonly = enabled;
        self
    }

    pub fn with_system_prompt(mut self, path: PathBuf) -> Self {
        self.config.system_prompt = Some(path);
        self
    }

    /// Write a log line to the task log file
    fn write_to_log(&self, level: &str, message: &str) -> Result<()> {
        if let Some(ref task_id) = self.task_id {
            let log_dir = self
                .log_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from(".doodoori/logs"));
            fs::create_dir_all(&log_dir)?;
            let log_path = log_dir.join(format!("{}.log", task_id));

            let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
            let line = format!("[{}] [{}] {}", timestamp, level, message);

            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    /// Build the command arguments for Claude Code
    fn build_args(&self, prompt: &str) -> Vec<String> {
        // Build arguments for Claude Code CLI
        // Use alias (haiku, sonnet, opus) instead of full model ID
        // Claude Code CLI resolves aliases to the best available model internally
        //
        // Note: Do NOT use --print as it prevents actual tool execution
        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--model".to_string(),
            self.config.model.to_string(), // alias: haiku, sonnet, opus
        ];

        if let Some(max_tokens) = self.config.max_tokens {
            args.push("--max-tokens".to_string());
            args.push(max_tokens.to_string());
        }

        if self.config.yolo_mode {
            args.push("--dangerously-skip-permissions".to_string());
        }

        if let Some(ref tools) = self.config.allowed_tools {
            args.push("--allowedTools".to_string());
            args.push(tools.clone());
        }

        if self.config.readonly {
            args.push("--allowedTools".to_string());
            args.push("Read,Grep,Glob".to_string());
        }

        if let Some(ref system_prompt) = self.config.system_prompt {
            args.push("--system-prompt".to_string());
            args.push(system_prompt.to_string_lossy().to_string());
        }

        // The prompt must be the last argument (use -p, not --prompt)
        args.push("-p".to_string());
        args.push(prompt.to_string());

        args
    }

    /// Execute Claude Code with the given prompt, streaming events
    pub async fn execute(
        &self,
        prompt: &str,
    ) -> Result<(
        mpsc::Receiver<ClaudeEvent>,
        tokio::task::JoinHandle<Result<ExecutionUsage>>,
    )> {
        let args = self.build_args(prompt);
        tracing::debug!("Executing claude with args: {:?}", args);

        // Log start
        self.write_to_log("INFO", "Starting task...")?;

        let mut cmd = Command::new("claude");
        cmd.args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().context("Failed to spawn claude command")?;

        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let (tx, rx) = mpsc::channel(100);

        // Clone task_id and log_dir for the async task
        let task_id = self.task_id.clone();
        let log_dir = self.log_dir.clone();

        let handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut usage = ExecutionUsage::default();

            // Helper to write logs from async context
            let write_log = |level: &str, message: &str| -> Result<()> {
                if let Some(ref tid) = task_id {
                    let ld = log_dir
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(".doodoori/logs"));
                    fs::create_dir_all(&ld)?;
                    let log_path = ld.join(format!("{}.log", tid));

                    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
                    let line = format!("[{}] [{}] {}", timestamp, level, message);

                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(log_path)?;
                    writeln!(file, "{}", line)?;
                }
                Ok(())
            };

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<ClaudeEvent>(&line) {
                    Ok(event) => {
                        // Log events
                        match &event {
                            ClaudeEvent::Assistant(asst) => {
                                if let Some(msg) = &asst.message {
                                    let text = msg.as_text();
                                    let _ = write_log(
                                        "CLAUDE",
                                        &text.chars().take(200).collect::<String>(),
                                    );
                                }
                            }
                            ClaudeEvent::ToolUse(tool) => {
                                let _ = write_log("TOOL", &tool.tool_name);
                            }
                            ClaudeEvent::ToolResult(result) => {
                                if result.is_error {
                                    let _ =
                                        write_log("ERROR", &format!("{} failed", result.tool_name));
                                }
                            }
                            ClaudeEvent::Result(result) => {
                                if result.is_error {
                                    let _ = write_log("ERROR", "Task failed");
                                } else {
                                    let _ = write_log("INFO", "Task completed");
                                }
                            }
                            _ => {}
                        }

                        // Extract usage information from result events
                        if let ClaudeEvent::Result(ref result) = event {
                            if let Some(ref stats) = result.usage {
                                usage.add(stats);
                            }
                            if let Some(cost) = result.total_cost_usd {
                                usage.total_cost_usd = cost;
                            }
                            if let Some(duration) = result.duration_ms {
                                usage.duration_ms = duration;
                            }
                        }

                        if tx.send(event).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse Claude event: {} - line: {}", e, line);
                    }
                }
            }

            // Wait for the process to complete
            let status = child.wait().await.context("Failed to wait for claude")?;
            if !status.success() {
                tracing::warn!("Claude exited with status: {}", status);
            }

            Ok(usage)
        });

        Ok((rx, handle))
    }

    /// Execute Claude Code and collect all output (blocking until completion)
    pub async fn execute_and_wait(
        &self,
        prompt: &str,
    ) -> Result<(Vec<ClaudeEvent>, ExecutionUsage)> {
        let (mut rx, handle) = self.execute(prompt).await?;
        let mut events = Vec::new();

        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        let usage = handle.await.context("Task panicked")??;
        Ok((events, usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClaudeConfig::default();
        assert_eq!(config.model, ModelAlias::Sonnet);
        assert!(!config.yolo_mode);
        assert!(!config.readonly);
        assert!(config.allowed_tools.is_none());
    }

    #[test]
    fn test_build_args_basic() {
        let runner = ClaudeRunner::new(ClaudeConfig::default());
        let args = runner.build_args("Test prompt");

        // --print should NOT be present (it prevents actual tool execution)
        assert!(!args.contains(&"--print".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Test prompt".to_string()));
    }

    #[test]
    fn test_build_args_yolo() {
        let config = ClaudeConfig {
            yolo_mode: true,
            ..Default::default()
        };
        let runner = ClaudeRunner::new(config);
        let args = runner.build_args("Test");

        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn test_build_args_allowed_tools() {
        let config = ClaudeConfig {
            allowed_tools: Some("Read,Write,Edit".to_string()),
            ..Default::default()
        };
        let runner = ClaudeRunner::new(config);
        let args = runner.build_args("Test");

        assert!(args.contains(&"--allowedTools".to_string()));
        assert!(args.contains(&"Read,Write,Edit".to_string()));
    }

    #[test]
    fn test_build_args_model() {
        let config = ClaudeConfig {
            model: ModelAlias::Opus,
            ..Default::default()
        };
        let runner = ClaudeRunner::new(config);
        let args = runner.build_args("Test");

        assert!(args.contains(&"--model".to_string()));
        // Should contain the actual model ID
        assert!(args.iter().any(|a| a.contains("opus")));
    }

    #[test]
    fn test_build_args_readonly() {
        let config = ClaudeConfig {
            readonly: true,
            ..Default::default()
        };
        let runner = ClaudeRunner::new(config);
        let args = runner.build_args("Test");

        assert!(args.contains(&"--allowedTools".to_string()));
        assert!(args.contains(&"Read,Grep,Glob".to_string()));
    }

    #[test]
    fn test_builder_pattern() {
        let runner = ClaudeRunner::new(ClaudeConfig::default())
            .with_model(ModelAlias::Haiku)
            .with_yolo_mode(true)
            .with_readonly(false);

        assert_eq!(runner.config.model, ModelAlias::Haiku);
        assert!(runner.config.yolo_mode);
        assert!(!runner.config.readonly);
    }

    #[test]
    fn test_parse_system_event() {
        let json = r#"{"type":"system","subtype":"init","session_id":"abc123"}"#;
        let event: ClaudeEvent = serde_json::from_str(json).unwrap();

        match event {
            ClaudeEvent::System(sys) => {
                assert_eq!(sys.subtype, "init");
                assert_eq!(sys.session_id, Some("abc123".to_string()));
            }
            _ => panic!("Expected System event"),
        }
    }

    #[test]
    fn test_parse_assistant_event() {
        let json = r#"{"type":"assistant","message":"Hello, world!"}"#;
        let event: ClaudeEvent = serde_json::from_str(json).unwrap();

        match event {
            ClaudeEvent::Assistant(asst) => {
                assert!(asst.message.is_some());
                assert_eq!(asst.message.unwrap().as_text(), "Hello, world!");
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_assistant_event_with_object() {
        let json = r#"{
            "type": "assistant",
            "message": {
                "model": "claude-sonnet-4-5-20250929",
                "id": "msg_123",
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello from Claude"},
                    {"type": "tool_use", "id": "tool_1", "name": "Bash", "input": {"command": "ls"}}
                ]
            }
        }"#;
        let event: ClaudeEvent = serde_json::from_str(json).unwrap();

        match event {
            ClaudeEvent::Assistant(asst) => {
                assert!(asst.message.is_some());
                let text = asst.message.unwrap().as_text();
                assert!(text.contains("Hello from Claude"));
                assert!(text.contains("Bash"));
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_result_event_with_usage() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "duration_ms": 5000,
            "total_cost_usd": 0.05,
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 500,
                "cache_creation_input_tokens": 0,
                "cache_read_input_tokens": 200
            }
        }"#;
        let event: ClaudeEvent = serde_json::from_str(json).unwrap();

        match event {
            ClaudeEvent::Result(result) => {
                assert!(!result.is_error);
                assert_eq!(result.duration_ms, Some(5000));
                assert_eq!(result.total_cost_usd, Some(0.05));
                let usage = result.usage.unwrap();
                assert_eq!(usage.input_tokens, 1000);
                assert_eq!(usage.output_tokens, 500);
                assert_eq!(usage.cache_read_input_tokens, 200);
            }
            _ => panic!("Expected Result event"),
        }
    }

    #[test]
    fn test_parse_tool_use_event() {
        let json =
            r#"{"type":"tool_use","tool_name":"Read","tool_input":{"file_path":"/test.txt"}}"#;
        let event: ClaudeEvent = serde_json::from_str(json).unwrap();

        match event {
            ClaudeEvent::ToolUse(tool) => {
                assert_eq!(tool.tool_name, "Read");
                assert!(tool.tool_input.is_some());
            }
            _ => panic!("Expected ToolUse event"),
        }
    }

    #[test]
    fn test_execution_usage_add() {
        let mut usage = ExecutionUsage::default();
        let stats = UsageStats {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 10,
            cache_read_input_tokens: 20,
        };

        usage.add(&stats);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.cache_creation_tokens, 10);
        assert_eq!(usage.cache_read_tokens, 20);

        // Add again
        usage.add(&stats);
        assert_eq!(usage.input_tokens, 200);
        assert_eq!(usage.output_tokens, 100);
    }

    #[test]
    fn test_log_file_creation() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let log_dir = dir.path().to_path_buf();

        let runner = ClaudeRunner::new(ClaudeConfig::default())
            .with_task_id("test-task-123".to_string())
            .with_log_dir(log_dir.clone());

        // Write a log entry
        runner.write_to_log("INFO", "Test message").unwrap();

        // Verify log file exists
        let log_path = log_dir.join("test-task-123.log");
        assert!(log_path.exists());

        // Verify content
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("[INFO]"));
        assert!(content.contains("Test message"));
    }

    #[test]
    fn test_log_file_append() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let log_dir = dir.path().to_path_buf();

        let runner = ClaudeRunner::new(ClaudeConfig::default())
            .with_task_id("test-task-456".to_string())
            .with_log_dir(log_dir.clone());

        // Write multiple log entries
        runner.write_to_log("INFO", "First message").unwrap();
        runner.write_to_log("ERROR", "Second message").unwrap();
        runner.write_to_log("CLAUDE", "Third message").unwrap();

        // Verify all entries are in the file
        let log_path = log_dir.join("test-task-456.log");
        let content = std::fs::read_to_string(&log_path).unwrap();

        assert!(content.contains("[INFO] First message"));
        assert!(content.contains("[ERROR] Second message"));
        assert!(content.contains("[CLAUDE] Third message"));

        // Verify lines are separate
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_no_log_without_task_id() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let log_dir = dir.path().to_path_buf();

        let runner = ClaudeRunner::new(ClaudeConfig::default()).with_log_dir(log_dir.clone());

        // Should not fail even without task_id
        runner.write_to_log("INFO", "Test message").unwrap();

        // No log file should be created
        let entries: Vec<_> = std::fs::read_dir(&log_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_with_task_id_builder() {
        let runner = ClaudeRunner::new(ClaudeConfig::default()).with_task_id("test-id".to_string());

        assert_eq!(runner.task_id, Some("test-id".to_string()));
    }

    #[test]
    fn test_with_log_dir_builder() {
        let log_dir = PathBuf::from("/tmp/test-logs");
        let runner = ClaudeRunner::new(ClaudeConfig::default()).with_log_dir(log_dir.clone());

        assert_eq!(runner.log_dir, Some(log_dir));
    }
}
