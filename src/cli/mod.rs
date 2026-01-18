pub mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;

use commands::{
    cost::CostArgs, dashboard::DashboardArgs, git::GitArgs, parallel::ParallelArgs,
    resume::ResumeArgs, run::RunArgs, sandbox::SandboxArgs, secret::SecretArgs, spec::SpecArgs,
    template::TemplateCommand, watch::WatchArgs, workflow::WorkflowArgs,
};
use crate::config::DoodooriConfig;
use crate::pricing::{format_cost, CostCalculator, PricingConfig};
use crate::claude::ModelAlias;

/// Doodoori - Autonomous CLI tool powered by Claude Code
///
/// Named after the Silla dynasty's blacksmith deity (두두리, 豆豆里),
/// Doodoori forges code through persistent iteration until completion.
#[derive(Parser, Debug)]
#[command(name = "doodoori")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Config file path
    #[arg(short, long, global = true, default_value = "doodoori.toml")]
    pub config: String,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a task with Claude Code
    Run(RunArgs),

    /// Run multiple tasks in parallel
    Parallel(ParallelArgs),

    /// Generate or manage spec files
    Spec(SpecArgs),

    /// Manage Docker sandbox environment
    Sandbox(SandboxArgs),

    /// Resume an interrupted task
    Resume(ResumeArgs),

    /// Manage secrets in the system keychain
    Secret(SecretArgs),

    /// View cost history and tracking
    Cost(CostArgs),

    /// Run and manage workflows
    Workflow(WorkflowArgs),

    /// Launch the TUI dashboard
    Dashboard(DashboardArgs),

    /// Git workflow management (worktrees, commits, PRs)
    Git(GitArgs),

    /// Watch for file changes and run tasks automatically
    Watch(WatchArgs),

    /// Manage templates
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },

    /// Show current configuration
    Config,

    /// Update price information
    #[command(name = "price")]
    Price {
        /// Update prices from the web
        #[arg(long)]
        update: bool,

        /// Show price for specific model
        #[arg(long)]
        model: Option<String>,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Run(args) => args.execute().await,
            Commands::Parallel(args) => args.execute().await,
            Commands::Spec(args) => args.execute().await,
            Commands::Sandbox(args) => args.execute().await,
            Commands::Resume(args) => args.execute().await,
            Commands::Secret(args) => args.execute().await,
            Commands::Cost(args) => args.execute().await,
            Commands::Workflow(args) => args.execute().await,
            Commands::Dashboard(args) => args.execute().await,
            Commands::Git(args) => args.execute().await,
            Commands::Watch(args) => args.execute().await,
            Commands::Template { command } => {
                match command {
                    TemplateCommand::List(args) => args.execute().await,
                    TemplateCommand::Show(args) => args.execute().await,
                    TemplateCommand::Use(args) => args.execute().await,
                    TemplateCommand::Create(args) => args.execute().await,
                    TemplateCommand::Delete(args) => args.execute().await,
                }
            }
            Commands::Config => {
                self.execute_config().await
            }
            Commands::Price { update, ref model } => {
                self.execute_price(update, model.clone()).await
            }
        }
    }

    /// Execute the config command - display current configuration
    async fn execute_config(&self) -> Result<()> {
        use console::{style, Emoji};

        let config_path = Path::new(&self.config);
        let config = DoodooriConfig::from_file(config_path)?;

        println!("{} Configuration", Emoji("⚙️", ""));
        println!();

        // Config file info
        if config_path.exists() {
            println!("  Config file: {} {}", self.config, style("(loaded)").green());
        } else {
            println!("  Config file: {} {}", self.config, style("(using defaults)").dim());
        }
        println!();

        // General settings
        println!("{}", style("[General]").bold());
        println!("  Default model:     {}", config.default_model);
        println!("  Max iterations:    {}", config.max_iterations);
        println!("  Budget limit:      {}", config.budget_limit
            .map(|b| format!("${:.2}", b))
            .unwrap_or_else(|| "unlimited".to_string()));
        println!("  YOLO mode:         {}", if config.yolo_mode { "enabled" } else { "disabled" });
        println!("  Sandbox mode:      {}", if config.sandbox_mode { "enabled" } else { "disabled" });
        if let Some(ref instructions) = config.instructions_file {
            let exists = instructions.exists();
            println!("  Instructions:      {} {}",
                instructions.display(),
                if exists { style("✓").green() } else { style("(not found)").dim() }
            );
        }
        println!();

        // Git settings
        println!("{}", style("[Git]").bold());
        println!("  Enabled:           {}", if config.git.enabled { "yes" } else { "no" });
        println!("  Auto branch:       {}", if config.git.auto_branch { "yes" } else { "no" });
        println!("  Auto commit:       {}", if config.git.auto_commit { "yes" } else { "no" });
        println!("  Auto PR:           {}", if config.git.auto_pr { "yes" } else { "no" });
        println!("  Auto merge:        {}", if config.git.auto_merge { "yes" } else { "no" });
        println!("  Branch prefix:     {}", config.git.branch_prefix);
        println!();

        // Logging settings
        println!("{}", style("[Logging]").bold());
        println!("  Level:             {}", config.logging.level);
        println!("  Progress:          {}", if config.logging.progress { "enabled" } else { "disabled" });
        if let Some(ref file) = config.logging.file {
            println!("  Log file:          {}", file.display());
        }
        println!();

        // Parallel settings
        println!("{}", style("[Parallel]").bold());
        println!("  Workers:           {}", config.parallel.workers);
        println!("  Isolate:           {}", if config.parallel.isolate_workspaces { "yes" } else { "no" });

        Ok(())
    }

    /// Execute the price command - display model pricing
    async fn execute_price(&self, update: bool, model: Option<String>) -> Result<()> {
        use console::{style, Emoji};

        if update {
            #[cfg(feature = "price-update")]
            {
                println!("Updating prices from Anthropic...");
                // Price update logic would go here
                println!("Price update not yet implemented");
            }
            #[cfg(not(feature = "price-update"))]
            {
                println!("{} Price update requires --features price-update", Emoji("⚠️", "[!]"));
                println!("  cargo build --features price-update");
            }
            return Ok(());
        }

        let pricing = PricingConfig::default_pricing();
        let calc = CostCalculator::with_default_pricing();

        println!("{} Model Pricing", Emoji("💰", ""));
        println!();
        println!("  Version:  {}", pricing.meta.version);
        println!("  Updated:  {}", pricing.meta.updated_at);
        println!();

        if let Some(model_name) = model {
            // Show specific model
            let model_alias: ModelAlias = model_name.parse()
                .map_err(|_| anyhow::anyhow!("Unknown model: {}. Use haiku, sonnet, or opus", model_name))?;

            if let Some(model_pricing) = pricing.get_model_by_alias(&model_alias) {
                println!("{}", style(format!("[{}]", model_pricing.name)).bold());
                println!("  Family:              {}", model_pricing.family);
                println!("  Input:               {}/MTok", format_cost(model_pricing.input_per_mtok));
                println!("  Output:              {}/MTok", format_cost(model_pricing.output_per_mtok));
                if let Some(cache_read) = model_pricing.cache_read_per_mtok {
                    println!("  Cache read:          {}/MTok", format_cost(cache_read));
                }
                if let Some(cache_write) = model_pricing.cache_write_5m_per_mtok {
                    println!("  Cache write (5m):    {}/MTok", format_cost(cache_write));
                }
                if let Some(max_ctx) = model_pricing.max_context_tokens {
                    println!("  Max context:         {} tokens", max_ctx);
                }
                if model_pricing.deprecated {
                    println!("  Status:              {} Deprecated", style("⚠").yellow());
                    if let Some(ref date) = model_pricing.deprecation_date {
                        println!("  Deprecation date:    {}", date);
                    }
                }
            } else {
                println!("Model '{}' not found in pricing data", model_name);
            }
        } else {
            // Show all models
            println!("{}", style("[Active Models]").bold());
            println!();
            println!("  {:<12} {:<15} {:<15} {:<12}",
                style("ALIAS").dim(),
                style("INPUT/MTOK").dim(),
                style("OUTPUT/MTOK").dim(),
                style("FAMILY").dim()
            );
            println!("  {}", "-".repeat(55));

            // Show main aliases first
            for alias in &[ModelAlias::Haiku, ModelAlias::Sonnet, ModelAlias::Opus] {
                if let Some(model_pricing) = pricing.get_model_by_alias(alias) {
                    println!("  {:<12} {:<15} {:<15} {:<12}",
                        format!("{:?}", alias).to_lowercase(),
                        format_cost(model_pricing.input_per_mtok),
                        format_cost(model_pricing.output_per_mtok),
                        model_pricing.family
                    );
                }
            }

            // Cost examples
            println!();
            println!("{}", style("[Cost Examples (1M input + 100K output)]").bold());
            println!();

            for alias in &[ModelAlias::Haiku, ModelAlias::Sonnet, ModelAlias::Opus] {
                let estimate = calc.estimate_cost(alias, 1_000_000, 100_000);
                println!("  {:?}:  {} (input: {}, output: {})",
                    alias,
                    format_cost(estimate.total_cost),
                    format_cost(estimate.input_cost),
                    format_cost(estimate.output_cost)
                );
            }

            println!();
            println!("Use 'doodoori price --model <name>' for detailed info");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_run_with_prompt() {
        let cli = Cli::try_parse_from(["doodoori", "run", "Build a REST API"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.prompt, Some("Build a REST API".to_string()));
                assert_eq!(args.spec, None);
                assert!(!args.dry_run);
                assert!(!args.sandbox);
                assert!(!args.yolo);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_with_spec() {
        let cli = Cli::try_parse_from(["doodoori", "run", "--spec", "task.md"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.prompt, None);
                assert_eq!(args.spec, Some("task.md".to_string()));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_with_model() {
        use crate::claude::ModelAlias;

        let cli = Cli::try_parse_from(["doodoori", "run", "-m", "opus", "My task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.model, ModelAlias::Opus);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_dry_run() {
        let cli = Cli::try_parse_from(["doodoori", "run", "--dry-run", "Test task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert!(args.dry_run);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_sandbox_mode() {
        let cli = Cli::try_parse_from(["doodoori", "run", "--sandbox", "Test task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert!(args.sandbox);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_yolo_mode() {
        let cli = Cli::try_parse_from(["doodoori", "run", "--yolo", "Test task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert!(args.yolo);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_budget() {
        let cli =
            Cli::try_parse_from(["doodoori", "run", "--budget", "5.0", "Test task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.budget, Some(5.0));
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_max_iterations() {
        let cli =
            Cli::try_parse_from(["doodoori", "run", "--max-iterations", "100", "Test task"])
                .unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert_eq!(args.max_iterations, 100);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_run_no_git() {
        let cli = Cli::try_parse_from(["doodoori", "run", "--no-git", "Test task"]).unwrap();

        match cli.command {
            Commands::Run(args) => {
                assert!(args.no_git);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_cli_parallel_basic() {
        let cli = Cli::try_parse_from(["doodoori", "parallel", "-t", "Build API"]).unwrap();

        match cli.command {
            Commands::Parallel(args) => {
                assert_eq!(args.task, vec!["Build API".to_string()]);
                assert_eq!(args.workers, 3); // default value
            }
            _ => panic!("Expected Parallel command"),
        }
    }

    #[test]
    fn test_cli_parallel_workers() {
        let cli = Cli::try_parse_from(["doodoori", "parallel", "-w", "8", "-t", "Task 1"]).unwrap();

        match cli.command {
            Commands::Parallel(args) => {
                assert_eq!(args.workers, 8);
            }
            _ => panic!("Expected Parallel command"),
        }
    }

    #[test]
    fn test_cli_parallel_multiple_tasks() {
        let cli = Cli::try_parse_from([
            "doodoori",
            "parallel",
            "-t",
            "Task 1:haiku",
            "-t",
            "Task 2:opus",
        ])
        .unwrap();

        match cli.command {
            Commands::Parallel(args) => {
                assert_eq!(args.task.len(), 2);
                assert_eq!(args.task[0], "Task 1:haiku");
                assert_eq!(args.task[1], "Task 2:opus");
            }
            _ => panic!("Expected Parallel command"),
        }
    }

    #[test]
    fn test_cli_parallel_specs_pattern() {
        let cli = Cli::try_parse_from(["doodoori", "parallel", "--specs", "specs/*.md"]).unwrap();

        match cli.command {
            Commands::Parallel(args) => {
                assert_eq!(args.specs, Some("specs/*.md".to_string()));
            }
            _ => panic!("Expected Parallel command"),
        }
    }

    #[test]
    fn test_cli_spec_generate() {
        let cli = Cli::try_parse_from(["doodoori", "spec", "Create a user login feature"]).unwrap();

        match cli.command {
            Commands::Spec(args) => {
                assert_eq!(args.description, Some("Create a user login feature".to_string()));
                assert!(!args.interactive);
            }
            _ => panic!("Expected Spec command"),
        }
    }

    #[test]
    fn test_cli_spec_interactive() {
        let cli = Cli::try_parse_from(["doodoori", "spec", "--interactive"]).unwrap();

        match cli.command {
            Commands::Spec(args) => {
                assert!(args.interactive);
            }
            _ => panic!("Expected Spec command"),
        }
    }

    #[test]
    fn test_cli_spec_validate() {
        let cli = Cli::try_parse_from(["doodoori", "spec", "--validate", "task.md"]).unwrap();

        match cli.command {
            Commands::Spec(args) => {
                assert_eq!(args.validate, Some("task.md".to_string()));
            }
            _ => panic!("Expected Spec command"),
        }
    }

    #[test]
    fn test_cli_spec_output() {
        let cli = Cli::try_parse_from([
            "doodoori",
            "spec",
            "-o",
            "output.md",
            "My task description",
        ])
        .unwrap();

        match cli.command {
            Commands::Spec(args) => {
                assert_eq!(args.output, Some("output.md".to_string()));
                assert_eq!(args.description, Some("My task description".to_string()));
            }
            _ => panic!("Expected Spec command"),
        }
    }

    #[test]
    fn test_cli_cost_basic() {
        let cli = Cli::try_parse_from(["doodoori", "cost"]).unwrap();

        match cli.command {
            Commands::Cost(args) => {
                assert!(!args.history);
                assert!(!args.daily);
                assert!(!args.reset);
                assert!(args.task_id.is_none());
            }
            _ => panic!("Expected Cost command"),
        }
    }

    #[test]
    fn test_cli_cost_history() {
        let cli = Cli::try_parse_from(["doodoori", "cost", "--history"]).unwrap();

        match cli.command {
            Commands::Cost(args) => {
                assert!(args.history);
            }
            _ => panic!("Expected Cost command"),
        }
    }

    #[test]
    fn test_cli_cost_daily() {
        let cli = Cli::try_parse_from(["doodoori", "cost", "--daily"]).unwrap();

        match cli.command {
            Commands::Cost(args) => {
                assert!(args.daily);
            }
            _ => panic!("Expected Cost command"),
        }
    }

    #[test]
    fn test_cli_cost_task_id() {
        let cli = Cli::try_parse_from(["doodoori", "cost", "--task-id", "abc123"]).unwrap();

        match cli.command {
            Commands::Cost(args) => {
                assert_eq!(args.task_id, Some("abc123".to_string()));
            }
            _ => panic!("Expected Cost command"),
        }
    }

    #[test]
    fn test_cli_config() {
        let cli = Cli::try_parse_from(["doodoori", "config"]).unwrap();

        matches!(cli.command, Commands::Config);
    }

    #[test]
    fn test_cli_price() {
        let cli = Cli::try_parse_from(["doodoori", "price"]).unwrap();

        match cli.command {
            Commands::Price { update, model } => {
                assert!(!update);
                assert!(model.is_none());
            }
            _ => panic!("Expected Price command"),
        }
    }

    #[test]
    fn test_cli_price_update() {
        let cli = Cli::try_parse_from(["doodoori", "price", "--update"]).unwrap();

        match cli.command {
            Commands::Price { update, .. } => {
                assert!(update);
            }
            _ => panic!("Expected Price command"),
        }
    }

    #[test]
    fn test_cli_verbose() {
        let cli = Cli::try_parse_from(["doodoori", "-v", "run", "Test"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn test_cli_custom_config() {
        let cli =
            Cli::try_parse_from(["doodoori", "-c", "custom.toml", "run", "Test"]).unwrap();
        assert_eq!(cli.config, "custom.toml");
    }

    #[test]
    fn test_cli_run_requires_prompt_or_spec() {
        // Should fail without prompt or spec
        let result = Cli::try_parse_from(["doodoori", "run"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_watch_basic() {
        let cli = Cli::try_parse_from(["doodoori", "watch", "Run tests"]).unwrap();

        match cli.command {
            Commands::Watch(args) => {
                assert_eq!(args.prompt, Some("Run tests".to_string()));
                assert_eq!(args.model, "sonnet");
                assert_eq!(args.max_iterations, 30);
                assert_eq!(args.budget, 1.0);
                assert!(!args.yolo);
            }
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn test_cli_watch_with_patterns() {
        let cli = Cli::try_parse_from([
            "doodoori", "watch",
            "-p", "src/**/*.rs",
            "-p", "tests/**/*.rs",
            "Run tests"
        ]).unwrap();

        match cli.command {
            Commands::Watch(args) => {
                assert_eq!(args.patterns, vec!["src/**/*.rs", "tests/**/*.rs"]);
            }
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn test_cli_watch_with_spec() {
        let cli = Cli::try_parse_from([
            "doodoori", "watch",
            "--spec", "task.md"
        ]).unwrap();

        match cli.command {
            Commands::Watch(args) => {
                assert_eq!(args.spec, Some("task.md".to_string()));
            }
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn test_cli_watch_with_options() {
        let cli = Cli::try_parse_from([
            "doodoori", "watch",
            "--model", "opus",
            "--max-iterations", "50",
            "--budget", "5.0",
            "--debounce", "1000",
            "--clear",
            "--run-initial",
            "--yolo",
            "Complex task"
        ]).unwrap();

        match cli.command {
            Commands::Watch(args) => {
                assert_eq!(args.model, "opus");
                assert_eq!(args.max_iterations, 50);
                assert_eq!(args.budget, 5.0);
                assert_eq!(args.debounce, 1000);
                assert!(args.clear);
                assert!(args.run_initial);
                assert!(args.yolo);
            }
            _ => panic!("Expected Watch command"),
        }
    }

    #[test]
    fn test_cli_sandbox_login() {
        let cli = Cli::try_parse_from(["doodoori", "sandbox", "login"]).unwrap();

        match cli.command {
            Commands::Sandbox(args) => {
                match args.command {
                    commands::sandbox::SandboxCommand::Login(login_args) => {
                        assert_eq!(login_args.image, "doodoori/sandbox:latest");
                        assert_eq!(login_args.volume, "doodoori-claude-credentials");
                    }
                    _ => panic!("Expected Login subcommand"),
                }
            }
            _ => panic!("Expected Sandbox command"),
        }
    }

    #[test]
    fn test_cli_sandbox_login_custom() {
        let cli = Cli::try_parse_from([
            "doodoori", "sandbox", "login",
            "--image", "custom:v1",
            "--volume", "my-volume"
        ]).unwrap();

        match cli.command {
            Commands::Sandbox(args) => {
                match args.command {
                    commands::sandbox::SandboxCommand::Login(login_args) => {
                        assert_eq!(login_args.image, "custom:v1");
                        assert_eq!(login_args.volume, "my-volume");
                    }
                    _ => panic!("Expected Login subcommand"),
                }
            }
            _ => panic!("Expected Sandbox command"),
        }
    }

    #[test]
    fn test_cli_sandbox_status() {
        let cli = Cli::try_parse_from(["doodoori", "sandbox", "status"]).unwrap();

        match cli.command {
            Commands::Sandbox(args) => {
                matches!(args.command, commands::sandbox::SandboxCommand::Status);
            }
            _ => panic!("Expected Sandbox command"),
        }
    }

    #[test]
    fn test_cli_sandbox_cleanup_all() {
        let cli = Cli::try_parse_from(["doodoori", "sandbox", "cleanup", "--all"]).unwrap();

        match cli.command {
            Commands::Sandbox(args) => {
                match args.command {
                    commands::sandbox::SandboxCommand::Cleanup(cleanup_args) => {
                        assert!(cleanup_args.all);
                    }
                    _ => panic!("Expected Cleanup subcommand"),
                }
            }
            _ => panic!("Expected Sandbox command"),
        }
    }

    #[test]
    fn test_cli_template_list() {
        let cli = Cli::try_parse_from(["doodoori", "template", "list"]).unwrap();

        match cli.command {
            Commands::Template { command } => {
                matches!(command, TemplateCommand::List(_));
            }
            _ => panic!("Expected Template command"),
        }
    }

    #[test]
    fn test_cli_template_show() {
        let cli = Cli::try_parse_from(["doodoori", "template", "show", "api-endpoint"]).unwrap();

        match cli.command {
            Commands::Template { command } => {
                match command {
                    TemplateCommand::Show(args) => {
                        assert_eq!(args.name, "api-endpoint");
                    }
                    _ => panic!("Expected Show subcommand"),
                }
            }
            _ => panic!("Expected Template command"),
        }
    }

    #[test]
    fn test_cli_template_use() {
        let cli = Cli::try_parse_from([
            "doodoori", "template", "use", "api-endpoint",
            "--var", "name=users"
        ]).unwrap();

        match cli.command {
            Commands::Template { command } => {
                match command {
                    TemplateCommand::Use(args) => {
                        assert_eq!(args.name, "api-endpoint");
                        assert_eq!(args.var, vec!["name=users"]);
                    }
                    _ => panic!("Expected Use subcommand"),
                }
            }
            _ => panic!("Expected Template command"),
        }
    }

    #[test]
    fn test_cli_template_create() {
        let cli = Cli::try_parse_from(["doodoori", "template", "create", "my-template"]).unwrap();

        match cli.command {
            Commands::Template { command } => {
                match command {
                    TemplateCommand::Create(args) => {
                        assert_eq!(args.name, "my-template");
                    }
                    _ => panic!("Expected Create subcommand"),
                }
            }
            _ => panic!("Expected Template command"),
        }
    }

    #[test]
    fn test_cli_template_delete() {
        let cli = Cli::try_parse_from(["doodoori", "template", "delete", "my-template"]).unwrap();

        match cli.command {
            Commands::Template { command } => {
                match command {
                    TemplateCommand::Delete(args) => {
                        assert_eq!(args.name, "my-template");
                        assert!(!args.force);
                    }
                    _ => panic!("Expected Delete subcommand"),
                }
            }
            _ => panic!("Expected Template command"),
        }
    }
}
