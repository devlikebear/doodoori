//! Parallel execution module for running multiple tasks concurrently.
//!
//! This module provides the infrastructure for executing multiple doodoori tasks
//! in parallel using a worker pool pattern.

#![allow(dead_code)]

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};
use uuid::Uuid;

use crate::claude::ModelAlias;
use crate::loop_engine::{LoopConfig, LoopEngine, LoopStatus};

/// Definition of a task to be executed in parallel
#[derive(Debug, Clone)]
pub struct TaskDefinition {
    /// Unique identifier for this task
    pub task_id: String,
    /// The prompt/description for the task
    pub prompt: String,
    /// Task name (for branch naming)
    pub name: Option<String>,
    /// Model to use for this task
    pub model: ModelAlias,
    /// Maximum iterations for this task
    pub max_iterations: u32,
    /// Budget limit for this task (USD)
    pub budget_limit: Option<f64>,
    /// Working directory for this task
    pub working_dir: Option<PathBuf>,
    /// YOLO mode (skip permissions)
    pub yolo_mode: bool,
    /// Git branch associated with this task (set when using worktrees)
    pub git_branch: Option<String>,
}

impl TaskDefinition {
    /// Create a new task definition with defaults
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            prompt: prompt.into(),
            name: None,
            model: ModelAlias::Sonnet,
            max_iterations: 50,
            budget_limit: None,
            working_dir: None,
            yolo_mode: false,
            git_branch: None,
        }
    }

    /// Set the task name (used for branch naming)
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the model for this task
    pub fn with_model(mut self, model: ModelAlias) -> Self {
        self.model = model;
        self
    }

    /// Set the maximum iterations
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    /// Set the budget limit
    pub fn with_budget(mut self, budget: f64) -> Self {
        self.budget_limit = Some(budget);
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set YOLO mode
    pub fn with_yolo_mode(mut self, enabled: bool) -> Self {
        self.yolo_mode = enabled;
        self
    }

    /// Set the git branch name for this task (used with git worktrees)
    pub fn with_git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }
}

/// Result of a single task execution
#[derive(Debug)]
pub struct TaskResult {
    /// Task ID
    pub task_id: String,
    /// Task prompt (truncated for display)
    pub prompt_summary: String,
    /// Final status
    pub status: TaskStatus,
    /// Number of iterations executed
    pub iterations: u32,
    /// Total cost in USD
    pub total_cost: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Final output (if available)
    pub output: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Status of a task
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task completed successfully
    Completed,
    /// Task reached max iterations
    MaxIterationsReached,
    /// Task exceeded budget
    BudgetExceeded,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
}

impl From<LoopStatus> for TaskStatus {
    fn from(status: LoopStatus) -> Self {
        match status {
            LoopStatus::Completed => TaskStatus::Completed,
            LoopStatus::MaxIterationsReached => TaskStatus::MaxIterationsReached,
            LoopStatus::BudgetExceeded => TaskStatus::BudgetExceeded,
            LoopStatus::Error(_) => TaskStatus::Failed,
            LoopStatus::Stopped => TaskStatus::Cancelled,
            LoopStatus::Running => TaskStatus::Failed, // Should not happen
        }
    }
}

/// Aggregated result of all parallel tasks
#[derive(Debug)]
pub struct ParallelResult {
    /// Results for each task
    pub tasks: Vec<TaskResult>,
    /// Total cost across all tasks
    pub total_cost: f64,
    /// Total duration (wall clock time)
    pub total_duration_ms: u64,
    /// Number of successful tasks
    pub succeeded: usize,
    /// Number of failed tasks
    pub failed: usize,
}

impl ParallelResult {
    /// Create a new parallel result from task results
    pub fn from_tasks(tasks: Vec<TaskResult>, total_duration_ms: u64) -> Self {
        let total_cost = tasks.iter().map(|t| t.total_cost).sum();
        let succeeded = tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
        let failed = tasks.len() - succeeded;

        Self {
            tasks,
            total_cost,
            total_duration_ms,
            succeeded,
            failed,
        }
    }
}

/// Events emitted during parallel execution
#[derive(Debug, Clone)]
pub enum ParallelEvent {
    /// Execution started
    Started { total_tasks: usize },
    /// Task started
    TaskStarted { task_id: String, prompt_summary: String },
    /// Task progress update
    TaskProgress { task_id: String, iteration: u32, total: u32 },
    /// Task completed
    TaskCompleted { task_id: String, status: TaskStatus, cost: f64 },
    /// All tasks finished
    Finished { succeeded: usize, failed: usize, total_cost: f64 },
}

