use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::claude::{ClaudeEvent, ModelAlias};
use crate::instructions::SpecParser;
use crate::loop_engine::{LoopConfig, LoopEngine, LoopEvent, LoopStatus};
use crate::notifications::{NotificationManager, NotificationsConfig};
use crate::output::{OutputFormat, OutputWriter, TaskOutput};

/// Truncate a string to a maximum number of characters (Unicode-safe)
fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        let truncated: String = chars[..max_chars.saturating_sub(3)].iter().collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Run a task with Claude Code
#[derive(Args, Debug)]
pub struct RunArgs {
    /// The task prompt or description
    #[arg(required_unless_present_any = ["spec", "template"])]
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

    /// Output format (text, json, json-pretty, yaml, markdown)
    #[arg(long, short = 'f', default_value = "text")]
    pub format: String,

    /// Output file path (default: stdout)
    #[arg(long, short = 'o')]
    pub output: Option<String>,

    /// Use a template instead of direct prompt
    #[arg(long, short = 't')]
    pub template: Option<String>,

    /// Template variables (key=value format)
    #[arg(long = "var")]
    pub template_vars: Vec<String>,

    /// Run task in background (detached mode)
    /// The task will continue running even if the terminal is closed
    #[arg(long, short = 'd')]
    pub detach: bool,

    /// Internal flag: indicates this is a detached worker process
    #[arg(long, hide = true)]
    pub internal_detached: bool,

    /// Run with TUI dashboard for real-time monitoring
    #[cfg(feature = "dashboard")]
    #[arg(long)]
    pub dashboard: bool,
}

