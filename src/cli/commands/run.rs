use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::claude::{ClaudeEvent, ModelAlias};
use crate::instructions::SpecParser;
use crate::loop_engine::{LoopConfig, LoopEngine, LoopEvent, LoopStatus};
use crate::notifications::{NotificationManager, NotificationsConfig};

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

    /// Show verbose output (Claude events, tool calls)
    #[arg(long)]
    pub verbose: bool,

    /// Disable hooks execution
    #[arg(long)]
    pub no_hooks: bool,

    /// Send notifications (Slack/Discord/Webhook URL)
    /// If URL is provided, notifications will be sent to that endpoint
    /// Otherwise, uses doodoori.toml configuration
    #[arg(long)]
    pub notify: Option<Option<String>>,

    /// Disable notifications
    #[arg(long)]
    pub no_notify: bool,
}

impl RunArgs {
    pub async fn execute(self) -> Result<()> {
        if self.dry_run {
            return self.execute_dry_run().await;
        }

        // Load prompt from spec file or use direct prompt
        let (prompt, spec_model, spec_max_iterations) = if let Some(spec_path) = &self.spec {
            tracing::info!("Loading spec file: {}", spec_path);
            let spec = SpecParser::parse_file(std::path::Path::new(spec_path))?;
            let prompt = spec.to_prompt();
            let model = spec.effective_model();
            let max_iter = spec.max_iterations;
            (prompt, Some(model), max_iter)
        } else {
            (self.prompt.clone().unwrap(), None, None)
        };

        // Use spec values as defaults, CLI overrides take precedence
        // If CLI model is default (sonnet) and spec has a model, use spec's model
        let model = if self.model == ModelAlias::Sonnet && spec_model.is_some() {
            spec_model.unwrap()
        } else {
            self.model.clone()
        };
        let max_iterations = spec_max_iterations.unwrap_or(self.max_iterations);

        tracing::info!("Running task with model: {:?}", model);
        tracing::info!("Prompt: {}", prompt);
        tracing::info!("Max iterations: {}", max_iterations);

        if let Some(budget) = self.budget {
            tracing::info!("Budget limit: ${:.2}", budget);
        }

        if self.sandbox {
            return self.execute_sandbox(&prompt).await;
        }

        if self.yolo {
            tracing::warn!("YOLO mode enabled - all permissions granted!");
        }

        // Execute with Loop Engine
        self.execute_loop_engine(&prompt, model, max_iterations).await
    }

