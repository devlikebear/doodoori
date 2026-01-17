#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::claude::{ClaudeConfig, ClaudeEvent, ClaudeRunner, ExecutionUsage, ModelAlias};
use crate::hooks::{HookContext, HookExecutor, HookType, HooksConfig};
use crate::notifications::{NotificationEvent, NotificationManager, NotificationPayload, NotificationsConfig};
use crate::pricing::CostHistoryManager;
use crate::state::{StateManager, TaskState};

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
    /// Enable state persistence
    pub enable_state: bool,
    /// Enable cost tracking
    pub enable_cost_tracking: bool,
    /// Project directory for state/cost files
    pub project_dir: Option<PathBuf>,
    /// Hooks configuration
    pub hooks: HooksConfig,
    /// Disable hooks execution
    pub disable_hooks: bool,
    /// Notifications configuration
    pub notifications: NotificationsConfig,
    /// Disable notifications
    pub disable_notifications: bool,
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
            enable_state: true,
            enable_cost_tracking: true,
            project_dir: None,
            hooks: HooksConfig::default(),
            disable_hooks: false,
            notifications: NotificationsConfig::default(),
            disable_notifications: false,
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
    /// Hook executed
    HookExecuted {
        hook_type: HookType,
        success: bool,
        duration_ms: u64,
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

/// Persistence managers for state and cost tracking
struct PersistenceManagers {
    state_manager: Option<StateManager>,
    cost_manager: Option<CostHistoryManager>,
}

/// The Loop Engine - implements the self-improvement loop until completion
pub struct LoopEngine {
    config: LoopConfig,
    persistence: Option<PersistenceManagers>,
}

impl LoopEngine {
    pub fn new(config: LoopConfig) -> Self {
        // Initialize persistence managers if enabled
        let persistence = Self::init_persistence(&config);
        Self { config, persistence }
    }

    fn init_persistence(config: &LoopConfig) -> Option<PersistenceManagers> {
        if !config.enable_state && !config.enable_cost_tracking {
            return None;
        }

        let project_dir = config.project_dir.clone()
            .or_else(|| config.working_dir.clone())
            .or_else(|| std::env::current_dir().ok())?;

        let state_manager = if config.enable_state {
            StateManager::new(&project_dir).ok()
        } else {
            None
        };

        let cost_manager = if config.enable_cost_tracking {
            CostHistoryManager::for_project(&project_dir).ok()
        } else {
            None
        };

        Some(PersistenceManagers {
            state_manager,
            cost_manager,
        })
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

    /// Create hook context for the current execution state
    fn create_hook_context(
        &self,
        task_id: &str,
        prompt: &str,
        iteration: Option<u32>,
        total_iterations: Option<u32>,
        cost: f64,
        status: &str,
        error: Option<&str>,
    ) -> HookContext {
        let mut ctx = HookContext::new()
            .with_task_id(task_id)
            .with_prompt(prompt)
            .with_model(self.config.model.to_string())
            .with_cost(cost)
            .with_status(status);

        if let Some(iter) = iteration {
            ctx = ctx.with_iteration(iter);
        }
        if let Some(total) = total_iterations {
            ctx = ctx.with_total_iterations(total);
        }
        if let Some(err) = error {
            ctx = ctx.with_error(err);
        }
        if let Some(ref dir) = self.config.working_dir {
            ctx = ctx.with_working_dir(dir.clone());
        }

        ctx
    }

    /// Execute a hook and send event
    async fn execute_hook(
        &self,
        hook_executor: &HookExecutor,
        hook_type: HookType,
        context: &HookContext,
        tx: &mpsc::Sender<LoopEvent>,
    ) -> Result<bool> {
        if self.config.disable_hooks {
            return Ok(true);
        }

        let result = hook_executor.execute_with_policy(hook_type, context).await?;

        // Send hook event
        let _ = tx
            .send(LoopEvent::HookExecuted {
                hook_type,
                success: result.success,
                duration_ms: result.duration_ms,
            })
            .await;

        Ok(result.success)
    }

    /// Run the loop internally
    async fn run_loop(&self, initial_prompt: &str, tx: mpsc::Sender<LoopEvent>) -> Result<LoopResult> {
        let mut total_usage = ExecutionUsage::default();
        let mut iteration = 0u32;
        let mut previous_output: Option<String> = None;
        let mut final_output: Option<String> = None;
        let mut status = LoopStatus::Running;

        // Initialize hook executor
        let working_dir = self.config.working_dir.clone()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let hook_executor = HookExecutor::new(self.config.hooks.clone(), &working_dir);

        // Initialize notification manager
        let notification_manager = if !self.config.disable_notifications && self.config.notifications.enabled {
            Some(NotificationManager::new(self.config.notifications.clone()))
        } else {
            None
        };

        // Track start time for duration calculation
        let start_time = std::time::Instant::now();

        // Initialize task state if state management is enabled
        let mut task_state = if self.config.enable_state {
            let state = TaskState::new(
                initial_prompt.to_string(),
                self.config.model.to_string(),
                self.config.max_iterations,
            );
            Some(state)
        } else {
            None
        };

        // Get task ID for cost tracking
        let task_id = task_state.as_ref()
            .map(|s| s.task_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Mark task as started and save initial state
        if let (Some(state), Some(persistence)) = (&mut task_state, &self.persistence) {
            state.start();
            if let Some(state_manager) = &persistence.state_manager {
                let _ = state_manager.save_state(state);
            }
        }

        // Send "Started" notification
        if let Some(ref manager) = notification_manager {
            let payload = NotificationPayload {
                event: NotificationEvent::Started,
                task_id: task_id.clone(),
                prompt: initial_prompt.to_string(),
                model: self.config.model.to_string(),
                iterations: 0,
                cost_usd: 0.0,
                duration_ms: 0,
                error: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                metadata: std::collections::HashMap::new(),
            };
            manager.notify_silent(&payload).await;
        }

        // Execute pre_run hook
        let pre_run_context = self.create_hook_context(
            &task_id,
            initial_prompt,
            None,
            Some(self.config.max_iterations),
            0.0,
            "starting",
            None,
        );
        if let Err(e) = self.execute_hook(&hook_executor, HookType::PreRun, &pre_run_context, &tx).await {
            tracing::warn!("Pre-run hook failed: {}", e);
            status = LoopStatus::Error(format!("Pre-run hook failed: {}", e));

            // Execute on_error hook for pre_run failure
            let error_context = self.create_hook_context(
                &task_id,
                initial_prompt,
                None,
                Some(self.config.max_iterations),
                0.0,
                "error",
                Some(&e.to_string()),
            );
            let _ = self.execute_hook(&hook_executor, HookType::OnError, &error_context, &tx).await;

            // Return early with error result
            let _ = tx
                .send(LoopEvent::LoopFinished {
                    status: status.clone(),
                    total_iterations: 0,
                    total_usage: total_usage.clone(),
                })
                .await;

            return Ok(LoopResult {
                status,
                iterations: 0,
                total_usage,
                final_output: None,
            });
        }

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

                    // Update task state and save
                    if let (Some(state), Some(persistence)) = (&mut task_state, &self.persistence) {
                        state.update_iteration(iteration);
                        state.update_usage(&total_usage);
                        if let Some(state_manager) = &persistence.state_manager {
                            let _ = state_manager.save_state(state);
                        }
                    }

                    previous_output = Some(output_buffer.clone());
                    final_output = Some(output_buffer);

                    // Execute on_iteration hook
                    let iter_context = self.create_hook_context(
                        &task_id,
                        initial_prompt,
                        Some(iteration),
                        Some(self.config.max_iterations),
                        total_usage.total_cost_usd,
                        if completed { "completed" } else { "running" },
                        None,
                    );
                    let _ = self.execute_hook(&hook_executor, HookType::OnIteration, &iter_context, &tx).await;

                    if completed {
                        status = LoopStatus::Completed;
                        break;
                    }
                }
                Err(e) => {
                    // Execute on_error hook
                    let error_context = self.create_hook_context(
                        &task_id,
                        initial_prompt,
                        Some(iteration),
                        Some(self.config.max_iterations),
                        total_usage.total_cost_usd,
                        "error",
                        Some(&e.to_string()),
                    );
                    let _ = self.execute_hook(&hook_executor, HookType::OnError, &error_context, &tx).await;

                    // Mark task as failed
                    if let (Some(state), Some(persistence)) = (&mut task_state, &self.persistence) {
                        state.fail(e.to_string());
                        if let Some(state_manager) = &persistence.state_manager {
                            let _ = state_manager.save_state(state);
                        }
                    }
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

        // Finalize task state based on final status
        if let Some(ref mut state) = task_state {
            match &status {
                LoopStatus::Completed => {
                    state.complete(final_output.clone());
                }
                LoopStatus::MaxIterationsReached | LoopStatus::BudgetExceeded | LoopStatus::Stopped => {
                    state.interrupt();
                }
                LoopStatus::Error(err) => {
                    state.fail(err.clone());
                }
                LoopStatus::Running => {
                    // Should not happen at this point
                    state.interrupt();
                }
            }

            // Save final state
            if let Some(persistence) = &self.persistence {
                if let Some(state_manager) = &persistence.state_manager {
                    let _ = state_manager.save_state(state);

                    // Archive completed or failed tasks
                    if matches!(status, LoopStatus::Completed | LoopStatus::Error(_)) {
                        let _ = state_manager.archive_task(state);
                    }
                }
            }
        }

        // Record total cost for the task
        // Note: We re-initialize cost manager here to get a mutable reference
        if self.config.enable_cost_tracking {
            let project_dir = self.config.project_dir.clone()
                .or_else(|| self.config.working_dir.clone())
                .or_else(|| std::env::current_dir().ok());

            if let Some(ref project_dir) = project_dir {
                if let Ok(mut cost_manager) = CostHistoryManager::for_project(project_dir) {
                    let status_str = format!("{:?}", status);
                    let prompt_summary = if initial_prompt.len() > 50 {
                        format!("{}...", &initial_prompt[..47])
                    } else {
                        initial_prompt.to_string()
                    };
                    let _ = cost_manager.record_cost(
                        &task_id,
                        &self.config.model.to_string(),
                        total_usage.input_tokens,
                        total_usage.output_tokens,
                        total_usage.cache_read_tokens,
                        total_usage.cache_creation_tokens,
                        total_usage.total_cost_usd,
                        &status_str,
                        Some(prompt_summary),
                    );
                }
            }
        }

        // Execute final hooks based on status
        let final_status_str = match &status {
            LoopStatus::Completed => "completed",
            LoopStatus::MaxIterationsReached => "max_iterations",
            LoopStatus::BudgetExceeded => "budget_exceeded",
            LoopStatus::Error(_) => "error",
            LoopStatus::Stopped => "stopped",
            LoopStatus::Running => "running",
        };

        // Execute on_complete hook if task completed successfully
        if status == LoopStatus::Completed {
            let complete_context = self.create_hook_context(
                &task_id,
                initial_prompt,
                Some(iteration),
                Some(self.config.max_iterations),
                total_usage.total_cost_usd,
                final_status_str,
                None,
            );
            let _ = self.execute_hook(&hook_executor, HookType::OnComplete, &complete_context, &tx).await;
        }

        // Execute post_run hook (always runs at the end)
        let post_run_context = self.create_hook_context(
            &task_id,
            initial_prompt,
            Some(iteration),
            Some(self.config.max_iterations),
            total_usage.total_cost_usd,
            final_status_str,
            match &status {
                LoopStatus::Error(e) => Some(e.as_str()),
                _ => None,
            },
        );
        let _ = self.execute_hook(&hook_executor, HookType::PostRun, &post_run_context, &tx).await;

        // Send final notification based on status
        if let Some(ref manager) = notification_manager {
            let notification_event = match &status {
                LoopStatus::Completed => NotificationEvent::Completed,
                LoopStatus::MaxIterationsReached => NotificationEvent::MaxIterations,
                LoopStatus::BudgetExceeded => NotificationEvent::BudgetExceeded,
                LoopStatus::Error(_) => NotificationEvent::Error,
                _ => NotificationEvent::Completed, // Stopped/Running treated as completed
            };

            let payload = NotificationPayload {
                event: notification_event,
                task_id: task_id.clone(),
                prompt: initial_prompt.to_string(),
                model: self.config.model.to_string(),
                iterations: iteration + 1,
                cost_usd: total_usage.total_cost_usd,
                duration_ms: start_time.elapsed().as_millis() as u64,
                error: match &status {
                    LoopStatus::Error(e) => Some(e.clone()),
                    _ => None,
                },
                timestamp: chrono::Utc::now().to_rfc3339(),
                metadata: std::collections::HashMap::new(),
            };

            manager.notify_silent(&payload).await;
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