/// Configuration for parallel execution
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Number of concurrent workers
    pub workers: usize,
    /// Total budget limit across all tasks (USD)
    pub total_budget: Option<f64>,
    /// Whether to stop all tasks on first failure
    pub fail_fast: bool,
    /// Whether to use sandbox mode
    pub sandbox: bool,
    /// Base working directory for task isolation
    pub base_working_dir: Option<PathBuf>,
    /// Enable task isolation (separate workspace per task)
    pub task_isolation: bool,
    /// Use git worktrees for task isolation
    pub use_git_worktrees: bool,
    /// Branch prefix for git worktrees (e.g., "feature/", "task/")
    pub git_branch_prefix: String,
    /// Auto-commit changes on task completion
    pub git_auto_commit: bool,
    /// Auto-create PR on task completion
    pub git_auto_pr: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            workers: 3,
            total_budget: None,
            fail_fast: false,
            sandbox: false,
            base_working_dir: None,
            task_isolation: false,
            use_git_worktrees: false,
            git_branch_prefix: "task/".to_string(),
            git_auto_commit: false,
            git_auto_pr: false,
        }
    }
}

/// Parallel task executor using a worker pool
pub struct ParallelExecutor {
    config: ParallelConfig,
}

impl ParallelExecutor {
    /// Create a new parallel executor with the given configuration
    pub fn new(config: ParallelConfig) -> Self {
        Self { config }
    }

    /// Create a new parallel executor with default configuration
    pub fn with_workers(workers: usize) -> Self {
        Self::new(ParallelConfig {
            workers,
            ..Default::default()
        })
    }

    /// Set total budget limit
    pub fn with_budget(mut self, budget: f64) -> Self {
        self.config.total_budget = Some(budget);
        self
    }

    /// Set fail-fast mode
    pub fn with_fail_fast(mut self, enabled: bool) -> Self {
        self.config.fail_fast = enabled;
        self
    }

    /// Enable task isolation
    pub fn with_task_isolation(mut self, enabled: bool) -> Self {
        self.config.task_isolation = enabled;
        self
    }

    /// Set base working directory
    pub fn with_base_dir(mut self, dir: PathBuf) -> Self {
        self.config.base_working_dir = Some(dir);
        self
    }

    /// Enable git worktrees for task isolation
    pub fn with_git_worktrees(mut self, enabled: bool) -> Self {
        self.config.use_git_worktrees = enabled;
        if enabled {
            self.config.task_isolation = true; // Worktrees imply isolation
        }
        self
    }