impl RunArgs {
    /// Parse template variables from the --var flags
    /// Returns a HashMap of variable name to value
    fn parse_template_vars(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut vars = std::collections::HashMap::new();
        for var_str in &self.template_vars {
            let parts: Vec<&str> = var_str.splitn(2, '=').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid variable format: {}. Expected key=value", var_str);
            }
            vars.insert(parts[0].to_string(), parts[1].to_string());
        }
        Ok(vars)
    }

    pub async fn execute(self) -> Result<()> {
        // Handle detached mode - spawn background worker and exit
        if self.detach && !self.internal_detached {
            return self.spawn_detached().await;
        }

        if self.dry_run {
            return self.execute_dry_run().await;
        }

        // Load prompt from template, spec file, or use direct prompt
        let (prompt, spec_model, spec_max_iterations) = if let Some(template_name) = &self.template {
            // Load template
            use crate::templates::storage::TemplateStorage;

            tracing::info!("Loading template: {}", template_name);
            let storage = TemplateStorage::new()?;
            let template = storage.get(template_name)
                .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

            // Parse variables
            let vars = self.parse_template_vars()?;

            // Validate and render template
            template.validate_variables(&vars)?;
            let mut rendered = template.render(&vars)?;

            // Append additional prompt if provided
            if let Some(ref additional) = self.prompt {
                rendered = format!("{}\n\nAdditional instructions:\n{}", rendered, additional);
            }

            (rendered, template.default_model, template.default_max_iterations)
        } else if let Some(spec_path) = &self.spec {
            tracing::info!("Loading spec file: {}", spec_path);
            let spec = SpecParser::parse_file(std::path::Path::new(spec_path))?;
            let prompt = spec.to_prompt();
            let model = spec.effective_model();
            let max_iter = spec.max_iterations;
            (prompt, Some(model), max_iter)
        } else {
            (self.prompt.clone().unwrap(), None, None)
        };

        // Use spec/template values as defaults, CLI overrides take precedence
        // If CLI model is default (sonnet) and spec/template has a model, use spec/template's model
        let model = if self.model == ModelAlias::Sonnet && spec_model.is_some() {
            spec_model.unwrap()
        } else {
            self.model.clone()
        };
        // If CLI max_iterations is default (50) and spec/template has max_iterations, use spec/template's value
        let max_iterations = if self.max_iterations == 50 && spec_max_iterations.is_some() {
            spec_max_iterations.unwrap()
        } else {
            self.max_iterations
        };

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

        // Execute with Dashboard TUI if --dashboard flag is set
        #[cfg(feature = "dashboard")]
        if self.dashboard {
            return self.execute_with_dashboard(&prompt, model, max_iterations).await;
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
        println!("  Task:       {}", truncate_str(prompt, 60));
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
        let model_name = format!("{:?}", model); // Save for output before move
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

        // Generate task ID for output
        let result_task_id = uuid::Uuid::new_v4().to_string();

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

                    // Parse output format
                    let output_format: OutputFormat = self.format.parse().unwrap_or_default();

                    // If using text format, print the usual result
                    if output_format == OutputFormat::Text {
                        println!();
                        self.print_result(&status, total_iterations, &total_usage);
                    } else {
                        // Build TaskOutput for structured formats
                        let task_output = TaskOutput::new(
                            result_task_id.clone(),
                            prompt.to_string(),
                        )
                        .with_model(model_name.clone())
                        .with_status(format!("{:?}", status))
                        .with_iterations(total_iterations)
                        .with_cost(total_usage.total_cost_usd)
                        .with_duration(total_usage.duration_ms)
                        .with_tokens(total_usage.input_tokens, total_usage.output_tokens);

                        // Add error if present
                        let task_output = if let LoopStatus::Error(ref e) = status {
                            task_output.with_error(e)
                        } else {
                            task_output
                        };

                        // Write output
                        let writer = if let Some(ref path) = self.output {
                            OutputWriter::new(output_format).with_file(path)
                        } else {
                            OutputWriter::new(output_format)
                        };

                        if let Err(e) = writer.write_task(&task_output) {
                            tracing::error!("Failed to write output: {}", e);
                        }
                    }
                }
            }
        }

        // Wait for the loop to finish
        let result = handle.await??;

        // Print final status if not already printed (only for text format)
        let output_format: OutputFormat = self.format.parse().unwrap_or_default();
        if result.status != LoopStatus::Completed && output_format == OutputFormat::Text {
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
                    let text = msg.as_text();
                    let display_msg = if text.len() > 200 {
                        format!("{}...", &text[..197])
                    } else {
                        text
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

    /// Execute task with TUI dashboard for real-time monitoring
    #[cfg(feature = "dashboard")]
    async fn execute_with_dashboard(
        &self,
        prompt: &str,
        model: ModelAlias,
        max_iterations: u32,
    ) -> Result<()> {
        use crossterm::{
            event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
            execute,
            terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
        };
        use ratatui::{Terminal, backend::CrosstermBackend};
        use std::io;
        use std::time::{Duration, Instant};

        use crate::notifications::{NotificationManager, NotificationsConfig};

        // Build loop configuration (same as execute_loop_engine)
        let working_dir = std::env::current_dir().ok();
        let system_prompt = if self.no_instructions {
            None
        } else {
            self.instructions.as_ref().map(PathBuf::from).or_else(|| {
                let default_path = PathBuf::from("doodoori.md");
                if default_path.exists() {
                    Some(default_path)
                } else {
                    None
                }
            })
        };

        let doodoori_config = crate::config::DoodooriConfig::load().unwrap_or_default();
        let hooks_config = doodoori_config.hooks.to_hooks_config();

        let notifications_config = if let Some(ref notify_arg) = self.notify {
            match notify_arg {
                Some(url) => {
                    NotificationManager::from_url(url)
                        .map(|_| {
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
                    let mut config = doodoori_config.notifications.to_notifications_config();
                    config.enabled = true;
                    config
                }
            }
        } else {
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

        // Create engine
        let engine = LoopEngine::new(loop_config);

        // Execute with live monitoring - this returns (rx, handle)
        let (event_rx, execution_handle) = engine.execute_live(prompt).await?;

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create dashboard app with live monitoring
        let mut app = super::dashboard::tui::App::new(false);
        app.start_live_monitoring(event_rx, max_iterations);

        // Main TUI loop
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| super::dashboard::tui::ui(f, &app))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => {
                                app.should_quit = true;
                            }
                            KeyCode::Esc => {
                                if !app.live_monitoring_active {
                                    app.should_quit = true;
                                }
                                // If still monitoring, Esc just waits for completion
                            }
                            KeyCode::Up => app.scroll_live_up(),
                            KeyCode::Down => app.scroll_live_down(),
                            KeyCode::PageUp => app.scroll_live_page_up(),
                            KeyCode::PageDown => app.scroll_live_page_down(),
                            _ => {}
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                app.process_live_events();
                last_tick = Instant::now();
            }

            // Exit when done and user wants to quit, or if not monitoring anymore
            if app.should_quit || (!app.live_monitoring_active && !app.live_output_buffer.is_empty()) {
                // Wait a moment to show final state if just completed
                if !app.live_monitoring_active && !app.should_quit {
                    // Give user a chance to see the final result
                    app.process_live_events();
                    terminal.draw(|f| super::dashboard::tui::ui(f, &app))?;

                    // Wait for user to press a key to exit
                    loop {
                        if event::poll(Duration::from_millis(100))? {
                            if let Event::Key(key) = event::read()? {
                                if key.kind == KeyEventKind::Press {
                                    break;
                                }
                            }
                        }
                        app.process_live_events();
                        terminal.draw(|f| super::dashboard::tui::ui(f, &app))?;
                    }
                }
                break;
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        // Wait for execution to complete
        match execution_handle.await {
            Ok(Ok(result)) => {
                println!("\nExecution completed: {:?}", result.status);
                println!("Iterations: {}", result.iterations);
                println!("Cost: ${:.4}", result.total_usage.total_cost_usd);
            }
            Ok(Err(e)) => {
                eprintln!("\nExecution error: {}", e);
            }
            Err(e) => {
                eprintln!("\nTask panicked: {}", e);
            }
        }

        Ok(())
    }

    async fn execute_dry_run(&self) -> Result<()> {
        println!("=== Dry Run Preview ===\n");

        // Handle template, spec, or direct prompt
        let (prompt, template_model, template_max_iter, is_template) = if let Some(template_name) = &self.template {
            use crate::templates::storage::TemplateStorage;

            let storage = TemplateStorage::new()?;
            let template = storage.get(template_name)
                .ok_or_else(|| anyhow::anyhow!("Template not found: {}", template_name))?;

            let vars = self.parse_template_vars()?;
            template.validate_variables(&vars)?;
            let mut rendered = template.render(&vars)?;

            if let Some(ref additional) = self.prompt {
                rendered = format!("{}\n\nAdditional instructions:\n{}", rendered, additional);
            }

            println!("=== Template: {} ===", template_name);
            println!("Category: {:?}", template.category);
            if let Some(ref model) = template.default_model {
                println!("Default Model: {:?}", model);
            }
            if let Some(max_iter) = template.default_max_iterations {
                println!("Default Max Iterations: {}", max_iter);
            }
            println!();

            (rendered, template.default_model, template.default_max_iterations, true)
        } else if let Some(spec) = &self.spec {
            println!("[Prompt Source]");
            println!("  Spec file: {}", spec);
            let spec = SpecParser::parse_file(std::path::Path::new(spec))?;
            let prompt = spec.to_prompt();
            (prompt, Some(spec.effective_model()), spec.max_iterations, false)
        } else if let Some(prompt) = &self.prompt {
            println!("[Prompt Source]");
            println!("  Direct prompt");
            (prompt.clone(), None, None, false)
        } else {
            anyhow::bail!("Either --prompt, --spec, or --template is required");
        };

        if is_template {
            println!("=== Rendered Prompt ===");
        } else {
            println!("\n[Prompt]");
        }

        // Truncate long prompts for display
        if prompt.len() > 500 {
            println!("  \"{}...\"", &prompt[..497]);
            println!("  (truncated, {} total characters)", prompt.len());
        } else {
            println!("  \"{}\"", prompt);
        }

        println!("\n[Model]");
        let display_model = if self.model == ModelAlias::Sonnet && template_model.is_some() {
            template_model.as_ref().unwrap()
        } else {
            &self.model
        };
        println!("  {:?}", display_model);

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
        let display_max_iter = if self.max_iterations == 50 && template_max_iter.is_some() {
            template_max_iter.unwrap()
        } else {
            self.max_iterations
        };
        println!("  Max iterations: {}", display_max_iter);
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

    /// Spawn a detached background process to run the task
    async fn spawn_detached(&self) -> Result<()> {
        use anyhow::Context;
        use console::{style, Emoji};
        use std::process::{Command, Stdio};
        use uuid::Uuid;

        let task_id = Uuid::new_v4().to_string()[..8].to_string();

        println!(
            "\n{} {}",
            Emoji("🚀", ""),
            style("Starting detached task...").bold().cyan()
        );

        // Warn about auto-yolo in detached mode
        if !self.yolo && !self.readonly {
            println!(
                "  {} {}",
                Emoji("⚠️", "!"),
                style("Auto-enabling YOLO mode (required for background execution)").yellow()
            );
        }

        // Build the command arguments
        let mut args = vec!["run".to_string()];

        // Add prompt or spec
        if let Some(ref prompt) = self.prompt {
            args.push(prompt.clone());
        }
        if let Some(ref spec) = self.spec {
            args.push("--spec".to_string());
            args.push(spec.clone());
        }
        if let Some(ref template) = self.template {
            args.push("--template".to_string());
            args.push(template.clone());
        }

        // Add flags
        args.push("--model".to_string());
        args.push(self.model.to_string());
        args.push("--max-iterations".to_string());
        args.push(self.max_iterations.to_string());

        if let Some(budget) = self.budget {
            args.push("--budget".to_string());
            args.push(budget.to_string());
        }
        if self.sandbox {
            args.push("--sandbox".to_string());
            args.push("--image".to_string());
            args.push(self.image.clone());
            args.push("--network".to_string());
            args.push(self.network.clone());
        }
        // In detached mode, auto-enable yolo unless readonly is specified
        // (interactive permission prompts don't work in background)
        if self.yolo || (!self.readonly) {
            args.push("--yolo".to_string());
        }
        if self.readonly {
            args.push("--readonly".to_string());
        }
        if let Some(ref allow) = self.allow {
            args.push("--allow".to_string());
            args.push(allow.clone());
        }
        if let Some(ref instr) = self.instructions {
            args.push("--instructions".to_string());
            args.push(instr.clone());
        }
        if self.no_instructions {
            args.push("--no-instructions".to_string());
        }
        if self.no_git {
            args.push("--no-git".to_string());
        }
        if self.no_auto_merge {
            args.push("--no-auto-merge".to_string());
        }
        if self.verbose {
            args.push("--verbose".to_string());
        }
        if self.no_hooks {
            args.push("--no-hooks".to_string());
        }
        if self.no_notify {
            args.push("--no-notify".to_string());
        }
        for var in &self.template_vars {
            args.push("--var".to_string());
            args.push(var.clone());
        }

        // Add internal flag to indicate this is a detached worker
        args.push("--internal-detached".to_string());

        // Get current executable path
        let exe_path = std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("doodoori"));

        // Get current working directory
        let cwd = std::env::current_dir()?;

        // Create log file path
        let log_dir = cwd.join(".doodoori/logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{}.log", task_id));

        // Open log file for stdout/stderr
        let log_file = std::fs::File::create(&log_path)?;
        let log_file_err = log_file.try_clone()?;

        // Spawn the detached process
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;

            let mut cmd = Command::new(&exe_path);
            cmd.args(&args)
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err));

            // Create new session (setsid) so process survives parent death
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            let child = cmd.spawn().context("Failed to spawn detached process")?;
            let pid = child.id();

            println!(
                "  Task ID:  {}",
                style(&task_id).green().bold()
            );
            println!("  PID:      {}", pid);
            println!("  Log:      {}", log_path.display());
            println!();
            println!(
                "{} Task is running in the background.",
                Emoji("✅", "")
            );
            println!("  Check status with: doodoori dashboard");
            println!("  View logs with:    tail -f {}", log_path.display());
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, just spawn normally (may not survive parent death)
            let child = Command::new(&exe_path)
                .args(&args)
                .current_dir(&cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .spawn()
                .context("Failed to spawn detached process")?;

            let pid = child.id();

            println!(
                "  Task ID:  {}",
                style(&task_id).green().bold()
            );
            println!("  PID:      {}", pid);
            println!("  Log:      {}", log_path.display());
            println!();
            println!(
                "{} Task is running in the background.",
                Emoji("✅", "")
            );
            println!("  Note: On Windows, the task may not survive if this terminal is closed.");
            println!("  Check status with: doodoori dashboard");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_template_vars_valid() {
        let args = RunArgs {
            prompt: None,
            spec: None,
            model: ModelAlias::Sonnet,
            budget: None,
            max_iterations: 50,
            sandbox: false,
            image: "doodoori/sandbox:latest".to_string(),
            network: "bridge".to_string(),
            dry_run: false,
            yolo: false,
            readonly: false,
            allow: None,
            instructions: None,
            no_instructions: false,
            no_git: false,
            no_auto_merge: false,
            verbose: false,
            no_hooks: false,
            notify: None,
            no_notify: false,
            format: "text".to_string(),
            output: None,
            template: None,
            template_vars: vec![
                "resource=users".to_string(),
                "path=/api/v1".to_string(),
            ],
            detach: false,
            internal_detached: false,
            #[cfg(feature = "dashboard")]
            dashboard: false,
        };

        let vars = args.parse_template_vars().unwrap();
        assert_eq!(vars.get("resource"), Some(&"users".to_string()));
        assert_eq!(vars.get("path"), Some(&"/api/v1".to_string()));
    }

    #[test]
    fn test_parse_template_vars_with_equals_in_value() {
        let args = RunArgs {
            prompt: None,
            spec: None,
            model: ModelAlias::Sonnet,
            budget: None,
            max_iterations: 50,
            sandbox: false,
            image: "doodoori/sandbox:latest".to_string(),
            network: "bridge".to_string(),
            dry_run: false,
            yolo: false,
            readonly: false,
            allow: None,
            instructions: None,
            no_instructions: false,
            no_git: false,
            no_auto_merge: false,
            verbose: false,
            no_hooks: false,
            notify: None,
            no_notify: false,
            format: "text".to_string(),
            output: None,
            template: None,
            template_vars: vec!["url=https://example.com/path?foo=bar".to_string()],
            detach: false,
            internal_detached: false,
            #[cfg(feature = "dashboard")]
            dashboard: false,
        };

        let vars = args.parse_template_vars().unwrap();
        assert_eq!(
            vars.get("url"),
            Some(&"https://example.com/path?foo=bar".to_string())
        );
    }

    #[test]
    fn test_parse_template_vars_invalid_format() {
        let args = RunArgs {
            prompt: None,
            spec: None,
            model: ModelAlias::Sonnet,
            budget: None,
            max_iterations: 50,
            sandbox: false,
            image: "doodoori/sandbox:latest".to_string(),
            network: "bridge".to_string(),
            dry_run: false,
            yolo: false,
            readonly: false,
            allow: None,
            instructions: None,
            no_instructions: false,
            no_git: false,
            no_auto_merge: false,
            verbose: false,
            no_hooks: false,
            notify: None,
            no_notify: false,
            format: "text".to_string(),
            output: None,
            template: None,
            template_vars: vec!["invalid_format".to_string()],
            detach: false,
            internal_detached: false,
            #[cfg(feature = "dashboard")]
            dashboard: false,
        };

        let result = args.parse_template_vars();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid variable format"));
    }

    #[test]
    fn test_parse_template_vars_empty() {
        let args = RunArgs {
            prompt: None,
            spec: None,
            model: ModelAlias::Sonnet,
            budget: None,
            max_iterations: 50,
            sandbox: false,
            image: "doodoori/sandbox:latest".to_string(),
            network: "bridge".to_string(),
            dry_run: false,
            yolo: false,
            readonly: false,
            allow: None,
            instructions: None,
            no_instructions: false,
            no_git: false,
            no_auto_merge: false,
            verbose: false,
            no_hooks: false,
            notify: None,
            no_notify: false,
            format: "text".to_string(),
            output: None,
            template: None,
            template_vars: vec![],
            detach: false,
            internal_detached: false,
            #[cfg(feature = "dashboard")]
            dashboard: false,
        };

        let vars = args.parse_template_vars().unwrap();
        assert!(vars.is_empty());
    }

    #[test]
    fn test_template_required_variable_validation() {
        use crate::templates::{Template, TemplateCategory, TemplateVariable};

        let template = Template {
            name: "test-template".to_string(),
            description: "Test".to_string(),
            category: TemplateCategory::Test,
            prompt: "Do something with {{resource}}".to_string(),
            variables: vec![TemplateVariable {
                name: "resource".to_string(),
                description: "Name of the resource".to_string(),
                default: None,
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let vars = HashMap::new();
        let result = template.validate_variables(&vars);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Missing required variable: resource"));
    }

    #[test]
    fn test_template_default_model_override() {
        // Test that CLI model overrides template default when not using default
        let default_cli_model = ModelAlias::Sonnet;

        // When CLI has non-default model, it should be used
        let template_model = Some(ModelAlias::Opus);
        let cli_model = ModelAlias::Haiku;
        let effective_model = if cli_model == default_cli_model && template_model.is_some() {
            template_model.unwrap()
        } else {
            cli_model.clone()
        };
        assert_eq!(effective_model, ModelAlias::Haiku);

        // When CLI has default model and template has a model, use template's
        let template_model = Some(ModelAlias::Opus);
        let cli_model = ModelAlias::Sonnet;
        let effective_model = if cli_model == default_cli_model && template_model.is_some() {
            template_model.unwrap()
        } else {
            cli_model.clone()
        };
        assert_eq!(effective_model, ModelAlias::Opus);
    }

    #[test]
    fn test_template_default_max_iterations_override() {
        // Test that CLI max_iterations overrides template default when not using default
        let template_max_iter = Some(10u32);
        let cli_max_iter = 100u32;
        let default_cli_max_iter = 50u32;

        // When CLI has non-default value, it should be used
        let effective_max_iter = if cli_max_iter == default_cli_max_iter && template_max_iter.is_some() {
            template_max_iter.unwrap()
        } else {
            cli_max_iter
        };
        assert_eq!(effective_max_iter, 100);

        // When CLI has default value and template has a value, use template's
        let cli_max_iter = 50u32;
        let effective_max_iter = if cli_max_iter == default_cli_max_iter && template_max_iter.is_some() {
            template_max_iter.unwrap()
        } else {
            cli_max_iter
        };
        assert_eq!(effective_max_iter, 10);
    }
}
