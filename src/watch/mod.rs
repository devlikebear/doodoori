#![allow(dead_code)]

use anyhow::{Context, Result};
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Events emitted by the file watcher
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// Files changed
    FilesChanged {
        paths: Vec<PathBuf>,
    },
    /// Watch started
    Started {
        patterns: Vec<String>,
        base_dir: PathBuf,
    },
    /// Watch stopped
    Stopped,
    /// Error occurred
    Error(String),
}

/// Configuration for file watching
#[derive(Debug, Clone)]
pub struct WatchConfig {
    /// Glob patterns to watch (e.g., "src/**/*.rs")
    pub patterns: Vec<String>,
    /// Base directory to watch
    pub base_dir: PathBuf,
    /// Debounce duration in milliseconds
    pub debounce_ms: u64,
    /// Whether to watch recursively
    pub recursive: bool,
    /// Ignore patterns (e.g., "target/**", ".git/**")
    pub ignore_patterns: Vec<String>,
    /// Clear screen before each run
    pub clear_screen: bool,
    /// Run once immediately on start
    pub run_initial: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            patterns: vec!["**/*".to_string()],
            base_dir: PathBuf::from("."),
            debounce_ms: 500,
            recursive: true,
            ignore_patterns: vec![
                "target/**".to_string(),
                ".git/**".to_string(),
                ".doodoori/**".to_string(),
                "node_modules/**".to_string(),
                "*.log".to_string(),
            ],
            clear_screen: false,
            run_initial: false,
        }
    }
}

impl WatchConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = patterns;
        self
    }

    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = dir.into();
        self
    }

    pub fn with_debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    pub fn with_ignore(mut self, patterns: Vec<String>) -> Self {
        self.ignore_patterns = patterns;
        self
    }

    pub fn with_clear_screen(mut self, clear: bool) -> Self {
        self.clear_screen = clear;
        self
    }

    pub fn with_run_initial(mut self, run: bool) -> Self {
        self.run_initial = run;
        self
    }
}

/// File watcher that monitors file system changes
pub struct FileWatcher {
    config: WatchConfig,
    event_tx: broadcast::Sender<WatchEvent>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(config: WatchConfig) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self { config, event_tx }
    }

    /// Subscribe to watch events
    pub fn subscribe(&self) -> broadcast::Receiver<WatchEvent> {
        self.event_tx.subscribe()
    }

    /// Check if a path matches the watch patterns
    fn matches_pattern(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check ignore patterns first
        for ignore in &self.config.ignore_patterns {
            if let Ok(pattern) = glob::Pattern::new(ignore) {
                if pattern.matches(&path_str) {
                    return false;
                }
            }
        }

        // Check if matches any watch pattern
        for pattern in &self.config.patterns {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                if glob_pattern.matches(&path_str) {
                    return true;
                }
            }
        }

        // If no patterns specified, match all
        self.config.patterns.is_empty()
    }

    /// Start watching for file changes
    pub async fn watch(&self) -> Result<()> {
        let (tx, rx) = mpsc::channel();

        // Create debouncer
        let debounce_duration = Duration::from_millis(self.config.debounce_ms);
        let mut debouncer = new_debouncer(debounce_duration, tx)
            .context("Failed to create file watcher")?;

        // Watch the base directory
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        debouncer
            .watcher()
            .watch(&self.config.base_dir, mode)
            .with_context(|| format!("Failed to watch directory: {}", self.config.base_dir.display()))?;

        // Send started event
        let _ = self.event_tx.send(WatchEvent::Started {
            patterns: self.config.patterns.clone(),
            base_dir: self.config.base_dir.clone(),
        });

        tracing::info!(
            "Watching {} for changes (patterns: {:?})",
            self.config.base_dir.display(),
            self.config.patterns
        );

        // Process events
        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    let changed_paths: Vec<PathBuf> = events
                        .into_iter()
                        .filter(|e| matches!(e.kind, DebouncedEventKind::Any))
                        .map(|e| e.path)
                        .filter(|p| self.matches_pattern(p))
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect();

                    if !changed_paths.is_empty() {
                        tracing::debug!("Files changed: {:?}", changed_paths);
                        let _ = self.event_tx.send(WatchEvent::FilesChanged {
                            paths: changed_paths,
                        });
                    }
                }
                Ok(Err(error)) => {
                    tracing::error!("Watch error: {:?}", error);
                    let _ = self.event_tx.send(WatchEvent::Error(format!("{:?}", error)));
                }
                Err(e) => {
                    tracing::error!("Channel error: {}", e);
                    let _ = self.event_tx.send(WatchEvent::Error(e.to_string()));
                    break;
                }
            }
        }

        let _ = self.event_tx.send(WatchEvent::Stopped);
        Ok(())
    }
}

