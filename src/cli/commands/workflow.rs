use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::executor::{ParallelConfig, ParallelExecutor, TaskDefinition};
use crate::output::{OutputFormat, OutputWriter, StepOutput, WorkflowOutput};
use crate::pricing::format_cost;
use crate::workflow::{
    DagScheduler, StepStatus, WorkflowDefinition, WorkflowState, WorkflowStateManager, WorkflowStatus,
};

/// Workflow management commands
#[derive(Args, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommand,
}

#[derive(Subcommand, Debug)]
pub enum WorkflowCommand {
    /// Run a workflow from a YAML file
    Run(WorkflowRunArgs),
    /// Resume a failed or interrupted workflow
    Resume(WorkflowResumeArgs),
    /// Validate a workflow file
    Validate(WorkflowValidateArgs),
    /// Show workflow information
    Info(WorkflowInfoArgs),
}

/// Arguments for running a workflow
#[derive(Args, Debug)]
pub struct WorkflowRunArgs {
    /// Path to workflow YAML file
    pub file: PathBuf,

    /// Dry run - show execution plan without running
    #[arg(long)]
    pub dry_run: bool,

    /// Override maximum parallel workers
    #[arg(short, long)]
    pub workers: Option<usize>,

    /// Override total budget in USD
    #[arg(short, long)]
    pub budget: Option<f64>,

    /// YOLO mode (skip all permissions)
    #[arg(long)]
    pub yolo: bool,

    /// Run in sandbox mode (Docker)
    #[arg(long)]
    pub sandbox: bool,

    /// Output format (text, json, json-pretty, yaml, markdown)
    #[arg(long, short = 'f', default_value = "text")]
    pub format: String,

    /// Output file path (default: stdout)
    #[arg(long, short = 'o')]
    pub output: Option<String>,
}

/// Arguments for resuming a workflow
#[derive(Args, Debug)]
pub struct WorkflowResumeArgs {
    /// Workflow ID to resume (or prefix)
    #[arg(required_unless_present = "list")]
    pub workflow_id: Option<String>,

    /// Resume from a specific step
    #[arg(long)]
    pub from_step: Option<String>,

    /// List resumable workflows
    #[arg(long)]
    pub list: bool,

    /// YOLO mode (skip all permissions)
    #[arg(long)]
    pub yolo: bool,

    /// Run in sandbox mode (Docker)
    #[arg(long)]
    pub sandbox: bool,
}

/// Arguments for validating a workflow
#[derive(Args, Debug)]
pub struct WorkflowValidateArgs {
    /// Path to workflow YAML file
    pub file: PathBuf,
}

/// Arguments for showing workflow info
#[derive(Args, Debug)]
pub struct WorkflowInfoArgs {
    /// Path to workflow YAML file
    pub file: PathBuf,
}

impl WorkflowArgs {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            WorkflowCommand::Run(args) => args.execute().await,
            WorkflowCommand::Resume(args) => args.execute().await,
            WorkflowCommand::Validate(args) => args.execute().await,
            WorkflowCommand::Info(args) => args.execute().await,
        }
    }
}

