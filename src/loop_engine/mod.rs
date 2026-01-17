#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::claude::{ClaudeConfig, ClaudeEvent, ClaudeRunner, ExecutionUsage, ModelAlias};

/// Completion detection strategies
#[derive(Debug, Clone)]
pub enum CompletionStrategy {
    /// Look for a specific promise string (e.g., "<promise>COMPLETE</promise>")
    Promise(String),
    /// Look for any of the given strings
    AnyOf(Vec<String>),
    /// Custom regex pattern
    Regex(String),
}

impl Default for CompletionStrategy {
    fn default() -> Self {
        CompletionStrategy::Promise("<promise>COMPLETE</promise>".to_string())
    }
}

/// Configuration for the Loop Engine
#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// Maximum number of iterations
    pub max_iterations: u32,
    /// Budget limit in USD (None = unlimited)
    pub budget_limit: Option<f64>,
    /// How to detect completion
    pub completion_strategy: CompletionStrategy,
    /// Model to use
    pub model: ModelAlias,
    /// Working directory
    pub working_dir: Option<PathBuf>,
    /// YOLO mode (skip permissions)
    pub yolo_mode: bool,
    /// Read-only mode
    pub readonly: bool,
    /// Custom system prompt file (e.g., doodoori.md)
    pub system_prompt: Option<PathBuf>,
    /// Allowed tools
    pub allowed_tools: Option<String>,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            budget_limit: None,
            completion_strategy: CompletionStrategy::default(),
            model: ModelAlias::Sonnet,
            working_dir: None,
            yolo_mode: false,
            readonly: false,
            system_prompt: None,
            allowed_tools: None,
        }
    }
}

/// Status of a loop execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopStatus {
    /// Still running
    Running,
    /// Completed successfully (task finished)
    Completed,
    /// Max iterations reached without completion
    MaxIterationsReached,
    /// Budget exceeded
    BudgetExceeded,
    /// Error during execution
    Error(String),
    /// Stopped by user
    Stopped,
}

/// Events emitted by the Loop Engine
#[derive(Debug, Clone)]
pub enum LoopEvent {
    /// New iteration started
    IterationStarted { iteration: u32 },
    /// Claude event from current iteration
    ClaudeEvent(ClaudeEvent),
    /// Iteration completed
    IterationCompleted {
        iteration: u32,
        usage: ExecutionUsage,
        completed: bool,
    },
    /// Loop finished
    LoopFinished {
        status: LoopStatus,
        total_iterations: u32,
        total_usage: ExecutionUsage,
    },
}

/// Result of a completed loop execution
#[derive(Debug)]
pub struct LoopResult {
    pub status: LoopStatus,
    pub iterations: u32,
    pub total_usage: ExecutionUsage,
    pub final_output: Option<String>,
}

/// The Loop Engine - implements the self-improvement loop until completion
pub struct LoopEngine {
    config: LoopConfig,
}

impl LoopEngine {
    pub fn new(config: LoopConfig) -> Self {
        Self { config }
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.config.max_iterations = max;
        self
    }

