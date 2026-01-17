use anyhow::Result;
use clap::Args;

use crate::claude::ModelAlias;

/// Run a task with Claude Code
#[derive(Args, Debug)]
pub struct RunArgs {
    /// The task prompt or description
    #[arg(required_unless_present = "spec")]
    pub prompt: Option<String>,

    /// Path to spec file (markdown)
    #[arg(short, long)]
    pub spec: Option<String>,

    /// Model to use (haiku, sonnet, opus)
    #[arg(short, long, default_value = "sonnet")]
    pub model: ModelAlias,

    /// Maximum budget in USD
    #[arg(short, long)]
    pub budget: Option<f64>,

    /// Maximum iterations for loop engine
    #[arg(long, default_value = "50")]
    pub max_iterations: u32,

    /// Run in sandbox mode (Docker)
    #[arg(long)]
    pub sandbox: bool,

    /// Dry run - show what would be executed without running
    #[arg(long)]
    pub dry_run: bool,

    /// Skip all permission prompts (DANGEROUS)
    #[arg(long)]
    pub yolo: bool,

    /// Read-only mode - no file modifications
    #[arg(long)]
    pub readonly: bool,

    /// Additional tools to allow
    #[arg(long)]
    pub allow: Option<String>,

    /// Custom instructions file path
    #[arg(long)]
    pub instructions: Option<String>,

    /// Skip reading doodoori.md instructions
    #[arg(long)]
    pub no_instructions: bool,

    /// Disable git workflow (no commits, PRs)
    #[arg(long)]
    pub no_git: bool,

    /// Disable automatic PR merge
    #[arg(long)]
    pub no_auto_merge: bool,
}

impl RunArgs {
    pub async fn execute(self) -> Result<()> {
        if self.dry_run {
            return self.execute_dry_run().await;
        }

        let prompt = if let Some(spec_path) = &self.spec {
            // Load spec file
            tracing::info!("Loading spec file: {}", spec_path);
            // TODO: Parse spec file and extract prompt
            format!("Spec from: {}", spec_path)
        } else {
            self.prompt.clone().unwrap()
        };

        tracing::info!("Running task with model: {:?}", self.model);
        tracing::info!("Prompt: {}", prompt);
        tracing::info!("Max iterations: {}", self.max_iterations);

        if let Some(budget) = self.budget {
            tracing::info!("Budget limit: ${:.2}", budget);
        }

        if self.sandbox {
            tracing::info!("Sandbox mode enabled");
        }

        if self.yolo {
            tracing::warn!("YOLO mode enabled - all permissions granted!");
        }

        // TODO: Execute with Loop Engine
        println!("🔨 Doodoori is forging your code...");
        println!("Task: {}", prompt);
        println!("Model: {:?}", self.model);

        Ok(())
    }

    async fn execute_dry_run(&self) -> Result<()> {
        println!("=== Dry Run Preview ===\n");

        println!("[Prompt]");
        if let Some(spec) = &self.spec {
            println!("  Spec file: {}", spec);
        } else if let Some(prompt) = &self.prompt {
            println!("  \"{}\"", prompt);
        }

        println!("\n[Model]");
        println!("  {:?}", self.model);

        println!("\n[Estimated Cost]");
        println!("  (Cost estimation not yet implemented)");

        println!("\n[Permissions]");
        if self.yolo {
            println!("  Mode: YOLO (all permissions granted)");
        } else if self.readonly {
            println!("  Mode: Read-only");
            println!("  Allowed: Read, Grep, Glob");
        } else {
            println!("  Allowed: Read, Write, Edit, Grep, Glob");
            if let Some(allow) = &self.allow {
                println!("  Additional: {}", allow);
            }
        }

        println!("\n[Execution Mode]");
        if self.sandbox {
            println!("  Sandbox (Docker)");
        } else {
            println!("  Direct (local)");
        }

        println!("\n[Loop Engine]");
        println!("  Max iterations: {}", self.max_iterations);
        println!("  Completion promise: \"COMPLETE\"");

        if let Some(budget) = self.budget {
            println!("\n[Budget]");
            println!("  Limit: ${:.2}", budget);
        }

        println!("\n[Git Workflow]");
        if self.no_git {
            println!("  Disabled");
        } else {
            println!("  Enabled");
            println!("  Auto-merge: {}", !self.no_auto_merge);
        }

        println!("\n=== End Preview ===");

        Ok(())
    }
}
