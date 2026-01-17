use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::watch::{WatchConfig, WatchRunner, WatchTaskConfig};

/// Watch for file changes and run tasks automatically
#[derive(Args, Debug)]
pub struct WatchArgs {
    /// The task prompt to run on file changes
    #[arg(required_unless_present = "spec")]
    pub prompt: Option<String>,

    /// Spec file to use instead of prompt
    #[arg(short, long)]
    pub spec: Option<String>,

    /// Glob patterns to watch (e.g., "src/**/*.rs")
    /// Can be specified multiple times
    #[arg(short, long = "pattern", default_value = "**/*")]
    pub patterns: Vec<String>,

    /// Directory to watch
    #[arg(short, long, default_value = ".")]
    pub dir: PathBuf,

    /// Patterns to ignore (e.g., "target/**")
    /// Can be specified multiple times
    #[arg(short, long = "ignore")]
    pub ignore: Vec<String>,

    /// Debounce duration in milliseconds
    #[arg(long, default_value = "500")]
    pub debounce: u64,

    /// Model to use (haiku, sonnet, opus)
    #[arg(short, long, default_value = "sonnet")]
    pub model: String,

    /// Maximum iterations per run
    #[arg(long, default_value = "30")]
    pub max_iterations: u32,

    /// Budget limit per run in USD
    #[arg(long, default_value = "1.0")]
    pub budget: f64,

    /// Skip all permission prompts (DANGEROUS)
    #[arg(long)]
    pub yolo: bool,

    /// Read-only mode - no file modifications
    #[arg(long)]
    pub readonly: bool,

    /// Clear screen before each run
    #[arg(long)]
    pub clear: bool,

    /// Run the task once immediately on start
    #[arg(long)]
    pub run_initial: bool,

    /// Watch non-recursively (only top-level directory)
    #[arg(long)]
    pub no_recursive: bool,
}

impl WatchArgs {
    pub async fn execute(self) -> Result<()> {
        // Build ignore patterns
        let mut ignore_patterns = vec![
            "target/**".to_string(),
            ".git/**".to_string(),
            ".doodoori/**".to_string(),
            "node_modules/**".to_string(),
            "*.log".to_string(),
        ];
        ignore_patterns.extend(self.ignore);

        // Build watch config
        let watch_config = WatchConfig::new()
            .with_patterns(self.patterns)
            .with_base_dir(&self.dir)
            .with_debounce(self.debounce)
            .with_ignore(ignore_patterns)
            .with_clear_screen(self.clear)
            .with_run_initial(self.run_initial);

        // Handle recursive option
        let watch_config = if self.no_recursive {
            WatchConfig {
                recursive: false,
                ..watch_config
            }
        } else {
            watch_config
        };

        // Build task config
        let task_config = WatchTaskConfig {
            model: self.model,
            max_iterations: self.max_iterations,
            budget_limit: Some(self.budget),
            yolo_mode: self.yolo,
            readonly: self.readonly,
            spec_file: self.spec.map(PathBuf::from),
        };

        // Get prompt
        let prompt = self.prompt.unwrap_or_else(|| {
            "Execute the task defined in the spec file".to_string()
        });

        // Create and run watch runner
        let runner = WatchRunner::new(watch_config, prompt, task_config);
        runner.run().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        watch: WatchArgs,
    }

    #[test]
    fn test_watch_args_basic() {
        let cli = TestCli::parse_from([
            "test",
            "Run tests",
        ]);
        assert_eq!(cli.watch.prompt, Some("Run tests".to_string()));
        assert_eq!(cli.watch.model, "sonnet");
        assert_eq!(cli.watch.max_iterations, 30);
    }

    #[test]
    fn test_watch_args_with_patterns() {
        let cli = TestCli::parse_from([
            "test",
            "-p", "src/**/*.rs",
            "-p", "tests/**/*.rs",
            "Run tests",
        ]);
        assert_eq!(cli.watch.patterns, vec!["src/**/*.rs", "tests/**/*.rs"]);
    }

    #[test]
    fn test_watch_args_with_spec() {
        let cli = TestCli::parse_from([
            "test",
            "--spec", "task.md",
        ]);
        assert_eq!(cli.watch.spec, Some("task.md".to_string()));
        assert!(cli.watch.prompt.is_none());
    }

    #[test]
    fn test_watch_args_with_options() {
        let cli = TestCli::parse_from([
            "test",
            "--model", "opus",
            "--max-iterations", "50",
            "--budget", "5.0",
            "--debounce", "1000",
            "--clear",
            "--run-initial",
            "--yolo",
            "Complex task",
        ]);
        assert_eq!(cli.watch.model, "opus");
        assert_eq!(cli.watch.max_iterations, 50);
        assert_eq!(cli.watch.budget, 5.0);
        assert_eq!(cli.watch.debounce, 1000);
        assert!(cli.watch.clear);
        assert!(cli.watch.run_initial);
        assert!(cli.watch.yolo);
    }

    #[test]
    fn test_watch_args_with_ignore() {
        let cli = TestCli::parse_from([
            "test",
            "-i", "*.tmp",
            "-i", "build/**",
            "Task",
        ]);
        assert_eq!(cli.watch.ignore, vec!["*.tmp", "build/**"]);
    }
}
