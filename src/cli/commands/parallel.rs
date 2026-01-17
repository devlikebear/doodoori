use anyhow::Result;
use clap::Args;

use crate::claude::ModelAlias;

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

    /// Dry run - show execution plan without running
    #[arg(long)]
    pub dry_run: bool,
}

impl ParallelArgs {
    pub async fn execute(self) -> Result<()> {
        if self.dry_run {
            return self.execute_dry_run().await;
        }

        tracing::info!("Running parallel tasks with {} workers", self.workers);

        if !self.task.is_empty() {
            println!("Tasks:");
            for (i, task) in self.task.iter().enumerate() {
                let (desc, model) = Self::parse_task(task);
                let model = self.model.clone().unwrap_or(model);
                println!("  {}. {} (model: {:?})", i + 1, desc, model);
            }
        }

        if let Some(spec) = &self.spec {
            println!("Spec file: {}", spec);
            // TODO: Parse spec and extract tasks
        }

        if let Some(specs) = &self.specs {
            println!("Spec pattern: {}", specs);
            // TODO: Glob and load multiple specs
        }

        // TODO: Execute parallel tasks
        println!("\n🔨 Doodoori is forging {} tasks in parallel...", self.task.len());

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
