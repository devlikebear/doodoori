use anyhow::Result;
use clap::Args;
use std::path::Path;

use crate::claude::ModelAlias;
use crate::executor::{
    ParallelConfig, ParallelEvent, ParallelExecutor, TaskDefinition, TaskStatus,
};
use crate::git::sanitize_branch_name;
use crate::instructions::{SpecFile, SpecParser};
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

    /// Use git worktrees for task isolation (each task gets its own branch)
    #[arg(long)]
    pub git_worktree: bool,

    /// Branch prefix for git worktrees (e.g., "feature/", "task/")
    #[arg(long, default_value = "task/")]
    pub branch_prefix: String,

    /// Auto-commit changes on task completion
    #[arg(long)]
    pub auto_commit: bool,

    /// Auto-create PR on task completion
    #[arg(long)]
    pub auto_pr: bool,
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

            let task = TaskDefinition::new(desc.clone())
                .with_name(desc) // Use description as name for branch naming
                .with_model(model)
                .with_max_iterations(self.max_iterations)
                .with_yolo_mode(self.yolo);

            tasks.push(task);
        }

        // Add tasks from single spec file
        if let Some(ref spec_path) = self.spec {
            let spec_tasks = self.load_spec_file(spec_path)?;
            tasks.extend(spec_tasks);
        }

        // Add tasks from glob pattern (multiple spec files)
        if let Some(ref pattern) = self.specs {
            let spec_tasks = self.load_specs_pattern(pattern)?;
            tasks.extend(spec_tasks);
        }

        if tasks.is_empty() {
            println!("No tasks specified. Use --task or --spec to add tasks.");
            return Ok(());
        }

        if self.git_worktree {
            println!(
                "🔨 Doodoori is forging {} tasks in parallel with {} workers (git worktree mode)...\n",
                tasks.len(),
                self.workers
            );
        } else {
            println!(
                "🔨 Doodoori is forging {} tasks in parallel with {} workers...\n",
                tasks.len(),
                self.workers
            );
        }

        // Create executor config
        let config = ParallelConfig {
            workers: self.workers,
            total_budget: self.budget,
            fail_fast: self.fail_fast,
            sandbox: self.sandbox,
            base_working_dir: std::env::current_dir().ok(),
            task_isolation: self.isolate || self.git_worktree, // Worktrees imply isolation
            use_git_worktrees: self.git_worktree,
            git_branch_prefix: self.branch_prefix.clone(),
            git_auto_commit: self.auto_commit,
            git_auto_pr: self.auto_pr,
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
            println!("\n  From spec file: {}", spec);
            if let Ok(tasks) = self.load_spec_file(spec) {
                for (i, task) in tasks.iter().enumerate() {
                    let name = task.name.as_deref().unwrap_or("unnamed");
                    println!("    {}. \"{}\" ({:?})", i + 1, name, task.model);
                    if self.git_worktree {
                        if let Some(ref branch) = task.git_branch {
                            println!("       Branch: {}", branch);
                        }
                    }
                }
            }
        }

        if let Some(specs) = &self.specs {
            println!("\n  From pattern: {}", specs);
            if let Ok(tasks) = self.load_specs_pattern(specs) {
                for (i, task) in tasks.iter().enumerate() {
                    let name = task.name.as_deref().unwrap_or("unnamed");
                    println!("    {}. \"{}\" ({:?})", i + 1, name, task.model);
                    if self.git_worktree {
                        if let Some(ref branch) = task.git_branch {
                            println!("       Branch: {}", branch);
                        }
                    }
                }
            }
        }

        println!("\n[Execution Mode]");
        if self.sandbox {
            println!("  Sandbox (Docker) - isolated containers per task");
        } else if self.git_worktree {
            println!("  Git Worktree - each task gets its own worktree and branch");
            println!("    Branch prefix: {}", self.branch_prefix);
            println!("    Auto-commit: {}", self.auto_commit);
            println!("    Auto-PR: {}", self.auto_pr);
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

    /// Load tasks from a single spec file
    fn load_spec_file(&self, spec_path: &str) -> Result<Vec<TaskDefinition>> {
        let path = Path::new(spec_path);

        if !path.exists() {
            anyhow::bail!("Spec file not found: {}", spec_path);
        }

        let spec = SpecParser::parse_file(path)?;
        let mut tasks = Vec::new();

        // Check if this is a multi-task spec
        if spec.is_multi_task() {
            // Each TaskSpec becomes a separate TaskDefinition
            for task_spec in &spec.tasks {
                let default_model = spec.effective_model();
                let model = self.model.clone()
                    .unwrap_or_else(|| task_spec.effective_model(&default_model));

                let max_iterations = task_spec.max_iterations
                    .unwrap_or(self.max_iterations);

                // Build prompt from task spec description + requirements
                let mut prompt = format!("# Task: {}\n\n", task_spec.id);
                prompt.push_str(&format!("## Objective\n{}\n\n", task_spec.description));

                if !task_spec.requirements.is_empty() {
                    prompt.push_str("## Requirements\n");
                    for req in &task_spec.requirements {
                        let checkbox = if req.completed { "[x]" } else { "[ ]" };
                        prompt.push_str(&format!("- {} {}\n", checkbox, req.description));
                    }
                    prompt.push('\n');
                }

                if let Some(ref criteria) = task_spec.completion_criteria {
                    prompt.push_str(&format!("## Completion Criteria\n{}\n\n", criteria));
                }

                prompt.push_str("---\n\n");
                prompt.push_str(&format!(
                    "When complete, output: {}\n",
                    spec.effective_completion_promise()
                ));

                let task = TaskDefinition::new(prompt)
                    .with_name(task_spec.id.clone())
                    .with_model(model)
                    .with_max_iterations(max_iterations)
                    .with_yolo_mode(self.yolo);

                tasks.push(task);
            }
        } else {
            // Single task spec - use the whole spec as one task
            let task = self.spec_to_task(&spec, spec_path)?;
            tasks.push(task);
        }

        Ok(tasks)
    }

    /// Load tasks from glob pattern (multiple spec files)
    fn load_specs_pattern(&self, pattern: &str) -> Result<Vec<TaskDefinition>> {
        let mut tasks = Vec::new();
        let mut found_files = 0;

        for entry in glob::glob(pattern)? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        found_files += 1;
                        let spec = SpecParser::parse_file(&path)?;
                        let path_str = path.to_string_lossy().to_string();

                        // Each spec file becomes one task
                        let task = self.spec_to_task(&spec, &path_str)?;

                        println!("  📄 Loaded: {} ({})", spec.title, path.display());
                        tasks.push(task);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read glob entry: {}", e);
                }
            }
        }

        if found_files == 0 {
            println!("  ⚠ No spec files found matching pattern: {}", pattern);
        } else {
            println!("  ✓ Loaded {} spec files\n", found_files);
        }

        Ok(tasks)
    }

    /// Convert a SpecFile to a TaskDefinition
    fn spec_to_task(&self, spec: &SpecFile, source: &str) -> Result<TaskDefinition> {
        let model = self.model.clone()
            .unwrap_or_else(|| spec.effective_model());

        let max_iterations = spec.max_iterations
            .unwrap_or(self.max_iterations);

        // Use the spec's to_prompt() method to generate the full prompt
        let prompt = spec.to_prompt();

        // Use spec title for task name (used for branch naming)
        let task_name = if spec.title.is_empty() {
            // Fallback to filename if no title
            Path::new(source)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("task")
                .to_string()
        } else {
            spec.title.clone()
        };

        // Generate branch name from task name
        let branch_name = format!(
            "{}{}",
            self.branch_prefix,
            sanitize_branch_name(&task_name)
        );

        let mut task = TaskDefinition::new(prompt)
            .with_name(task_name)
            .with_model(model)
            .with_max_iterations(max_iterations)
            .with_yolo_mode(self.yolo);

        // Set the git branch if worktree mode is enabled
        if self.git_worktree {
            task = task.with_git_branch(branch_name);
        }

        // Set budget if specified in spec
        if let Some(budget) = spec.budget {
            task = task.with_budget(budget);
        }

        Ok(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_args() -> ParallelArgs {
        ParallelArgs {
            task: Vec::new(),
            spec: None,
            specs: None,
            workers: 3,
            model: None,
            budget: None,
            sandbox: false,
            isolate: false,
            fail_fast: false,
            max_iterations: 50,
            yolo: false,
            dry_run: false,
            git_worktree: false,
            branch_prefix: "feature/".to_string(),
            auto_commit: false,
            auto_pr: false,
        }
    }

    #[test]
    fn test_parse_task_simple() {
        let (desc, model) = ParallelArgs::parse_task("Build REST API");
        assert_eq!(desc, "Build REST API");
        assert_eq!(model, ModelAlias::Sonnet);
    }

    #[test]
    fn test_parse_task_with_model() {
        let (desc, model) = ParallelArgs::parse_task("Build REST API:opus");
        assert_eq!(desc, "Build REST API");
        assert_eq!(model, ModelAlias::Opus);

        let (desc2, model2) = ParallelArgs::parse_task("Quick fix:haiku");
        assert_eq!(desc2, "Quick fix");
        assert_eq!(model2, ModelAlias::Haiku);
    }

    #[test]
    fn test_load_spec_file() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.md");

        let spec_content = r#"# Task: Test API

## Objective
Build a simple REST API

## Requirements
- [ ] GET /api endpoint
- [ ] POST /api endpoint

## Completion Criteria
All tests pass
"#;

        std::fs::write(&spec_path, spec_content).unwrap();

        let args = create_test_args();
        let tasks = args.load_spec_file(spec_path.to_str().unwrap()).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, Some("Test API".to_string()));
        assert!(tasks[0].prompt.contains("Build a simple REST API"));
    }

    #[test]
    fn test_load_spec_file_with_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("backend.md");

        let spec_content = r#"# Task: Backend API Development