    /// Execute task with the Loop Engine
    async fn execute_loop_engine(
        &self,
        prompt: &str,
        model: ModelAlias,
        max_iterations: u32,
    ) -> Result<()> {
        use console::{style, Emoji};
        use indicatif::{ProgressBar, ProgressStyle};

        println!("{} Doodoori is forging your code...", Emoji("🔨", ""));
        println!();
        println!("  Task:       {}", if prompt.len() > 60 { format!("{}...", &prompt[..57]) } else { prompt.to_string() });
        println!("  Model:      {:?}", model);
        println!("  Max Iter:   {}", max_iterations);
        if let Some(budget) = self.budget {
            println!("  Budget:     ${:.2}", budget);
        }
        if self.yolo {
            println!("  Mode:       {} YOLO", style("⚠").yellow());
        }
        println!();

        // Build loop configuration
        let working_dir = std::env::current_dir().ok();
        let system_prompt = if self.no_instructions {
            None
        } else {
            self.instructions.as_ref().map(PathBuf::from).or_else(|| {
                // Check for doodoori.md in current directory
                let default_path = PathBuf::from("doodoori.md");
                if default_path.exists() {
                    Some(default_path)
                } else {
                    None
                }
            })
        };

        // Load hooks configuration from doodoori.toml if available
        let doodoori_config = crate::config::DoodooriConfig::load().unwrap_or_default();
        let hooks_config = doodoori_config.hooks.to_hooks_config();

        // Load notifications configuration
        let notifications_config = if let Some(ref notify_arg) = self.notify {
            // --notify flag was provided
            match notify_arg {
                Some(url) => {
                    // URL was provided: --notify <url>
                    tracing::info!("Using notification URL: {}", url);
                    NotificationManager::from_url(url)
                        .map(|_| {
                            // Create config from URL
                            let mut config = NotificationsConfig::default();
                            config.enabled = true;
                            if url.contains("hooks.slack.com") {
                                config.slack = Some(crate::notifications::SlackConfig {
                                    webhook_url: url.clone(),
                                    channel: None,
                                    username: None,
                                    icon_emoji: None,
                                    events: vec![
                                        crate::notifications::NotificationEvent::Completed,
                                        crate::notifications::NotificationEvent::Error,
                                    ],
                                });
                            } else if url.contains("discord.com/api/webhooks") {
                                config.discord = Some(crate::notifications::DiscordConfig {
                                    webhook_url: url.clone(),
                                    username: None,
                                    avatar_url: None,
                                    events: vec![
                                        crate::notifications::NotificationEvent::Completed,
                                        crate::notifications::NotificationEvent::Error,
                                    ],
                                });
                            } else {
                                config.webhooks.push(crate::notifications::WebhookConfig {
                                    url: url.clone(),
                                    method: "POST".to_string(),
                                    headers: std::collections::HashMap::new(),
                                    events: vec![
                                        crate::notifications::NotificationEvent::Completed,
                                        crate::notifications::NotificationEvent::Error,
                                    ],
                                    timeout_secs: 30,
                                });
                            }
                            config
                        })
                        .unwrap_or_else(|e| {
                            tracing::warn!("Invalid notification URL: {}", e);
                            NotificationsConfig::default()
                        })
                }
                None => {
                    // No URL provided: --notify (enable from config)
                    let mut config = doodoori_config.notifications.to_notifications_config();
                    config.enabled = true;
                    config
                }
            }
        } else {
            // No --notify flag, use config file settings
            doodoori_config.notifications.to_notifications_config()
        };

        let loop_config = LoopConfig {
            max_iterations,
            budget_limit: self.budget,
            model,
            working_dir: working_dir.clone(),
            yolo_mode: self.yolo,
            readonly: self.readonly,
            system_prompt,
            allowed_tools: self.allow.clone(),
            enable_state: true,
            enable_cost_tracking: true,
            project_dir: working_dir,
            hooks: hooks_config,
            disable_hooks: self.no_hooks,
            notifications: notifications_config,
            disable_notifications: self.no_notify,
            ..Default::default()
        };

        let engine = LoopEngine::new(loop_config);

        // Create progress bar
        let progress = ProgressBar::new(max_iterations as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} iterations ({msg})")
                .unwrap()
                .progress_chars("█▓░"),
        );

        // Execute with event handling
        let (mut rx, handle) = engine.execute(prompt).await?;

        let mut total_cost = 0.0f64;

        while let Some(event) = rx.recv().await {
            match event {
                LoopEvent::IterationStarted { iteration } => {
                    progress.set_position(iteration as u64);
                    progress.set_message(format!("${:.4}", total_cost));
                }
                LoopEvent::ClaudeEvent(claude_event) => {
                    if self.verbose {
                        self.print_claude_event(&claude_event);
                    } else {
                        tracing::debug!("Claude event: {:?}", claude_event);
                    }
                }
                LoopEvent::IterationCompleted { iteration, usage, completed } => {
                    total_cost = usage.total_cost_usd;
                    progress.set_position((iteration + 1) as u64);
                    progress.set_message(format!("${:.4}", total_cost));

                    if completed {
                        progress.finish_with_message(format!("${:.4} - Completed!", total_cost));
                    }
                }
                LoopEvent::HookExecuted { hook_type, success, duration_ms } => {
                    if self.verbose {
                        let status_icon = if success { "✓" } else { "✗" };
                        println!("  {} Hook {}: {} ({}ms)",
                            status_icon,
                            hook_type,
                            if success { "ok" } else { "failed" },
                            duration_ms
                        );
                    }
                }
                LoopEvent::LoopFinished { status, total_iterations, total_usage } => {
                    progress.finish_and_clear();
                    println!();
                    self.print_result(&status, total_iterations, &total_usage);
                }
            }
        }

        // Wait for the loop to finish
        let result = handle.await??;

        // Print final status if not already printed
        if result.status != LoopStatus::Completed {
            println!();
            self.print_result(&result.status, result.iterations, &result.total_usage);
        }