impl WorkflowRunArgs {
    pub async fn execute(self) -> Result<()> {
        // Load and validate workflow
        let workflow = WorkflowDefinition::load_from_file(&self.file)?;
        let warnings = workflow.validate()?;

        for warning in &warnings {
            println!("Warning: {}", warning);
        }

        if self.dry_run {
            return self.execute_dry_run(&workflow).await;
        }

        // Create scheduler
        let scheduler = DagScheduler::new(workflow.clone());
        let groups = scheduler.get_execution_groups();

        // Determine workers
        let workers = self.workers.unwrap_or(workflow.global.max_parallel_workers);

        // Determine budget
        let budget = self.budget.or(workflow.global.budget_usd);

        println!("=== Workflow: {} ===\n", workflow.name);
        println!("Workers: {}", workers);
        if let Some(b) = budget {
            println!("Budget: ${:.2}", b);
        }
        println!();

        // Create workflow state manager
        let state_manager = WorkflowStateManager::new()?;

        // Create workflow state
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let workflow_file = std::fs::canonicalize(&self.file).unwrap_or_else(|_| self.file.clone());
        let mut state = WorkflowState::new(
            workflow_id.clone(),
            workflow.name.clone(),
            workflow_file,
            &workflow.steps,
        );
        state.status = WorkflowStatus::Running;

        // Save initial state
        state_manager.save(&state)?;
        println!("Workflow ID: {}", state.short_id());

        // Execute groups sequentially
        for (group_idx, group) in groups.iter().enumerate() {
            state.current_group = group_idx as u32;
            println!("=== Group {} ({} steps) ===", group_idx, group.len());

            if group.len() == 1 {
                // Single step, run directly
                let step = group[0];
                println!("  Running: {} ({})", step.name, workflow.get_step_model(step));

                state.update_step(&step.name, StepStatus::Running, 0.0, None);
                state_manager.save(&state)?;

                // Create task definition
                let task = TaskDefinition::new(
                    step.prompt.clone().unwrap_or_else(|| format!("Execute step: {}", step.name))
                )
                .with_model(workflow.get_step_model(step))
                .with_max_iterations(step.max_iterations)
                .with_yolo_mode(self.yolo);

                // Execute using parallel executor (single task)
                let executor = ParallelExecutor::with_workers(1);
                let result = executor.execute_and_wait(vec![task]).await?;

                if result.succeeded == 1 {
                    state.update_step(&step.name, StepStatus::Completed, result.total_cost, None);
                    state_manager.save(&state)?;
                    println!("  ✓ Completed: {} ({})", step.name, format_cost(result.total_cost));
                } else {
                    let error = result.tasks.first()
                        .and_then(|t| t.error.clone())
                        .unwrap_or_else(|| "Unknown error".to_string());
                    state.update_step(&step.name, StepStatus::Failed, result.total_cost, Some(error.clone()));
                    state.status = WorkflowStatus::Failed;
                    state_manager.save(&state)?;
                    println!("  ✗ Failed: {} - {}", step.name, error);
                    break;
                }
            } else {
                // Multiple steps, run in parallel
                let tasks: Vec<TaskDefinition> = group
                    .iter()
                    .map(|step| {
                        state.update_step(&step.name, StepStatus::Running, 0.0, None);
                        TaskDefinition::new(
                            step.prompt.clone().unwrap_or_else(|| format!("Execute step: {}", step.name))
                        )
                        .with_model(workflow.get_step_model(step))
                        .with_max_iterations(step.max_iterations)
                        .with_yolo_mode(self.yolo)
                    })
                    .collect();

                state_manager.save(&state)?;

                println!("  Running {} tasks in parallel...", tasks.len());
                for step in group.iter() {
                    println!("    - {} ({})", step.name, workflow.get_step_model(step));
                }

                let config = ParallelConfig {
                    workers,
                    total_budget: budget,
                    fail_fast: false,
                    sandbox: self.sandbox,
                    base_working_dir: std::env::current_dir().ok(),
                    task_isolation: true,
                    use_git_worktrees: false,
                    git_branch_prefix: "task/".to_string(),
                    git_auto_commit: false,
                    git_auto_pr: false,
                };

                let executor = ParallelExecutor::new(config);
                let result = executor.execute_and_wait(tasks).await?;

                // Update states based on results
                for (i, task_result) in result.tasks.iter().enumerate() {
                    let step_name = &group[i].name;
                    if task_result.status == crate::executor::TaskStatus::Completed {
                        state.update_step(step_name, StepStatus::Completed, task_result.total_cost, None);
                        println!("  ✓ Completed: {} ({})", step_name, format_cost(task_result.total_cost));
                    } else {
                        let error = task_result.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                        state.update_step(step_name, StepStatus::Failed, task_result.total_cost, Some(error.clone()));
                        println!("  ✗ Failed: {} - {}", step_name, error);
                    }
                }

                state_manager.save(&state)?;

                if result.failed > 0 {
                    state.status = WorkflowStatus::Failed;
                    state_manager.save(&state)?;
                    println!("\nWorkflow failed: {} steps failed", result.failed);
                    break;
                }
            }

            println!();
        }

        // Final status
        if state.status != WorkflowStatus::Failed {
            state.status = WorkflowStatus::Completed;
        }
        state_manager.save(&state)?;

        // Parse output format
        let output_format: OutputFormat = self.format.parse().unwrap_or_default();

        if output_format == OutputFormat::Text {
            println!("=== Workflow {} ===", if state.status == WorkflowStatus::Completed { "Completed" } else { "Failed" });
            println!("Total cost: {}", format_cost(state.total_cost_usd));
            println!("Workflow ID: {}", state.short_id());

            if state.status == WorkflowStatus::Failed {
                println!("\nTo resume: doodoori workflow resume {}", state.short_id());
            }
        } else {
            // Build structured output
            let mut output = WorkflowOutput::new(workflow.name.clone(), workflow_id.clone())
                .with_status(format!("{:?}", state.status));

            for (name, step_state) in &state.steps {
                let mut step_output = StepOutput::new(name.clone())
                    .with_status(format!("{:?}", step_state.status).to_lowercase())
                    .with_cost(step_state.cost_usd);

                if let Some(ref error) = step_state.error {
                    step_output = step_output.with_error(error);
                }

                output.add_step(step_output);
            }

            // Write output
            let writer = if let Some(ref path) = self.output {
                OutputWriter::new(output_format).with_file(path)
            } else {
                OutputWriter::new(output_format)
            };

            if let Err(e) = writer.write_workflow(&output) {
                tracing::error!("Failed to write output: {}", e);
            }
        }

        Ok(())
    }

