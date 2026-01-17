//! Resume command for continuing interrupted tasks

use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;

use crate::claude::ModelAlias;
use crate::loop_engine::{LoopConfig, LoopEngine, LoopEvent, LoopStatus};
use crate::state::{StateManager, TaskStatus};

/// Arguments for the resume command
#[derive(Args, Debug)]
pub struct ResumeArgs {
    /// Task ID to resume (can be short prefix)
    #[arg()]
    pub task_id: Option<String>,

    /// List all resumable tasks
    #[arg(long, short)]
    pub list: bool,

    /// Resume from specific iteration
    #[arg(long)]
    pub from_iteration: Option<u32>,

    /// Show task details without resuming
    #[arg(long)]
    pub info: bool,
}

impl ResumeArgs {
    pub async fn execute(self) -> Result<()> {
        let work_dir = std::env::current_dir()?;
        let state_manager = StateManager::new(&work_dir)?;

        if self.list {
            return self.list_resumable_tasks(&state_manager);
        }

        if let Some(task_id) = &self.task_id {
            if self.info {
                return self.show_task_info(&state_manager, task_id);
            }
            return self.resume_task(&state_manager, task_id).await;
        }

        // No task ID provided, show current state or list
        if let Some(state) = state_manager.load_state()? {
            if state.can_resume() {
                println!("Current task found: {} ({})", state.short_id(), state.status);
                println!();
                self.print_task_details(&state);
                println!();
                println!("Use 'doodoori resume {}' to continue this task", state.short_id());
            } else {
                println!("Current task is {}: {}", state.status, state.short_id());
            }
        } else {
            self.list_resumable_tasks(&state_manager)?;
        }

        Ok(())
    }

    fn list_resumable_tasks(&self, state_manager: &StateManager) -> Result<()> {
        let tasks = state_manager.list_resumable_tasks()?;

        if tasks.is_empty() {
            println!("No resumable tasks found.");
            println!();
            println!("Run a task with 'doodoori run \"your prompt\"' first.");
            return Ok(());
        }

        println!("Resumable tasks:");
        println!();
        println!("{:<10} {:<12} {:<8} {:<10} {}",
            "ID", "STATUS", "ITER", "COST", "PROMPT");
        println!("{}", "-".repeat(70));

        for task in tasks {
            let prompt_preview: String = task.prompt.chars().take(30).collect();
            let prompt_display = if task.prompt.len() > 30 {
                format!("{}...", prompt_preview)
            } else {
                prompt_preview
            };

            println!("{:<10} {:<12} {:<8} ${:<9.2} {}",
                task.short_id(),
                task.status.to_string(),
                format!("{}/{}", task.current_iteration, task.max_iterations),
                task.total_cost_usd,
                prompt_display
            );
        }

        println!();
        println!("Use 'doodoori resume <task-id>' to continue a task");
        println!("Use 'doodoori resume <task-id> --info' to see details");

        Ok(())
    }

    fn show_task_info(&self, state_manager: &StateManager, task_id: &str) -> Result<()> {
        // Try current state first
        let state = if let Some(s) = state_manager.load_state()? {
            if s.task_id.starts_with(task_id) || s.short_id() == task_id {
                Some(s)
            } else {
                None
            }
        } else {
            None
        };

        // Try history if not in current state
        let state = match state {
            Some(s) => s,
            None => state_manager
                .load_from_history(task_id)?
                .context(format!("Task not found: {}", task_id))?,
        };

        println!("Task Details");
        println!("============");
        println!();
        self.print_task_details(&state);

        Ok(())
    }

    fn print_task_details(&self, state: &crate::state::TaskState) {
        println!("Task ID:    {}", state.task_id);
        println!("Status:     {}", state.status);
        println!("Model:      {}", state.model);
        println!("Iteration:  {}/{}", state.current_iteration, state.max_iterations);
        println!("Created:    {}", state.created_at.format("%Y-%m-%d %H:%M:%S"));
        println!("Updated:    {}", state.updated_at.format("%Y-%m-%d %H:%M:%S"));
        println!();
        println!("Usage:");
        println!("  Input tokens:  {}", state.usage.input_tokens);
        println!("  Output tokens: {}", state.usage.output_tokens);
        println!("  Cache read:    {}", state.usage.cache_read_tokens);
        println!("  Cache write:   {}", state.usage.cache_creation_tokens);
        println!("  Total cost:    ${:.4}", state.total_cost_usd);
        println!("  Duration:      {}ms", state.duration_ms);
        println!();
        println!("Prompt:");
        println!("  {}", state.prompt);

        if let Some(ref session_id) = state.session_id {
            println!();
            println!("Session ID: {}", session_id);
        }

        if let Some(ref error) = state.error {
            println!();
            println!("Error: {}", error);
        }

        if let Some(ref output) = state.final_output {
            println!();
            println!("Final Output:");
            // Truncate if too long
            let preview: String = output.chars().take(500).collect();
            if output.len() > 500 {
                println!("  {}...", preview);
            } else {
                println!("  {}", preview);
            }
        }
    }