/// Watch mode runner that executes tasks on file changes
pub struct WatchRunner {
    config: WatchConfig,
    task_prompt: String,
    task_config: WatchTaskConfig,
}

/// Configuration for watch task execution
#[derive(Debug, Clone)]
pub struct WatchTaskConfig {
    /// Model to use
    pub model: String,
    /// Maximum iterations per run
    pub max_iterations: u32,
    /// Budget limit per run
    pub budget_limit: Option<f64>,
    /// YOLO mode
    pub yolo_mode: bool,
    /// Read-only mode
    pub readonly: bool,
    /// Spec file to use (alternative to prompt)
    pub spec_file: Option<PathBuf>,
}

impl Default for WatchTaskConfig {
    fn default() -> Self {
        Self {
            model: "sonnet".to_string(),
            max_iterations: 30,
            budget_limit: Some(1.0), // Default $1 per run for safety
            yolo_mode: false,
            readonly: false,
            spec_file: None,
        }
    }
}

impl WatchRunner {
    /// Create a new watch runner
    pub fn new(config: WatchConfig, task_prompt: String, task_config: WatchTaskConfig) -> Self {
        Self {
            config,
            task_prompt,
            task_config,
        }
    }

    /// Run the watch loop
    pub async fn run(&self) -> Result<()> {
        
        
        use console::{style, Emoji};

        let watcher = FileWatcher::new(self.config.clone());
        let mut event_rx = watcher.subscribe();

        // Spawn watcher in background
        let watcher_handle = tokio::spawn(async move {
            watcher.watch().await
        });

        println!(
            "{} Watch mode started",
            Emoji("👁️ ", "[WATCH]")
        );
        println!("  Patterns: {:?}", self.config.patterns);
        println!("  Directory: {}", self.config.base_dir.display());
        println!("  Task: {}", if self.task_prompt.len() > 50 {
            format!("{}...", &self.task_prompt[..47])
        } else {
            self.task_prompt.clone()
        });
        println!();
        println!("{} Waiting for file changes... (Ctrl+C to stop)", Emoji("⏳", "[WAIT]"));

        // Run initial if configured
        if self.config.run_initial {
            println!();
            println!("{} Running initial task...", Emoji("🚀", "[RUN]"));
            self.execute_task().await?;
        }

        let mut run_count = 0u32;

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Ok(WatchEvent::FilesChanged { paths }) => {
                            run_count += 1;

                            if self.config.clear_screen {
                                print!("\x1B[2J\x1B[1;1H"); // Clear screen
                            }

                            println!();
                            println!(
                                "{} Files changed (run #{})",
                                Emoji("📝", "[CHANGE]"),
                                run_count
                            );
                            for path in &paths {
                                println!("  - {}", path.display());
                            }
                            println!();

                            // Execute the task
                            match self.execute_task().await {
                                Ok(_) => {
                                    println!();
                                    println!(
                                        "{} Task completed. Waiting for changes...",
                                        Emoji("✅", "[OK]")
                                    );
                                }
                                Err(e) => {
                                    println!();
                                    println!(
                                        "{} Task failed: {}",
                                        Emoji("❌", "[ERR]"),
                                        style(e.to_string()).red()
                                    );
                                    println!("Waiting for changes...");
                                }
                            }
                        }
                        Ok(WatchEvent::Error(e)) => {
                            println!(
                                "{} Watch error: {}",
                                Emoji("⚠️", "[WARN]"),
                                style(e).yellow()
                            );
                        }
                        Ok(WatchEvent::Stopped) => {
                            println!("{} Watch stopped", Emoji("🛑", "[STOP]"));
                            break;
                        }
                        Ok(WatchEvent::Started { .. }) => {
                            // Already printed above
                        }
                        Err(_) => {
                            // Channel closed
                            break;
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!();
                    println!("{} Stopping watch mode...", Emoji("🛑", "[STOP]"));
                    break;
                }
            }
        }

        // Cleanup
        watcher_handle.abort();

        println!();
        println!(
            "{} Watch mode ended. Total runs: {}",
            Emoji("📊", "[STATS]"),
            run_count
        );

        Ok(())
    }

    /// Execute the task
    async fn execute_task(&self) -> Result<()> {
        use crate::claude::ModelAlias;
        use crate::loop_engine::{LoopConfig, LoopEngine, LoopEvent};
        use console::Emoji;
        use indicatif::{ProgressBar, ProgressStyle};

        // Get prompt from spec file or use direct prompt
        let prompt = if let Some(ref spec_path) = self.task_config.spec_file {
            let spec = crate::instructions::SpecParser::parse_file(spec_path)?;
            spec.to_prompt()
        } else {
            self.task_prompt.clone()
        };

        // Parse model
        let model: ModelAlias = self.task_config.model.parse().unwrap_or(ModelAlias::Sonnet);

        // Build loop config
        let loop_config = LoopConfig {
            max_iterations: self.task_config.max_iterations,
            budget_limit: self.task_config.budget_limit,
            model,
            working_dir: Some(self.config.base_dir.clone()),
            yolo_mode: self.task_config.yolo_mode,
            readonly: self.task_config.readonly,
            ..Default::default()
        };

        let engine = LoopEngine::new(loop_config);

        // Create progress bar
        let progress = ProgressBar::new(self.task_config.max_iterations as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({msg})")
                .unwrap()
                .progress_chars("█▓░"),
        );

        // Execute
        let (mut rx, handle) = engine.execute(&prompt).await?;

        let mut total_cost = 0.0f64;

        while let Some(event) = rx.recv().await {
            match event {
                LoopEvent::IterationStarted { iteration } => {
                    progress.set_position(iteration as u64);
                    progress.set_message(format!("${:.4}", total_cost));
                }
                LoopEvent::IterationCompleted { iteration, usage, completed } => {
                    total_cost = usage.total_cost_usd;
                    progress.set_position((iteration + 1) as u64);
                    progress.set_message(format!("${:.4}", total_cost));

                    if completed {
                        progress.finish_with_message(format!("${:.4} - Done!", total_cost));
                    }
                }
                LoopEvent::LoopFinished { status, total_iterations, total_usage } => {
                    progress.finish_and_clear();
                    println!(
                        "  {} Iterations: {}, Cost: ${:.4}",
                        match status {
                            crate::loop_engine::LoopStatus::Completed => Emoji("✅", "[OK]"),
                            crate::loop_engine::LoopStatus::MaxIterationsReached => Emoji("⚠️", "[!]"),
                            crate::loop_engine::LoopStatus::BudgetExceeded => Emoji("💸", "[$]"),
                            _ => Emoji("❌", "[ERR]"),
                        },
                        total_iterations,
                        total_usage.total_cost_usd
                    );
                }
                _ => {}
            }
        }

        handle.await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_default() {
        let config = WatchConfig::default();
        assert_eq!(config.debounce_ms, 500);
        assert!(config.recursive);
        assert!(!config.clear_screen);
        assert!(!config.run_initial);
    }

    #[test]
    fn test_watch_config_builder() {
        let config = WatchConfig::new()
            .with_patterns(vec!["*.rs".to_string()])
            .with_base_dir("/tmp")
            .with_debounce(1000)
            .with_clear_screen(true)
            .with_run_initial(true);

        assert_eq!(config.patterns, vec!["*.rs".to_string()]);
        assert_eq!(config.base_dir, PathBuf::from("/tmp"));
        assert_eq!(config.debounce_ms, 1000);
        assert!(config.clear_screen);
        assert!(config.run_initial);
    }

    #[test]
    fn test_matches_pattern() {
        let config = WatchConfig::new()
            .with_patterns(vec!["src/**/*.rs".to_string()])
            .with_ignore(vec!["target/**".to_string()]);

        let watcher = FileWatcher::new(config);

        // These should match
        assert!(watcher.matches_pattern(Path::new("src/main.rs")));
        assert!(watcher.matches_pattern(Path::new("src/lib/mod.rs")));

        // These should not match (ignored)
        assert!(!watcher.matches_pattern(Path::new("target/debug/main")));

        // These should not match (different pattern)
        assert!(!watcher.matches_pattern(Path::new("README.md")));
    }

    #[test]
    fn test_watch_task_config_default() {
        let config = WatchTaskConfig::default();
        assert_eq!(config.model, "sonnet");
        assert_eq!(config.max_iterations, 30);
        assert_eq!(config.budget_limit, Some(1.0));
        assert!(!config.yolo_mode);
        assert!(!config.readonly);
    }
}