    async fn execute_dry_run(&self, workflow: &WorkflowDefinition) -> Result<()> {
        let scheduler = DagScheduler::new(workflow.clone());
        let groups = scheduler.get_execution_groups();

        println!("=== Workflow: {} ===\n", workflow.name);

        println!("[Global Settings]");
        println!("  Default model: {}", workflow.global.default_model);
        println!("  Max parallel workers: {}", self.workers.unwrap_or(workflow.global.max_parallel_workers));
        if let Some(budget) = self.budget.or(workflow.global.budget_usd) {
            println!("  Total budget: ${:.2}", budget);
        }
        println!("  Completion promise: {}", workflow.global.completion_promise);

        println!("\n[Execution Plan]");

        let mut total_budget = 0.0;

        for (group_idx, group) in groups.iter().enumerate() {
            let mode = if group.len() == 1 { "Sequential" } else { "Parallel" };
            println!("\n[Group {}] {} ({} steps)", group_idx, mode, group.len());

            for step in group.iter() {
                let model = workflow.get_step_model(step);
                let budget_str = step.budget_usd
                    .map(|b| {
                        total_budget += b;
                        format!("${:.2}", b)
                    })
                    .unwrap_or_else(|| "unlimited".to_string());

                let deps = if step.depends_on.is_empty() {
                    String::new()
                } else {
                    format!(" (after: {})", step.depends_on.join(", "))
                };

                println!("  {} │ {:?} │ {} │ max_iter: {}{}",
                    step.name,
                    model,
                    budget_str,
                    step.max_iterations,
                    deps
                );

                if let Some(ref prompt) = step.prompt {
                    let short_prompt = if prompt.len() > 50 {
                        format!("{}...", &prompt[..47])
                    } else {
                        prompt.clone()
                    };
                    println!("    └─ \"{}\"", short_prompt);
                }
                if let Some(ref spec) = step.spec {
                    println!("    └─ spec: {}", spec);
                }
            }
        }

        println!("\n[Summary]");
        println!("  Total steps: {}", workflow.steps.len());
        println!("  Execution groups: {}", groups.len());
        if total_budget > 0.0 {
            println!("  Total step budgets: ${:.2}", total_budget);
        }

        // Topological order
        println!("\n[Execution Order (topological)]");
        let order = scheduler.topological_order()?;
        for (i, step) in order.iter().enumerate() {
            println!("  {}. {}", i + 1, step);
        }

        println!("\n=== End Dry Run ===");

        Ok(())
    }
}