    /// Set git branch prefix for worktrees
    pub fn with_git_branch_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.git_branch_prefix = prefix.into();
        self
    }

    /// Enable auto-commit on task completion
    pub fn with_git_auto_commit(mut self, enabled: bool) -> Self {
        self.config.git_auto_commit = enabled;
        self
    }

    /// Enable auto-PR creation on task completion
    pub fn with_git_auto_pr(mut self, enabled: bool) -> Self {
        self.config.git_auto_pr = enabled;
        self
    }

    /// Execute multiple tasks in parallel
    pub async fn execute(
        &self,
        tasks: Vec<TaskDefinition>,
    ) -> Result<(mpsc::Receiver<ParallelEvent>, tokio::task::JoinHandle<Result<ParallelResult>>)> {
        let (tx, rx) = mpsc::channel(100);
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            Self::run_parallel(config, tasks, tx).await
        });

        Ok((rx, handle))
    }

    /// Execute and wait for completion
    pub async fn execute_and_wait(&self, tasks: Vec<TaskDefinition>) -> Result<ParallelResult> {
        let (mut rx, handle) = self.execute(tasks).await?;

        // Drain events (they're also sent to the channel for monitoring)
        while rx.recv().await.is_some() {}

        handle.await.context("Parallel execution panicked")?
    }

    /// Internal parallel execution logic
    async fn run_parallel(
        config: ParallelConfig,
        tasks: Vec<TaskDefinition>,
        tx: mpsc::Sender<ParallelEvent>,
    ) -> Result<ParallelResult> {
        let start_time = std::time::Instant::now();
        let total_tasks = tasks.len();

        // Send started event
        let _ = tx.send(ParallelEvent::Started { total_tasks }).await;

        // Create semaphore for worker pool
        let semaphore = Arc::new(Semaphore::new(config.workers));

        // Shared state for budget tracking
        let spent_budget = Arc::new(Mutex::new(0.0f64));

        // Flag for fail-fast cancellation
        let cancelled = Arc::new(Mutex::new(false));

        // Channel to collect results
        let (result_tx, mut result_rx) = mpsc::channel(total_tasks);

        // Spawn tasks
        for task in tasks {
            let permit = semaphore.clone().acquire_owned().await?;
            let tx = tx.clone();
            let result_tx = result_tx.clone();
            let spent = spent_budget.clone();
            let cancelled = cancelled.clone();
            let config = config.clone();

            tokio::spawn(async move {
                // Check if cancelled
                if *cancelled.lock().await {
                    let _ = result_tx.send(TaskResult {
                        task_id: task.task_id.clone(),
                        prompt_summary: Self::truncate_prompt(&task.prompt),
                        status: TaskStatus::Cancelled,
                        iterations: 0,
                        total_cost: 0.0,
                        duration_ms: 0,
                        output: None,
                        error: Some("Cancelled due to fail-fast".to_string()),
                    }).await;
                    drop(permit);
                    return;
                }

                // Check budget
                if let Some(total_budget) = config.total_budget {
                    let current_spent = *spent.lock().await;
                    if current_spent >= total_budget {
                        let _ = result_tx.send(TaskResult {
                            task_id: task.task_id.clone(),
                            prompt_summary: Self::truncate_prompt(&task.prompt),
                            status: TaskStatus::BudgetExceeded,
                            iterations: 0,
                            total_cost: 0.0,
                            duration_ms: 0,
                            output: None,
                            error: Some("Total budget exceeded".to_string()),
                        }).await;
                        drop(permit);
                        return;
                    }
                }

                // Send task started event
                let _ = tx.send(ParallelEvent::TaskStarted {
                    task_id: task.task_id.clone(),
                    prompt_summary: Self::truncate_prompt(&task.prompt),
                }).await;

                // Determine working directory
                let working_dir = if config.task_isolation {
                    let base = config.base_working_dir.clone()
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_else(|| PathBuf::from("."));
                    let task_dir = base.join(".doodoori").join("workspaces").join(&task.task_id);
                    if let Err(e) = tokio::fs::create_dir_all(&task_dir).await {
                        tracing::warn!("Failed to create task workspace: {}", e);
                        task.working_dir.clone()
                    } else {
                        Some(task_dir)
                    }
                } else {
                    task.working_dir.clone()
                };

                // Create loop engine for this task
                let loop_config = LoopConfig {
                    max_iterations: task.max_iterations,
                    budget_limit: task.budget_limit,
                    model: task.model.clone(),
                    working_dir,
                    yolo_mode: task.yolo_mode,
                    enable_state: true,
                    enable_cost_tracking: true,
                    ..Default::default()
                };

                let engine = LoopEngine::new(loop_config);
                let task_start = std::time::Instant::now();

                // Execute the task
                let result = engine.execute_and_wait(&task.prompt).await;

                let duration_ms = task_start.elapsed().as_millis() as u64;

                let task_result = match result {
                    Ok(loop_result) => {
                        // Update spent budget
                        *spent.lock().await += loop_result.total_usage.total_cost_usd;

                        let status = TaskStatus::from(loop_result.status.clone());

                        // Check fail-fast
                        if config.fail_fast && status == TaskStatus::Failed {
                            *cancelled.lock().await = true;
                        }

                        // Send completed event
                        let _ = tx.send(ParallelEvent::TaskCompleted {
                            task_id: task.task_id.clone(),
                            status: status.clone(),
                            cost: loop_result.total_usage.total_cost_usd,
                        }).await;

                        TaskResult {
                            task_id: task.task_id.clone(),
                            prompt_summary: Self::truncate_prompt(&task.prompt),
                            status,
                            iterations: loop_result.iterations,
                            total_cost: loop_result.total_usage.total_cost_usd,
                            duration_ms,
                            output: loop_result.final_output,
                            error: None,
                        }
                    }
                    Err(e) => {
                        // Check fail-fast
                        if config.fail_fast {
                            *cancelled.lock().await = true;
                        }

                        // Send completed event
                        let _ = tx.send(ParallelEvent::TaskCompleted {
                            task_id: task.task_id.clone(),
                            status: TaskStatus::Failed,
                            cost: 0.0,
                        }).await;

                        TaskResult {
                            task_id: task.task_id.clone(),
                            prompt_summary: Self::truncate_prompt(&task.prompt),
                            status: TaskStatus::Failed,
                            iterations: 0,
                            total_cost: 0.0,
                            duration_ms,
                            output: None,
                            error: Some(e.to_string()),
                        }
                    }
                };

                let _ = result_tx.send(task_result).await;
                drop(permit);
            });
        }

        // Drop the original result_tx so the receiver knows when all tasks are done
        drop(result_tx);

        // Collect all results
        let mut results = Vec::with_capacity(total_tasks);
        while let Some(result) = result_rx.recv().await {
            results.push(result);
        }

        let total_duration_ms = start_time.elapsed().as_millis() as u64;
        let parallel_result = ParallelResult::from_tasks(results, total_duration_ms);

        // Send finished event
        let _ = tx.send(ParallelEvent::Finished {
            succeeded: parallel_result.succeeded,
            failed: parallel_result.failed,
            total_cost: parallel_result.total_cost,
        }).await;

        Ok(parallel_result)
    }

    /// Truncate prompt for display
    fn truncate_prompt(prompt: &str) -> String {
        if prompt.len() > 50 {
            format!("{}...", &prompt[..47])
        } else {
            prompt.to_string()
        }
    }
}

