//! Resume command for continuing interrupted tasks

use anyhow::{Context, Result};
use clap::Args;

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

        println!("Resuming task: {} ({})", state.short_id(), state.status);
        println!("Prompt: {}", state.prompt);
        println!("Starting from iteration: {}", state.current_iteration);

        if let Some(from_iter) = self.from_iteration {
            if from_iter < state.current_iteration {
                println!("Resetting to iteration: {}", from_iter);
                state.current_iteration = from_iter;
            }
        }

        // Check for session ID for context carry-over
        if state.session_id.is_some() {
            println!("Session context available for --continue");
        }

        // Update status to running
        state.status = TaskStatus::Running;
        state_manager.save_state(&state)?;

        // TODO: Actually resume the task using loop engine
        // This would call the loop engine with the saved state
        // For now, we just show what would happen

        println!();
        println!("Resume functionality is ready.");
        println!("Full integration with loop engine will be completed in the next step.");
        println!();
        println!("To manually continue with Claude CLI:");
        if let Some(ref session_id) = state.session_id {
            println!("  claude --continue {} -p \"Continue the task\"", session_id);
        } else {
            println!("  claude -p \"{}\"", state.prompt);
        }

        Ok(())
    }
}