    async fn resume_task(&self, state_manager: &StateManager, task_id: &str) -> Result<()> {
        use console::{style, Emoji};
        use indicatif::{ProgressBar, ProgressStyle};

        // Find the task
        let mut state = if let Some(s) = state_manager.load_state()? {
            if s.task_id.starts_with(task_id) || s.short_id() == task_id {
                s
            } else {
                state_manager
                    .load_from_history(task_id)?
                    .context(format!("Task not found: {}", task_id))?
            }
        } else {
            state_manager
                .load_from_history(task_id)?
                .context(format!("Task not found: {}", task_id))?
        };

        if !state.can_resume() {
            anyhow::bail!(
                "Task {} cannot be resumed (status: {})",
                state.short_id(),
                state.status
            );
        }

        println!("{} Resuming task: {}", Emoji("🔄", ""), state.short_id());
        println!();
        println!("  Prompt:     {}", if state.prompt.len() > 50 {
            format!("{}...", &state.prompt[..47])
        } else {
            state.prompt.clone()
        });
        println!("  Model:      {}", state.model);
        println!("  Iteration:  {}/{}", state.current_iteration, state.max_iterations);
        println!("  Cost so far: ${:.4}", state.total_cost_usd);

        let start_iteration = if let Some(from_iter) = self.from_iteration {
            if from_iter < state.current_iteration {
                println!("  Resetting to iteration: {}", from_iter);
                from_iter
            } else {
                state.current_iteration
            }
        } else {
            state.current_iteration
        };

        println!();

        // Update status to running
        state.status = TaskStatus::Running;
        state_manager.save_state(&state)?;

        // Parse model from state
        let model: ModelAlias = state.model.parse().unwrap_or(ModelAlias::Sonnet);

        // Calculate remaining iterations
        let remaining_iterations = state.max_iterations.saturating_sub(start_iteration);
        if remaining_iterations == 0 {
            println!("{} No iterations remaining (max: {})", Emoji("⚠️", "[!]"), state.max_iterations);
            return Ok(());
        }

        // Build continuation prompt
        let resume_prompt = format!(
            "Continue working on the task. You are resuming from iteration {}.\n\n\
            Original task: {}\n\n\
            When the task is complete, output: <promise>COMPLETE</promise>",
            start_iteration,
            state.prompt
        );

        // Build loop configuration
        let work_dir = std::env::current_dir().ok();
        let loop_config = LoopConfig {
            max_iterations: remaining_iterations,
            budget_limit: None, // Could be calculated from original budget - spent
            model,
            working_dir: work_dir.clone(),
            yolo_mode: false,
            readonly: false,
            system_prompt: None,
            allowed_tools: None,
            enable_state: true,
            enable_cost_tracking: true,
            project_dir: work_dir,
            ..Default::default()
        };

        let engine = LoopEngine::new(loop_config);

        // Create progress bar
        let progress = ProgressBar::new(remaining_iterations as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} iterations ({msg})")
                .unwrap()
                .progress_chars("█▓░"),
        );

        // Execute with event handling
        let (mut rx, handle) = engine.execute(&resume_prompt).await?;

        let mut total_cost = state.total_cost_usd;

        while let Some(event) = rx.recv().await {
            match event {
                LoopEvent::IterationStarted { iteration } => {
                    progress.set_position(iteration as u64);
                    progress.set_message(format!("${:.4}", total_cost));
                }
                LoopEvent::ClaudeEvent(claude_event) => {
                    tracing::debug!("Claude event: {:?}", claude_event);
                }
                LoopEvent::IterationCompleted { iteration, usage, completed } => {
                    total_cost += usage.total_cost_usd;
                    progress.set_position((iteration + 1) as u64);
                    progress.set_message(format!("${:.4}", total_cost));

                    if completed {
                        progress.finish_with_message(format!("${:.4} - Completed!", total_cost));
                    }
                }
                LoopEvent::HookExecuted { hook_type, success, duration_ms } => {
                    tracing::debug!("Hook {}: {} ({}ms)", hook_type, success, duration_ms);
                }
                LoopEvent::LoopFinished { status, total_iterations, total_usage } => {
                    progress.finish_and_clear();
                    println!();
                    self.print_result(&status, start_iteration + total_iterations, &total_usage, total_cost);
                }
            }
        }

        // Wait for the loop to finish
        let result = handle.await??;

        // Print final status if not already printed
        if result.status != LoopStatus::Completed {
            println!();
            self.print_result(&result.status, start_iteration + result.iterations, &result.total_usage, total_cost);
        }

        Ok(())
    }

    /// Print the final result
    fn print_result(
        &self,
        status: &LoopStatus,
        total_iterations: u32,
        usage: &crate::claude::ExecutionUsage,
        total_cost: f64,
    ) {
        use console::{style, Emoji};

        let (emoji, status_text, color) = match status {
            LoopStatus::Completed => (Emoji("✅", "[OK]"), "Completed", console::Color::Green),
            LoopStatus::MaxIterationsReached => (Emoji("⚠️", "[!]"), "Max iterations reached", console::Color::Yellow),
            LoopStatus::BudgetExceeded => (Emoji("💸", "[$]"), "Budget exceeded", console::Color::Yellow),
            LoopStatus::Stopped => (Emoji("🛑", "[X]"), "Stopped", console::Color::Red),
            LoopStatus::Error(e) => {
                println!("{} Error: {}", Emoji("❌", "[ERR]"), style(e).red());
                return;
            }
            LoopStatus::Running => (Emoji("🔄", "[~]"), "Running", console::Color::Blue),
        };

        println!("{} {}", emoji, style(status_text).fg(color).bold());
        println!();
        println!("  Total iterations: {}", total_iterations);
        println!("  Session tokens:   {} in / {} out", usage.input_tokens, usage.output_tokens);
        println!("  Session cost:     ${:.4}", usage.total_cost_usd);
        println!("  Total cost:       ${:.4}", total_cost);
        println!("  Duration:         {:.2}s", usage.duration_ms as f64 / 1000.0);
    }
}
