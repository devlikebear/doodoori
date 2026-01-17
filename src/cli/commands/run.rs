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

    /// Docker image for sandbox mode
    #[arg(long, default_value = "doodoori/sandbox:latest")]
    pub image: String,

    /// Network mode for sandbox (bridge, none, host)
    #[arg(long, default_value = "bridge")]
    pub network: String,

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
            return self.execute_sandbox(&prompt).await;
        }

        if self.yolo {
            tracing::warn!("YOLO mode enabled - all permissions granted!");
        }

        // TODO: Execute with Loop Engine (direct mode)
        println!("🔨 Doodoori is forging your code...");
        println!("Task: {}", prompt);
        println!("Model: {:?}", self.model);

        Ok(())
    }

    #[cfg(feature = "sandbox")]
    async fn execute_sandbox(&self, prompt: &str) -> Result<()> {
        use crate::sandbox::{ClaudeOptions, NetworkMode, SandboxConfig, SandboxRunner};

        println!("🐳 Initializing Docker sandbox...");

        // Parse network mode
        let network = match self.network.to_lowercase().as_str() {
            "none" => NetworkMode::None,
            "host" => NetworkMode::Host,
            _ => NetworkMode::Bridge,
        };

        // Get current working directory
        let workspace = std::env::current_dir()?;

        // Build sandbox configuration
        // By default, uses Docker volume for Claude credentials (recommended)
        let config = SandboxConfig::builder()
            .image(&self.image)
            .network(network)
            .workspace(&workspace)
            .build();

        // Create and initialize runner
        let mut runner = SandboxRunner::with_config(config)?;
        runner.init().await?;

        println!("📦 Sandbox container started: {}", runner.container_id().unwrap_or("unknown"));
        println!("🔨 Executing Claude Code in sandbox...");
        println!("   Image: {}", self.image);
        println!("   Network: {}", self.network);
        println!("   Workspace: {}", workspace.display());

        // First, check if Claude is authenticated in the sandbox
        println!("\n🔍 Checking Claude authentication in sandbox...");
        let auth_check = runner.exec(vec!["claude", "--version"]).await?;
        println!("   Claude version check: exit_code={}", auth_check.exit_code);
        if !auth_check.stdout.is_empty() {
            println!("   stdout: {}", auth_check.stdout.trim());
        }
        if !auth_check.stderr.is_empty() {
            println!("   stderr: {}", auth_check.stderr.trim());
        }

        // Check if credentials exist
        let cred_check = runner.exec(vec!["ls", "-la", "/home/doodoori/.claude/"]).await;
        match cred_check {
            Ok(output) => {
                println!("   Credentials directory:");
                for line in output.stdout.lines().take(5) {
                    println!("     {}", line);
                }
            }
            Err(e) => {
                println!("   ⚠️  No credentials found: {}", e);
                println!("   Run 'doodoori sandbox login' first to authenticate.");
            }
        }

        // Build Claude options
        let options = ClaudeOptions::new()
            .model(self.model.to_string());

        let options = if self.yolo {
            options.yolo()
        } else {
            options
        };

        println!("\n⏳ Executing Claude command (this may take a while)...");
        println!("   Command: claude -p \"{}\" --output-format stream-json --verbose", prompt);

        // Execute Claude
        let result = runner.run_claude(prompt, &options).await?;

        // Print output
        if !result.output.stdout.is_empty() {
            println!("\n--- Output ---");
            println!("{}", result.output.stdout);
        }

        if !result.output.stderr.is_empty() {
            eprintln!("\n--- Errors ---");
            eprintln!("{}", result.output.stderr);
        }

        println!("\n--- Result ---");
        println!("Exit code: {}", result.output.exit_code);
        println!("Success: {}", result.success);

        // Cleanup
        runner.cleanup().await?;
        println!("🧹 Sandbox cleaned up");

        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    async fn execute_sandbox(&self, _prompt: &str) -> Result<()> {
        anyhow::bail!(
            "Sandbox feature is not enabled. Rebuild with --features sandbox:\n\
             cargo build --features sandbox"
        )
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
            println!("    Image: {}", self.image);
            println!("    Network: {}", self.network);
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