impl WorkflowResumeArgs {
    pub async fn execute(self) -> Result<()> {
        let state_manager = WorkflowStateManager::new()?;

        // List resumable workflows
        if self.list {
            return self.list_resumable(&state_manager).await;
        }

        let workflow_id = self.workflow_id.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Workflow ID is required"))?;

        // Find workflow state by ID or prefix
        let mut state = state_manager.load(workflow_id)?
            .or_else(|| state_manager.find_by_prefix(workflow_id).ok().flatten())
            .ok_or_else(|| anyhow::anyhow!("Workflow not found: {}", workflow_id))?;

        if !state.can_resume() {
            anyhow::bail!(
                "Workflow {} cannot be resumed (status: {:?})",
                state.short_id(),
                state.status
            );
        }

        // Load workflow definition from saved file
        if !state.workflow_file.exists() {
            anyhow::bail!(
                "Workflow definition file not found: {}",
                state.workflow_file.display()
            );
        }
        let workflow = WorkflowDefinition::load_from_file(&state.workflow_file)?;

        println!("=== Resuming Workflow: {} ===", workflow.name);
        println!("Workflow ID: {}", state.short_id());
        println!("Total cost so far: {}", format_cost(state.total_cost_usd));
        println!();

        // Get completed steps
        let completed_steps: std::collections::HashSet<String> = state.get_completed_steps().into_iter().collect();

        // Show step status
        println!("[Step Status]");
        for step in &workflow.steps {
            let step_state = state.steps.get(&step.name);
            let status_icon = match step_state.map(|s| &s.status) {
                Some(StepStatus::Completed) => "✓",
                Some(StepStatus::Failed) => "✗",
                Some(StepStatus::Running) => "⟳",
                Some(StepStatus::Skipped) => "○",
                _ => "·",
            };
            println!("  {} {}", status_icon, step.name);
        }
        println!();

        // Create scheduler and mark completed steps
        let mut scheduler = DagScheduler::new(workflow.clone());
        for step_name in &completed_steps {
            scheduler.mark_completed(step_name);
        }

        // Get remaining groups to execute
        let groups = scheduler.get_execution_groups();
        let resumable_groups: Vec<_> = groups.iter()
            .filter(|group| {
                group.iter().any(|step| !completed_steps.contains(&step.name))
            })
            .collect();

        if resumable_groups.is_empty() {
            println!("All steps are already completed!");
            state.status = WorkflowStatus::Completed;
            state_manager.save(&state)?;
            return Ok(());
        }

        println!("Resuming from {} remaining groups...", resumable_groups.len());
        println!();

        // Update state to running
        state.status = WorkflowStatus::Running;
        state_manager.save(&state)?;

        // Determine workers and budget
        let workers = workflow.global.max_parallel_workers;

        // Execute remaining groups
        for group in resumable_groups {
            // Filter to only non-completed steps
            let pending_steps: Vec<_> = group.iter()
                .filter(|step| !completed_steps.contains(&step.name))
                .collect();

            if pending_steps.is_empty() {
                continue;
            }

            state.current_group += 1;
            println!("=== Group {} ({} steps) ===", state.current_group, pending_steps.len());

            if pending_steps.len() == 1 {
                let step = pending_steps[0];
                println!("  Running: {} ({})", step.name, workflow.get_step_model(step));

                state.update_step(&step.name, StepStatus::Running, 0.0, None);
                state_manager.save(&state)?;

                let task = TaskDefinition::new(
                    step.prompt.clone().unwrap_or_else(|| format!("Execute step: {}", step.name))
                )
                .with_model(workflow.get_step_model(step))
                .with_max_iterations(step.max_iterations)
                .with_yolo_mode(self.yolo);

                let executor = ParallelExecutor::with_workers(1);
                let result = executor.execute_and_wait(vec![task]).await?;

                if result.succeeded == 1 {
                    state.update_step(&step.name, StepStatus::Completed, result.total_cost, None);
                    state_manager.save(&state)?;
                    println!("  ✓ Completed: {} ({})", step.name, format_cost(result.total_cost));
                } else {
                    let error = result.tasks.first()
                        .and_then(|t| t.error.clone())
                        .unwrap_or_else(|| "Unknown error".to_string());
                    state.update_step(&step.name, StepStatus::Failed, result.total_cost, Some(error.clone()));
                    state.status = WorkflowStatus::Failed;
                    state_manager.save(&state)?;
                    println!("  ✗ Failed: {} - {}", step.name, error);
                    break;
                }
            } else {
                let tasks: Vec<TaskDefinition> = pending_steps.iter()
                    .map(|step| {
                        state.update_step(&step.name, StepStatus::Running, 0.0, None);
                        TaskDefinition::new(
                            step.prompt.clone().unwrap_or_else(|| format!("Execute step: {}", step.name))
                        )
                        .with_model(workflow.get_step_model(step))
                        .with_max_iterations(step.max_iterations)
                        .with_yolo_mode(self.yolo)
                    })
                    .collect();

                state_manager.save(&state)?;

                println!("  Running {} tasks in parallel...", tasks.len());
                for step in pending_steps.iter() {
                    println!("    - {} ({})", step.name, workflow.get_step_model(step));
                }

                let config = ParallelConfig {
                    workers,
                    total_budget: None,
                    fail_fast: false,
                    sandbox: self.sandbox,
                    base_working_dir: std::env::current_dir().ok(),
                    task_isolation: true,
                    use_git_worktrees: false,
                    git_branch_prefix: "task/".to_string(),
                    git_auto_commit: false,
                    git_auto_pr: false,
                };

                let executor = ParallelExecutor::new(config);
                let result = executor.execute_and_wait(tasks).await?;

                for (i, task_result) in result.tasks.iter().enumerate() {
                    let step_name = &pending_steps[i].name;
                    if task_result.status == crate::executor::TaskStatus::Completed {
                        state.update_step(step_name, StepStatus::Completed, task_result.total_cost, None);
                        println!("  ✓ Completed: {} ({})", step_name, format_cost(task_result.total_cost));
                    } else {
                        let error = task_result.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                        state.update_step(step_name, StepStatus::Failed, task_result.total_cost, Some(error.clone()));
                        println!("  ✗ Failed: {} - {}", step_name, error);
                    }
                }

                state_manager.save(&state)?;

                if result.failed > 0 {
                    state.status = WorkflowStatus::Failed;
                    state_manager.save(&state)?;
                    println!("\nWorkflow failed: {} steps failed", result.failed);
                    break;
                }
            }

            println!();
        }

        // Final status
        if state.status != WorkflowStatus::Failed {
            state.status = WorkflowStatus::Completed;
        }
        state_manager.save(&state)?;

        println!("=== Workflow {} ===", if state.status == WorkflowStatus::Completed { "Completed" } else { "Failed" });
        println!("Total cost: {}", format_cost(state.total_cost_usd));

        if state.status == WorkflowStatus::Failed {
            println!("\nTo resume again: doodoori workflow resume {}", state.short_id());
        }

        Ok(())
    }