    pub fn with_budget(mut self, budget: f64) -> Self {
        self.config.budget_limit = Some(budget);
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

    pub fn with_yolo_mode(mut self, enabled: bool) -> Self {
        self.config.yolo_mode = enabled;
        self
    }

    pub fn with_completion_strategy(mut self, strategy: CompletionStrategy) -> Self {
        self.config.completion_strategy = strategy;
        self
    }

    /// Check if the output indicates task completion
    fn is_complete(&self, output: &str) -> bool {
        match &self.config.completion_strategy {
            CompletionStrategy::Promise(promise) => output.contains(promise),
            CompletionStrategy::AnyOf(patterns) => patterns.iter().any(|p| output.contains(p)),
            CompletionStrategy::Regex(pattern) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(output))
                    .unwrap_or(false)
            }
        }
    }

    /// Build the prompt for a specific iteration
    fn build_prompt(&self, initial_prompt: &str, iteration: u32, previous_output: Option<&str>) -> String {
        if iteration == 0 {
            // First iteration: include completion instructions
            format!(
                "{}\n\n---\n\nWhen you have completed the task, output the completion marker: <promise>COMPLETE</promise>\n\nIf you cannot complete the task, explain why and still output: <promise>COMPLETE</promise>",
                initial_prompt
            )
        } else {
            // Subsequent iterations: continue from where we left off
            if let Some(prev) = previous_output {
                format!(
                    "Continue from the previous attempt. The task is not yet complete.\n\nOriginal task: {}\n\nPrevious output summary:\n{}\n\nPlease continue working on the task. When complete, output: <promise>COMPLETE</promise>",
                    initial_prompt,
                    Self::truncate_output(prev, 2000)
                )
            } else {
                format!(
                    "Continue working on the task. Original task: {}\n\nWhen complete, output: <promise>COMPLETE</promise>",
                    initial_prompt
                )
            }
        }
    }

    /// Truncate output to a maximum length
    fn truncate_output(output: &str, max_len: usize) -> &str {
        if output.len() <= max_len {
            output
        } else {
            &output[output.len() - max_len..]
        }
    }

    /// Execute the loop, returning events through a channel
    pub async fn execute(
        &self,
        prompt: &str,
    ) -> Result<(mpsc::Receiver<LoopEvent>, tokio::task::JoinHandle<Result<LoopResult>>)> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();
        let prompt = prompt.to_string();

        let handle = tokio::spawn(async move {
            let engine = LoopEngine::new(config);
            engine.run_loop(&prompt, tx).await
        });

        Ok((rx, handle))
    }

    /// Run the loop internally
    async fn run_loop(&self, initial_prompt: &str, tx: mpsc::Sender<LoopEvent>) -> Result<LoopResult> {
        let mut total_usage = ExecutionUsage::default();
        let mut iteration = 0u32;
        let mut previous_output: Option<String> = None;
        let mut final_output: Option<String> = None;
        let mut status = LoopStatus::Running;

        while iteration < self.config.max_iterations {
            // Check budget before starting
            if let Some(limit) = self.config.budget_limit {
                if total_usage.total_cost_usd >= limit {
                    status = LoopStatus::BudgetExceeded;
                    break;
                }
            }

            // Send iteration started event
            let _ = tx.send(LoopEvent::IterationStarted { iteration }).await;

            // Build the prompt for this iteration
            let prompt = self.build_prompt(initial_prompt, iteration, previous_output.as_deref());

            // Create Claude runner
            let claude_config = ClaudeConfig {
                model: self.config.model.clone(),
                working_dir: self.config.working_dir.clone(),
                allowed_tools: self.config.allowed_tools.clone(),
                yolo_mode: self.config.yolo_mode,
                readonly: self.config.readonly,
                system_prompt: self.config.system_prompt.clone(),
                ..Default::default()
            };

            let runner = ClaudeRunner::new(claude_config);

            // Execute Claude
            match runner.execute(&prompt).await {
                Ok((mut event_rx, usage_handle)) => {
                    let mut output_buffer = String::new();

                    // Forward events and collect output
                    while let Some(event) = event_rx.recv().await {
                        // Extract text from assistant events
                        if let ClaudeEvent::Assistant(ref asst) = event {
                            if let Some(ref msg) = asst.message {
                                output_buffer.push_str(msg);
                            }
                        }

                        // Forward the event
                        let _ = tx.send(LoopEvent::ClaudeEvent(event)).await;
                    }

                    // Get usage stats
                    let iter_usage = usage_handle.await.context("Task panicked")??;

                    // Check for completion
                    let completed = self.is_complete(&output_buffer);

                    // Send iteration completed event
                    let _ = tx
                        .send(LoopEvent::IterationCompleted {
                            iteration,
                            usage: iter_usage.clone(),
                            completed,
                        })
                        .await;

                    // Update totals
                    total_usage.input_tokens += iter_usage.input_tokens;
                    total_usage.output_tokens += iter_usage.output_tokens;
                    total_usage.cache_creation_tokens += iter_usage.cache_creation_tokens;
                    total_usage.cache_read_tokens += iter_usage.cache_read_tokens;
                    total_usage.total_cost_usd += iter_usage.total_cost_usd;
                    total_usage.duration_ms += iter_usage.duration_ms;

                    previous_output = Some(output_buffer.clone());
                    final_output = Some(output_buffer);

                    if completed {
                        status = LoopStatus::Completed;
                        break;
                    }
                }
                Err(e) => {
                    status = LoopStatus::Error(e.to_string());
                    break;
                }
            }

            iteration += 1;
        }

        // Check if we hit max iterations
        if status == LoopStatus::Running {
            status = LoopStatus::MaxIterationsReached;
        }

        // Send loop finished event
        let _ = tx
            .send(LoopEvent::LoopFinished {
                status: status.clone(),
                total_iterations: iteration + 1,
                total_usage: total_usage.clone(),
            })
            .await;

        Ok(LoopResult {
            status,
            iterations: iteration + 1,
            total_usage,
            final_output,
        })
    }

    /// Execute the loop and wait for completion (blocking)
    pub async fn execute_and_wait(&self, prompt: &str) -> Result<LoopResult> {
        let (mut rx, handle) = self.execute(prompt).await?;

        // Drain the channel (we don't need the events here)
        while rx.recv().await.is_some() {}

        handle.await.context("Task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LoopConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert!(config.budget_limit.is_none());
        assert!(!config.yolo_mode);
        assert!(!config.readonly);
    }

    #[test]
    fn test_completion_strategy_promise() {
        let engine = LoopEngine::new(LoopConfig::default());
        assert!(engine.is_complete("Task done! <promise>COMPLETE</promise>"));
        assert!(!engine.is_complete("Task still in progress"));
    }

    #[test]
    fn test_completion_strategy_any_of() {
        let config = LoopConfig {
            completion_strategy: CompletionStrategy::AnyOf(vec![
                "DONE".to_string(),
                "FINISHED".to_string(),
                "COMPLETE".to_string(),
            ]),
            ..Default::default()
        };
        let engine = LoopEngine::new(config);

        assert!(engine.is_complete("Task DONE"));
        assert!(engine.is_complete("Task FINISHED"));
        assert!(engine.is_complete("Task COMPLETE"));
        assert!(!engine.is_complete("Task in progress"));
    }

    #[test]
    fn test_completion_strategy_regex() {
        let config = LoopConfig {
            completion_strategy: CompletionStrategy::Regex(r"(?i)task\s+completed?".to_string()),
            ..Default::default()
        };
        let engine = LoopEngine::new(config);

        assert!(engine.is_complete("TASK COMPLETE"));
        assert!(engine.is_complete("task completed"));
        assert!(!engine.is_complete("still working"));
    }

    #[test]
    fn test_build_prompt_first_iteration() {
        let engine = LoopEngine::new(LoopConfig::default());
        let prompt = engine.build_prompt("Write hello world", 0, None);

        assert!(prompt.contains("Write hello world"));
        assert!(prompt.contains("<promise>COMPLETE</promise>"));
    }

    #[test]
    fn test_build_prompt_subsequent_iteration() {
        let engine = LoopEngine::new(LoopConfig::default());
        let prompt = engine.build_prompt("Write hello world", 1, Some("Started writing..."));

        assert!(prompt.contains("Continue"));
        assert!(prompt.contains("Write hello world"));
        assert!(prompt.contains("Started writing..."));
    }

    #[test]
    fn test_truncate_output() {
        let short = "hello";
        assert_eq!(LoopEngine::truncate_output(short, 10), "hello");

        let long = "hello world this is a long string";
        let truncated = LoopEngine::truncate_output(long, 10);
        assert_eq!(truncated.len(), 10);
    }

    #[test]
    fn test_builder_pattern() {
        let engine = LoopEngine::new(LoopConfig::default())
            .with_max_iterations(100)
            .with_budget(10.0)
            .with_model(ModelAlias::Opus)
            .with_yolo_mode(true);

        assert_eq!(engine.config.max_iterations, 100);
        assert_eq!(engine.config.budget_limit, Some(10.0));
        assert_eq!(engine.config.model, ModelAlias::Opus);
        assert!(engine.config.yolo_mode);
    }

    #[test]
    fn test_loop_status_equality() {
        assert_eq!(LoopStatus::Running, LoopStatus::Running);
        assert_eq!(LoopStatus::Completed, LoopStatus::Completed);
        assert_ne!(LoopStatus::Running, LoopStatus::Completed);
    }
}
