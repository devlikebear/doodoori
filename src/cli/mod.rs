pub mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::{
    cost::CostArgs, parallel::ParallelArgs, resume::ResumeArgs, run::RunArgs,
    sandbox::SandboxArgs, secret::SecretArgs, spec::SpecArgs,
};

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
            Commands::Config => {
                println!("Config file: {}", self.config);
                // TODO: Load and display config
                Ok(())
            }
            Commands::Price { update, model } => {
                if update {
                    println!("Updating prices...");
                    // TODO: Implement price update
                }
                if let Some(m) = model {
                    println!("Showing price for model: {}", m);
                    // TODO: Show model price
                } else {
                    println!("Showing all prices...");
                    // TODO: Show all prices
                }
                Ok(())
            }
        }
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
}
