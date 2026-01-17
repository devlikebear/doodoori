use anyhow::Result;
use clap::Args;

use crate::claude::ModelAlias;
use crate::executor::{
    ParallelConfig, ParallelEvent, ParallelExecutor, TaskDefinition, TaskStatus,
};
use crate::pricing::format_cost;

/// Run multiple tasks in parallel
#[derive(Args, Debug)]
pub struct ParallelArgs {
    /// Individual tasks (format: "description" or "description:model")
    #[arg(short, long)]
    pub task: Vec<String>,

    /// Path to spec file with Tasks section
    #[arg(long)]
    pub spec: Option<String>,

    /// Glob pattern for multiple spec files
    #[arg(long)]
    pub specs: Option<String>,

    /// Number of parallel workers
    #[arg(short, long, default_value = "3")]
    pub workers: usize,

    /// Override model for all tasks
    #[arg(short, long)]
    pub model: Option<ModelAlias>,

    /// Maximum total budget in USD
    #[arg(short, long)]
    pub budget: Option<f64>,

    /// Run in sandbox mode (Docker)
    #[arg(long)]
    pub sandbox: bool,

    /// Enable task isolation (separate workspace per task)
    #[arg(long)]
    pub isolate: bool,

    /// Stop all tasks on first failure
    #[arg(long)]
    pub fail_fast: bool,

    /// Maximum iterations per task
    #[arg(long, default_value = "50")]
    pub max_iterations: u32,

    /// YOLO mode (skip all permissions)
    #[arg(long)]
    pub yolo: bool,

    /// Dry run - show execution plan without running
    #[arg(long)]
    pub dry_run: bool,
}

impl ParallelArgs {
    pub async fn execute(self) -> Result<()> {
        if self.dry_run {
            return self.execute_dry_run().await;
        }

        // Collect tasks from CLI arguments
        let mut tasks = Vec::new();

        for task_str in &self.task {
            let (desc, model) = Self::parse_task(task_str);
            let model = self.model.clone().unwrap_or(model);

            let task = TaskDefinition::new(desc)
                .with_model(model)
                .with_max_iterations(self.max_iterations)
                .with_yolo_mode(self.yolo);

            tasks.push(task);
        }

        // TODO: Add tasks from spec file if provided
        if let Some(ref spec_path) = self.spec {
            tracing::info!("Loading tasks from spec file: {}", spec_path);
            // TODO: Parse spec and extract tasks
        }

        // TODO: Add tasks from glob pattern if provided
        if let Some(ref pattern) = self.specs {
            tracing::info!("Loading tasks from pattern: {}", pattern);
            // TODO: Glob and load multiple specs
        }

        if tasks.is_empty() {
            println!("No tasks specified. Use --task or --spec to add tasks.");
            return Ok(());
        }

        println!(
            "🔨 Doodoori is forging {} tasks in parallel with {} workers...\n",
            tasks.len(),
            self.workers
        );

        // Create executor config
        let config = ParallelConfig {
            workers: self.workers,
            total_budget: self.budget,
            fail_fast: self.fail_fast,
            sandbox: self.sandbox,
            base_working_dir: std::env::current_dir().ok(),
            task_isolation: self.isolate,
        };

        let executor = ParallelExecutor::new(config);

        // Execute and monitor progress
        let (mut event_rx, handle) = executor.execute(tasks).await?;

        // Handle events
        while let Some(event) = event_rx.recv().await {
            match event {
                ParallelEvent::Started { total_tasks } => {
                    println!("Starting {} tasks...\n", total_tasks);
                }
                ParallelEvent::TaskStarted {
                    task_id,
                    prompt_summary,
                } => {
                    let short_id = &task_id[..8.min(task_id.len())];
                    println!("  [{}] Started: {}", short_id, prompt_summary);
                }
                ParallelEvent::TaskProgress {
                    task_id,
                    iteration,
                    total,
                } => {
                    let short_id = &task_id[..8.min(task_id.len())];
                    println!("  [{}] Progress: {}/{}", short_id, iteration, total);
                }
                ParallelEvent::TaskCompleted {
                    task_id,
                    status,
                    cost,
                } => {
                    let short_id = &task_id[..8.min(task_id.len())];
                    let status_str = match status {
                        TaskStatus::Completed => "✓ Completed",
                        TaskStatus::MaxIterationsReached => "⚠ Max iterations",
                        TaskStatus::BudgetExceeded => "💰 Budget exceeded",
                        TaskStatus::Failed => "✗ Failed",
                        TaskStatus::Cancelled => "⊘ Cancelled",
                    };
                    println!("  [{}] {} ({})", short_id, status_str, format_cost(cost));
                }
                ParallelEvent::Finished {
                    succeeded,
                    failed,
                    total_cost,
                } => {
                    println!("\n=== Parallel Execution Complete ===");
                    println!("  Succeeded: {}", succeeded);
                    println!("  Failed:    {}", failed);
                    println!("  Total cost: {}", format_cost(total_cost));
                }
            }
        }

        // Wait for completion and get final result
        let result = handle.await??;

        // Print detailed results
        println!("\n=== Task Details ===\n");
        for task_result in &result.tasks {
            let short_id = &task_result.task_id[..8.min(task_result.task_id.len())];
            let status_icon = match task_result.status {
                TaskStatus::Completed => "✓",
                TaskStatus::MaxIterationsReached => "⚠",
                TaskStatus::BudgetExceeded => "💰",
                TaskStatus::Failed => "✗",
                TaskStatus::Cancelled => "⊘",
            };

            println!("{} [{}] {}", status_icon, short_id, task_result.prompt_summary);
            println!(
                "   Iterations: {} | Cost: {} | Duration: {}ms",
                task_result.iterations,
                format_cost(task_result.total_cost),
                task_result.duration_ms
            );

            if let Some(ref error) = task_result.error {
                println!("   Error: {}", error);
            }
            println!();
        }

        // Print summary
        println!("=== Summary ===");
        println!(
            "Total: {} tasks ({} succeeded, {} failed)",
            result.tasks.len(),
            result.succeeded,
            result.failed
        );
        println!("Total cost: {}", format_cost(result.total_cost));
        println!("Total time: {}ms", result.total_duration_ms);

        Ok(())
    }