    async fn list_resumable(&self, state_manager: &WorkflowStateManager) -> Result<()> {
        let states = state_manager.list_resumable()?;

        if states.is_empty() {
            println!("No resumable workflows found.");
            println!("\nWorkflows that failed or were cancelled can be resumed.");
            return Ok(());
        }

        println!("=== Resumable Workflows ===\n");
        println!("  {:<10} {:<20} {:<12} {:<10} {}",
            "ID", "NAME", "STATUS", "COST", "UPDATED"
        );
        println!("  {}", "-".repeat(70));

        for state in &states {
            let status = format!("{:?}", state.status);
            let updated = state.updated_at.format("%Y-%m-%d %H:%M");
            println!("  {:<10} {:<20} {:<12} {:<10} {}",
                state.short_id(),
                if state.name.len() > 18 { format!("{}...", &state.name[..15]) } else { state.name.clone() },
                status,
                format_cost(state.total_cost_usd),
                updated
            );
        }

        println!("\nTo resume: doodoori workflow resume <workflow-id>");

        Ok(())
    }
}

impl WorkflowValidateArgs {
    pub async fn execute(self) -> Result<()> {
        println!("Validating workflow: {}", self.file.display());

        let workflow = WorkflowDefinition::load_from_file(&self.file)?;
        let warnings = workflow.validate()?;

        println!("\n✓ Workflow '{}' is valid", workflow.name);

        if !warnings.is_empty() {
            println!("\nWarnings:");
            for warning in warnings {
                println!("  - {}", warning);
            }
        }

        println!("\nSteps: {}", workflow.steps.len());
        for step in &workflow.steps {
            let deps = if step.depends_on.is_empty() {
                String::new()
            } else {
                format!(" -> [{}]", step.depends_on.join(", "))
            };
            println!("  - {}{}", step.name, deps);
        }

        Ok(())
    }
}

impl WorkflowInfoArgs {
    pub async fn execute(self) -> Result<()> {
        let workflow = WorkflowDefinition::load_from_file(&self.file)?;

        println!("=== Workflow: {} ===\n", workflow.name);

        println!("[Global Settings]");
        println!("  Default model: {}", workflow.global.default_model);
        println!("  Max parallel workers: {}", workflow.global.max_parallel_workers);
        if let Some(budget) = workflow.global.budget_usd {
            println!("  Budget: ${:.2}", budget);
        }
        println!("  Completion promise: {}", workflow.global.completion_promise);

        println!("\n[Steps]");
        for step in &workflow.steps {
            println!("\n  {}:", step.name);
            if let Some(ref model) = step.model {
                println!("    Model: {}", model);
            }
            println!("    Parallel group: {}", step.parallel_group);
            if !step.depends_on.is_empty() {
                println!("    Depends on: {}", step.depends_on.join(", "));
            }
            println!("    Max iterations: {}", step.max_iterations);
            if let Some(budget) = step.budget_usd {
                println!("    Budget: ${:.2}", budget);
            }
            if let Some(ref prompt) = step.prompt {
                let short = if prompt.len() > 60 { format!("{}...", &prompt[..57]) } else { prompt.clone() };
                println!("    Prompt: \"{}\"", short);
            }
            if let Some(ref spec) = step.spec {
                println!("    Spec: {}", spec);
            }
        }

        Ok(())
    }
}
