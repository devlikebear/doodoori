use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::ModelAlias;

/// Configuration for Claude Code execution
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
    #[serde(default)]
    pub message: Option<String>,
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
pub struct ClaudeRunner {
    config: ClaudeConfig,
}

impl ClaudeRunner {
    pub fn new(config: ClaudeConfig) -> Self {
        Self { config }
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

    /// Build the command arguments for Claude Code
    fn build_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--print".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--model".to_string(),
            self.config.model.to_model_id().to_string(),
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

        // The prompt must be the last argument
        args.push("--prompt".to_string());
        args.push(prompt.to_string());

        args
    }

    /// Execute Claude Code with the given prompt, streaming events
    pub async fn execute(
        &self,
        prompt: &str,
    ) -> Result<(mpsc::Receiver<ClaudeEvent>, tokio::task::JoinHandle<Result<ExecutionUsage>>)>
    {
        let args = self.build_args(prompt);
        tracing::debug!("Executing claude with args: {:?}", args);

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

        let handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut usage = ExecutionUsage::default();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                match serde_json::from_str::<ClaudeEvent>(&line) {
                    Ok(event) => {
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
    pub async fn execute_and_wait(&self, prompt: &str) -> Result<(Vec<ClaudeEvent>, ExecutionUsage)>
    {
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

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--prompt".to_string()));
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
                assert_eq!(asst.message, Some("Hello, world!".to_string()));
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
        let json = r#"{"type":"tool_use","tool_name":"Read","tool_input":{"file_path":"/test.txt"}}"#;
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
}