    async fn execute_dry_run(&self) -> Result<()> {
        println!("=== Parallel Execution Plan ===\n");

        println!("[Workers]");
        println!("  Count: {}", self.workers);

        if let Some(budget) = self.budget {
            println!("\n[Budget]");
            println!("  Total limit: ${:.2}", budget);
        }

        println!("\n[Options]");
        println!("  Fail fast: {}", self.fail_fast);
        println!("  Task isolation: {}", self.isolate);
        println!("  YOLO mode: {}", self.yolo);
        println!("  Max iterations per task: {}", self.max_iterations);

        println!("\n[Tasks]");

        if !self.task.is_empty() {
            for (i, task) in self.task.iter().enumerate() {
                let (desc, model) = Self::parse_task(task);
                let model = self.model.clone().unwrap_or(model);
                println!("  {}. \"{}\" ({:?})", i + 1, desc, model);
            }
        }

        if let Some(spec) = &self.spec {
            println!("  From spec: {}", spec);
            println!("  (Tasks will be extracted from ## Tasks section)");
        }

        if let Some(specs) = &self.specs {
            println!("  From pattern: {}", specs);
        }

        println!("\n[Execution Mode]");
        if self.sandbox {
            println!("  Sandbox (Docker) - isolated containers per task");
        } else if self.isolate {
            println!("  Isolated - separate workspace per task in .doodoori/workspaces/");
        } else {
            println!("  Direct (local) - shared workspace");
        }

        println!("\n=== End Plan ===");

        Ok(())
    }

    /// Parse task string in format "description" or "description:model"
    fn parse_task(task: &str) -> (String, ModelAlias) {
        if let Some((desc, model)) = task.rsplit_once(':') {
            let model = match model.to_lowercase().as_str() {
                "haiku" => ModelAlias::Haiku,
                "sonnet" => ModelAlias::Sonnet,
                "opus" => ModelAlias::Opus,
                _ => ModelAlias::Sonnet,
            };
            (desc.to_string(), model)
        } else {
            (task.to_string(), ModelAlias::Sonnet)
        }
    }
}