## Objective
Implement the backend API

## Requirements
- [ ] Create database models
- [ ] Implement REST endpoints
"#;

        std::fs::write(&spec_path, spec_content).unwrap();

        let mut args = create_test_args();
        args.git_worktree = true;
        args.branch_prefix = "feature/".to_string();

        let tasks = args.load_spec_file(spec_path.to_str().unwrap()).unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, Some("Backend API Development".to_string()));
        assert_eq!(tasks[0].git_branch, Some("feature/backend-api-development".to_string()));
    }

    #[test]
    fn test_load_specs_pattern() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple spec files
        let spec1 = r#"# Task: Backend API

## Objective
Build backend
"#;
        let spec2 = r#"# Task: Frontend UI

## Objective
Build frontend
"#;
        let spec3 = r#"# Task: Database

## Objective
Setup database
"#;

        std::fs::write(temp_dir.path().join("backend.md"), spec1).unwrap();
        std::fs::write(temp_dir.path().join("frontend.md"), spec2).unwrap();
        std::fs::write(temp_dir.path().join("database.md"), spec3).unwrap();

        let mut args = create_test_args();
        args.git_worktree = true;

        let pattern = format!("{}/*.md", temp_dir.path().display());
        let tasks = args.load_specs_pattern(&pattern).unwrap();

        assert_eq!(tasks.len(), 3);

        // Check that branch names are generated
        let branch_names: Vec<_> = tasks.iter()
            .filter_map(|t| t.git_branch.as_ref())
            .collect();
        assert_eq!(branch_names.len(), 3);

        // Each branch should have the feature/ prefix
        for branch in &branch_names {
            assert!(branch.starts_with("feature/"), "Branch {} should start with feature/", branch);
        }
    }

    #[test]
    fn test_spec_to_task_branch_naming() {
        let temp_dir = TempDir::new().unwrap();
        let spec_path = temp_dir.path().join("test.md");

        // Test with special characters that should be sanitized
        let spec_content = r#"# Task: My Feature Task (v2.0)

## Objective
Test sanitization
"#;
        std::fs::write(&spec_path, spec_content).unwrap();

        let mut args = create_test_args();
        args.git_worktree = true;
        args.branch_prefix = "task/".to_string();

        let tasks = args.load_spec_file(spec_path.to_str().unwrap()).unwrap();

        assert_eq!(tasks.len(), 1);
        // Should be sanitized: spaces -> hyphens, special chars removed/replaced
        let branch = tasks[0].git_branch.as_ref().unwrap();
        assert!(branch.starts_with("task/"));
        assert!(!branch.contains(' '));
        assert!(!branch.contains('('));
        assert!(!branch.contains(')'));
    }
}