        Ok(())
    }

    /// Print the final result
    fn print_result(
        &self,
        status: &LoopStatus,
        iterations: u32,
        usage: &crate::claude::ExecutionUsage,
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
        println!("  Iterations:    {}", iterations);
        println!("  Input tokens:  {}", usage.input_tokens);
        println!("  Output tokens: {}", usage.output_tokens);
        println!("  Total cost:    ${:.4}", usage.total_cost_usd);
        println!("  Duration:      {:.2}s", usage.duration_ms as f64 / 1000.0);
    }

    /// Print a Claude event in verbose mode
    fn print_claude_event(&self, event: &ClaudeEvent) {
        use console::{style, Emoji};

        match event {
            ClaudeEvent::System(sys) => {
                if let Some(ref session_id) = sys.session_id {
                    println!("{} Session: {} ({})",
                        Emoji("📋", "[SYS]"),
                        style(session_id).cyan(),
                        sys.subtype
                    );
                }
            }
            ClaudeEvent::Assistant(asst) => {
                if let Some(ref msg) = asst.message {
                    // Truncate long messages
                    let display_msg = if msg.len() > 200 {
                        format!("{}...", &msg[..197])
                    } else {
                        msg.clone()
                    };
                    println!("{} {}", Emoji("🤖", "[AI]"), style(display_msg).dim());
                }
            }
            ClaudeEvent::ToolUse(tool) => {
                println!("{} {}",
                    Emoji("🔧", "[TOOL]"),
                    style(&tool.tool_name).yellow().bold()
                );
            }
            ClaudeEvent::ToolResult(result) => {
                let status = if result.is_error {
                    style("✗ error").red()
                } else {
                    style("✓ ok").green()
                };
                println!("   └─ {} ({})", status, result.tool_name);
            }
            ClaudeEvent::Result(res) => {
                let duration = res.duration_ms.unwrap_or(0);
                let (input_tok, output_tok) = res.usage.as_ref()
                    .map(|u| (u.input_tokens, u.output_tokens))
                    .unwrap_or((0, 0));
                println!("{} Result: {} ({}ms, {}in/{}out tokens)",
                    Emoji("📊", "[RES]"),
                    if res.is_error { style("error").red() } else { style("success").green() },
                    duration,
                    input_tok,
                    output_tok
                );
            }
            ClaudeEvent::Unknown => {
                // Ignore unknown events
            }
        }
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

        println!("\n[Hooks]");
        if self.no_hooks {
            println!("  Disabled");
        } else {
            let config = crate::config::DoodooriConfig::load().unwrap_or_default();
            if config.hooks.enabled {
                println!("  Enabled");
                if let Some(ref hook) = config.hooks.pre_run {
                    println!("  pre_run: {}", hook);
                }
                if let Some(ref hook) = config.hooks.post_run {
                    println!("  post_run: {}", hook);
                }
                if let Some(ref hook) = config.hooks.on_error {
                    println!("  on_error: {}", hook);
                }
                if let Some(ref hook) = config.hooks.on_iteration {
                    println!("  on_iteration: {}", hook);
                }
                if let Some(ref hook) = config.hooks.on_complete {
                    println!("  on_complete: {}", hook);
                }
                if config.hooks.pre_run.is_none() && config.hooks.post_run.is_none() {
                    println!("  (no hooks configured)");
                }
            } else {
                println!("  Disabled in config");
            }
        }

        println!("\n[Notifications]");
        if self.no_notify {
            println!("  Disabled");
        } else if let Some(ref notify_arg) = self.notify {
            match notify_arg {
                Some(url) => {
                    println!("  Enabled (URL: {})", url);
                }
                None => {
                    let config = crate::config::DoodooriConfig::load().unwrap_or_default();
                    let notif_config = config.notifications;
                    if notif_config.enabled {
                        println!("  Enabled (from config)");
                        if notif_config.slack_webhook.is_some() {
                            println!("  Slack: configured");
                        }
                        if notif_config.discord_webhook.is_some() {
                            println!("  Discord: configured");
                        }
                        if notif_config.webhook_url.is_some() {
                            println!("  Webhook: configured");
                        }
                    } else {
                        println!("  Disabled in config");
                    }
                }
            }
        } else {
            let config = crate::config::DoodooriConfig::load().unwrap_or_default();
            if config.notifications.enabled {
                println!("  Enabled (from config)");
                println!("  Events: {:?}", config.notifications.events);
            } else {
                println!("  Disabled");
            }
        }

        println!("\n=== End Preview ===");

        Ok(())
    }
}