/// Workspace manager for task isolation
pub struct WorkspaceManager {
    base_dir: PathBuf,
    workspaces: HashMap<String, WorkspaceInfo>,
}

/// Information about a task workspace
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Path to the workspace
    pub path: PathBuf,
    /// Git branch name (if using worktrees)
    pub branch: Option<String>,
    /// Whether this is a git worktree
    pub is_worktree: bool,
}

impl WorkspaceManager {
    /// Create a new workspace manager
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            workspaces: HashMap::new(),
        }
    }

    /// Create a workspace for a task
    pub async fn create_workspace(&mut self, task_id: &str) -> Result<PathBuf> {
        let workspace_dir = self.base_dir
            .join(".doodoori")
            .join("workspaces")
            .join(task_id);

        tokio::fs::create_dir_all(&workspace_dir)
            .await
            .context("Failed to create workspace directory")?;

        self.workspaces.insert(task_id.to_string(), WorkspaceInfo {
            path: workspace_dir.clone(),
            branch: None,
            is_worktree: false,
        });
        Ok(workspace_dir)
    }

    /// Create a git worktree workspace for a task
    pub fn create_worktree_workspace(
        &mut self,
        task_id: &str,
        task_name: &str,
        branch_prefix: &str,
    ) -> Result<(PathBuf, String)> {
        use crate::git::worktree::WorktreeManager as GitWorktreeManager;

        let worktree_manager = GitWorktreeManager::with_default_dir(&self.base_dir)
            .context("Failed to create worktree manager")?;

        let worktree = worktree_manager
            .create_for_task(task_id, task_name, branch_prefix)
            .context("Failed to create git worktree")?;

        self.workspaces.insert(task_id.to_string(), WorkspaceInfo {
            path: worktree.path.clone(),
            branch: Some(worktree.branch.clone()),
            is_worktree: true,
        });

        Ok((worktree.path, worktree.branch))
    }

    /// Get workspace path for a task
    pub fn get_workspace(&self, task_id: &str) -> Option<&PathBuf> {
        self.workspaces.get(task_id).map(|info| &info.path)
    }

    /// Get workspace info for a task
    pub fn get_workspace_info(&self, task_id: &str) -> Option<&WorkspaceInfo> {
        self.workspaces.get(task_id)
    }

    /// Clean up a task's workspace
    pub async fn cleanup_workspace(&mut self, task_id: &str, delete_branch: bool) -> Result<()> {
        if let Some(info) = self.workspaces.remove(task_id) {
            if info.is_worktree {
                use crate::git::worktree::WorktreeManager as GitWorktreeManager;
                if let Ok(worktree_manager) = GitWorktreeManager::with_default_dir(&self.base_dir) {
                    let _ = worktree_manager.remove_with_branch(task_id, delete_branch);
                }
            } else if info.path.exists() {
                tokio::fs::remove_dir_all(&info.path)
                    .await
                    .context("Failed to remove workspace")?;
            }
        }
        Ok(())
    }

    /// Clean up all workspaces
    pub async fn cleanup_all(&mut self) -> Result<()> {
        // Clean up worktrees first
        use crate::git::worktree::WorktreeManager as GitWorktreeManager;
        if let Ok(worktree_manager) = GitWorktreeManager::with_default_dir(&self.base_dir) {
            let _ = worktree_manager.cleanup_all();
        }

        // Clean up regular workspaces
        let workspaces_dir = self.base_dir.join(".doodoori").join("workspaces");
        if workspaces_dir.exists() {
            tokio::fs::remove_dir_all(&workspaces_dir)
                .await
                .context("Failed to remove workspaces directory")?;
        }
        self.workspaces.clear();
        Ok(())
    }

    /// List all workspaces
    pub fn list_workspaces(&self) -> Vec<(&str, &WorkspaceInfo)> {
        self.workspaces
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_definition_builder() {
        let task = TaskDefinition::new("Test task")
            .with_model(ModelAlias::Opus)
            .with_max_iterations(100)
            .with_budget(5.0)
            .with_yolo_mode(true);

        assert_eq!(task.prompt, "Test task");
        assert_eq!(task.model, ModelAlias::Opus);
        assert_eq!(task.max_iterations, 100);
        assert_eq!(task.budget_limit, Some(5.0));
        assert!(task.yolo_mode);
    }

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.workers, 3);
        assert!(config.total_budget.is_none());
        assert!(!config.fail_fast);
        assert!(!config.sandbox);
    }

    #[test]
    fn test_parallel_executor_builder() {
        let executor = ParallelExecutor::with_workers(5)
            .with_budget(100.0)
            .with_fail_fast(true)
            .with_task_isolation(true);

        assert_eq!(executor.config.workers, 5);
        assert_eq!(executor.config.total_budget, Some(100.0));
        assert!(executor.config.fail_fast);
        assert!(executor.config.task_isolation);
    }

    #[test]
    fn test_truncate_prompt() {
        let short = "Hello world";
        assert_eq!(ParallelExecutor::truncate_prompt(short), "Hello world");

        let long = "This is a very long prompt that should be truncated to fit within the display limit";
        let truncated = ParallelExecutor::truncate_prompt(long);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() <= 53); // 50 chars + "..."
    }

    #[test]
    fn test_task_status_from_loop_status() {
        assert_eq!(TaskStatus::from(LoopStatus::Completed), TaskStatus::Completed);
        assert_eq!(TaskStatus::from(LoopStatus::MaxIterationsReached), TaskStatus::MaxIterationsReached);
        assert_eq!(TaskStatus::from(LoopStatus::BudgetExceeded), TaskStatus::BudgetExceeded);
        assert_eq!(TaskStatus::from(LoopStatus::Error("test".to_string())), TaskStatus::Failed);
        assert_eq!(TaskStatus::from(LoopStatus::Stopped), TaskStatus::Cancelled);
    }

    #[test]
    fn test_parallel_result_aggregation() {
        let tasks = vec![
            TaskResult {
                task_id: "1".to_string(),
                prompt_summary: "Task 1".to_string(),
                status: TaskStatus::Completed,
                iterations: 5,
                total_cost: 1.5,
                duration_ms: 1000,
                output: None,
                error: None,
            },
            TaskResult {
                task_id: "2".to_string(),
                prompt_summary: "Task 2".to_string(),
                status: TaskStatus::Failed,
                iterations: 3,
                total_cost: 0.5,
                duration_ms: 500,
                output: None,
                error: Some("Error".to_string()),
            },
            TaskResult {
                task_id: "3".to_string(),
                prompt_summary: "Task 3".to_string(),
                status: TaskStatus::Completed,
                iterations: 10,
                total_cost: 2.0,
                duration_ms: 2000,
                output: None,
                error: None,
            },
        ];

        let result = ParallelResult::from_tasks(tasks, 5000);
        assert_eq!(result.succeeded, 2);
        assert_eq!(result.failed, 1);
        assert!((result.total_cost - 4.0).abs() < 0.001);
        assert_eq!(result.total_duration_ms, 5000);
    }

    #[tokio::test]
    async fn test_workspace_manager() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut manager = WorkspaceManager::new(temp_dir.path().to_path_buf());

        // Create workspace
        let workspace = manager.create_workspace("test-task").await.unwrap();
        assert!(workspace.exists());
        assert!(workspace.ends_with("test-task"));

        // Get workspace
        let retrieved = manager.get_workspace("test-task");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &workspace);

        // Cleanup workspace
        manager.cleanup_workspace("test-task", false).await.unwrap();
        assert!(!workspace.exists());
        assert!(manager.get_workspace("test-task").is_none());
    }

    #[test]
    fn test_parallel_executor_with_git_worktrees() {
        let executor = ParallelExecutor::with_workers(3)
            .with_git_worktrees(true)
            .with_git_branch_prefix("feature/")
            .with_git_auto_commit(true);

        assert!(executor.config.use_git_worktrees);
        assert!(executor.config.task_isolation); // Should be enabled automatically
        assert_eq!(executor.config.git_branch_prefix, "feature/");
        assert!(executor.config.git_auto_commit);
    }

    #[test]
    fn test_task_definition_with_name() {
        let task = TaskDefinition::new("Test task")
            .with_name("my-feature");

        assert_eq!(task.name, Some("my-feature".to_string()));
    }
}
