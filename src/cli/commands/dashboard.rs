//! TUI Dashboard for monitoring doodoori tasks
//!
//! Requires the `dashboard` feature to be enabled.

use anyhow::Result;
use clap::Args;

/// Launch the TUI dashboard
#[derive(Args, Debug)]
pub struct DashboardArgs {
    /// Refresh interval in milliseconds
    #[arg(short, long, default_value = "500")]
    pub refresh: u64,

    /// Show only active tasks
    #[arg(long)]
    pub active_only: bool,
}

impl DashboardArgs {
    pub async fn execute(self) -> Result<()> {
        #[cfg(feature = "dashboard")]
        {
            run_dashboard(self.refresh, self.active_only).await
        }

        #[cfg(not(feature = "dashboard"))]
        {
            println!("Dashboard feature not enabled.");
            println!("Rebuild with: cargo build --features dashboard");
            Ok(())
        }
    }
}

#[cfg(feature = "dashboard")]
mod tui {
    use anyhow::Result;
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    };
    use ratatui::{
        Frame, Terminal,
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    };
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use crate::pricing::CostHistoryManager;
    use crate::state::{StateManager, TaskState};

    /// View mode for the dashboard
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ViewMode {
        /// Task list view
        TaskList,
        /// Task detail view
        TaskDetail,
        /// Log viewer
        LogView,
    }

    /// App state for the dashboard
    pub struct App {
        /// Current tab index
        pub tab_index: usize,
        /// Tab titles
        pub tabs: Vec<&'static str>,
        /// State manager
        pub state_manager: Option<StateManager>,
        /// Cost manager
        pub cost_manager: Option<CostHistoryManager>,
        /// Should quit
        pub should_quit: bool,
        /// Active only filter
        pub active_only: bool,
        /// Current view mode
        pub view_mode: ViewMode,
        /// Selected task index (for navigation)
        pub selected_task: usize,
        /// List of all tasks (active + recent)
        pub tasks: Vec<TaskState>,
        /// Log content for selected task
        pub log_content: Vec<String>,
        /// Log scroll position
        pub log_scroll: usize,
        /// Is log auto-scrolling (for real-time)
        pub log_auto_scroll: bool,
        /// Status message to display
        pub status_message: Option<(String, Instant)>,
        /// Task to restart after dashboard exits
        pub restart_task: Option<RestartInfo>,
        /// Log filter
        pub log_filter: LogFilter,
        /// Budget limit from config (USD)
        pub budget_limit: Option<f64>,
    }

    /// Log filter options
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum LogFilter {
        /// Show all logs
        #[default]
        All,
        /// Show only INFO logs
        Info,
        /// Show only ERROR logs
        Error,
        /// Show only CLAUDE logs
        Claude,
        /// Show only TOOL logs
        Tool,
    }

    impl LogFilter {
        /// Get the display name for the filter
        pub fn name(&self) -> &'static str {
            match self {
                LogFilter::All => "ALL",
                LogFilter::Info => "INFO",
                LogFilter::Error => "ERROR",
                LogFilter::Claude => "CLAUDE",
                LogFilter::Tool => "TOOL",
            }
        }

        /// Check if a log line matches the filter
        pub fn matches(&self, line: &str) -> bool {
            match self {
                LogFilter::All => true,
                LogFilter::Info => line.contains("[INFO]"),
                LogFilter::Error => line.contains("[ERROR]"),
                LogFilter::Claude => line.contains("[CLAUDE]"),
                LogFilter::Tool => line.contains("[TOOL]"),
            }
        }

        /// Cycle to next filter
        pub fn next(&self) -> LogFilter {
            match self {
                LogFilter::All => LogFilter::Info,
                LogFilter::Info => LogFilter::Error,
                LogFilter::Error => LogFilter::Claude,
                LogFilter::Claude => LogFilter::Tool,
                LogFilter::Tool => LogFilter::All,
            }
        }
    }

    /// Information needed to restart a task
    #[derive(Debug, Clone)]
    pub struct RestartInfo {
        pub prompt: String,
        pub model: String,
        pub max_iterations: u32,
        pub working_dir: Option<String>,
    }

    impl App {
        pub fn new(active_only: bool) -> Self {
            let project_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state_manager = StateManager::new(&project_dir).ok();
            let cost_manager = CostHistoryManager::for_project(&project_dir).ok();

            // Load budget limit from config
            let budget_limit = crate::config::DoodooriConfig::load()
                .ok()
                .and_then(|c| c.budget_limit);

            let mut app = Self {
                tab_index: 0,
                tabs: vec!["Tasks", "Cost", "Help"],
                state_manager,
                cost_manager,
                should_quit: false,
                active_only,
                view_mode: ViewMode::TaskList,
                selected_task: 0,
                tasks: Vec::new(),
                log_content: Vec::new(),
                log_scroll: 0,
                log_auto_scroll: true,
                status_message: None,
                restart_task: None,
                log_filter: LogFilter::default(),
                budget_limit,
            };

            app.load_tasks();
            app
        }

        pub fn next_tab(&mut self) {
            self.tab_index = (self.tab_index + 1) % self.tabs.len();
        }

        pub fn prev_tab(&mut self) {
            if self.tab_index > 0 {
                self.tab_index -= 1;
            } else {
                self.tab_index = self.tabs.len() - 1;
            }
        }

        /// Load tasks from state manager
        pub fn load_tasks(&mut self) {
            if let Some(ref state_manager) = self.state_manager {
                let mut tasks = Vec::new();

                // Load current state
                if let Ok(Some(state)) = state_manager.load_state() {
                    tasks.push(state);
                }

                // Load history (last 20 tasks)
                if let Ok(history) = state_manager.list_history(20) {
                    for task in history {
                        if !tasks.iter().any(|t| t.task_id == task.task_id) {
                            tasks.push(task);
                        }
                    }
                }

                // Filter if needed
                if self.active_only {
                    tasks.retain(|t| {
                        matches!(
                            t.status,
                            crate::state::TaskStatus::Running | crate::state::TaskStatus::Pending
                        )
                    });
                }

                self.tasks = tasks;
            }
        }

        /// Navigate to next task in list
        pub fn next_task(&mut self) {
            if !self.tasks.is_empty() {
                self.selected_task = (self.selected_task + 1).min(self.tasks.len() - 1);
            }
        }

        /// Navigate to previous task in list
        pub fn prev_task(&mut self) {
            if self.selected_task > 0 {
                self.selected_task -= 1;
            }
        }

        /// Enter task detail view
        pub fn view_task_detail(&mut self) {
            if !self.tasks.is_empty() {
                self.view_mode = ViewMode::TaskDetail;
            }
        }

        /// Enter log view
        pub fn view_logs(&mut self) {
            if !self.tasks.is_empty() {
                self.load_log_content();
                self.view_mode = ViewMode::LogView;
            }
        }

        /// Go back to task list
        pub fn back_to_list(&mut self) {
            self.view_mode = ViewMode::TaskList;
        }

        /// Toggle auto-scroll in log view
        pub fn toggle_auto_scroll(&mut self) {
            self.log_auto_scroll = !self.log_auto_scroll;
            if self.log_auto_scroll {
                self.scroll_to_bottom();
            }
        }

        /// Cycle through log filters
        pub fn cycle_log_filter(&mut self) {
            self.log_filter = self.log_filter.next();
            self.log_scroll = 0; // Reset scroll when filter changes
        }

        /// Scroll log up
        pub fn scroll_log_up(&mut self) {
            self.log_auto_scroll = false;
            if self.log_scroll > 0 {
                self.log_scroll -= 1;
            }
        }

        /// Scroll log down
        pub fn scroll_log_down(&mut self) {
            self.log_auto_scroll = false;
            if self.log_scroll < self.log_content.len().saturating_sub(1) {
                self.log_scroll += 1;
            }
        }

        /// Scroll log page up
        pub fn scroll_log_page_up(&mut self) {
            self.log_auto_scroll = false;
            self.log_scroll = self.log_scroll.saturating_sub(10);
        }

        /// Scroll log page down
        pub fn scroll_log_page_down(&mut self) {
            self.log_auto_scroll = false;
            self.log_scroll = (self.log_scroll + 10).min(self.log_content.len().saturating_sub(1));
        }

        /// Scroll to bottom of log
        pub fn scroll_to_bottom(&mut self) {
            self.log_scroll = self.log_content.len().saturating_sub(1);
        }

        /// Load log content for selected task
        pub fn load_log_content(&mut self) {
            if let Some(task) = self.tasks.get(self.selected_task) {
                let log_path =
                    PathBuf::from(".doodoori/logs").join(format!("{}.log", task.task_id));
                if log_path.exists() {
                    if let Ok(content) = fs::read_to_string(&log_path) {
                        self.log_content = content.lines().map(String::from).collect();
                        if self.log_auto_scroll {
                            self.scroll_to_bottom();
                        }
                    }
                } else {
                    self.log_content = vec!["[No logs available]".to_string()];
                }
            }
        }

        /// Refresh data (for real-time updates)
        pub fn refresh(&mut self) {
            self.load_tasks();
            if self.view_mode == ViewMode::LogView {
                self.load_log_content();
            }
            // Clear old status messages (after 3 seconds)
            if let Some((_, time)) = &self.status_message {
                if time.elapsed() > Duration::from_secs(3) {
                    self.status_message = None;
                }
            }
        }

        /// Kill the selected running task
        pub fn kill_selected_task(&mut self) {
            if let Some(task) = self.tasks.get(self.selected_task) {
                if task.status != crate::state::TaskStatus::Running {
                    self.status_message = Some((
                        format!("Task {} is not running", &task.task_id[..8]),
                        Instant::now(),
                    ));
                    return;
                }

                // Try to find and kill the process
                let task_id = task.task_id.clone();
                match Self::find_and_kill_task(&task_id) {
                    Ok(killed) => {
                        if killed {
                            // Update state to interrupted
                            if let Some(ref state_manager) = self.state_manager {
                                if let Ok(Some(mut state)) = state_manager.load_state() {
                                    if state.task_id == task_id {
                                        state.status = crate::state::TaskStatus::Interrupted;
                                        let _ = state_manager.save_state(&state);
                                    }
                                }
                            }
                            self.status_message = Some((
                                format!("✓ Killed task {}", &task_id[..8]),
                                Instant::now(),
                            ));
                            self.load_tasks();
                        } else {
                            self.status_message = Some((
                                format!("Process not found for task {}", &task_id[..8]),
                                Instant::now(),
                            ));
                        }
                    }
                    Err(e) => {
                        self.status_message = Some((
                            format!("Failed to kill: {}", e),
                            Instant::now(),
                        ));
                    }
                }
            }
        }

        /// Find and kill a task's process
        fn find_and_kill_task(task_id: &str) -> Result<bool> {
            use std::process::Command;

            // Find doodoori processes
            let output = Command::new("pgrep")
                .args(["-f", &format!("doodoori.*{}", &task_id[..8])])
                .output()?;

            if output.status.success() {
                let pids: Vec<&str> = std::str::from_utf8(&output.stdout)?
                    .trim()
                    .lines()
                    .collect();

                for pid in pids {
                    let _ = Command::new("kill")
                        .args(["-TERM", pid])
                        .output();
                }
                Ok(true)
            } else {
                // Try finding claude process with session
                let output = Command::new("pgrep")
                    .args(["-f", "claude.*--print"])
                    .output()?;

                if output.status.success() {
                    let pids: Vec<&str> = std::str::from_utf8(&output.stdout)?
                        .trim()
                        .lines()
                        .collect();

                    let has_pids = !pids.is_empty();
                    for pid in pids {
                        let _ = Command::new("kill")
                            .args(["-TERM", pid])
                            .output();
                    }
                    Ok(has_pids)
                } else {
                    Ok(false)
                }
            }
        }

        /// Prune stale tasks (running state but no process)
        pub fn prune_stale_tasks(&mut self) {
            let mut pruned_count = 0;

            if let Some(ref state_manager) = self.state_manager {
                // Check current state
                if let Ok(Some(mut state)) = state_manager.load_state() {
                    if state.status == crate::state::TaskStatus::Running {
                        // Check if process exists
                        if !Self::is_task_process_running(&state.task_id) {
                            state.status = crate::state::TaskStatus::Interrupted;
                            state.error = Some("Pruned: process not found".to_string());
                            let _ = state_manager.save_state(&state);
                            pruned_count += 1;
                        }
                    }
                }
            }

            self.load_tasks();
            self.status_message = Some((
                if pruned_count > 0 {
                    format!("✓ Pruned {} stale task(s)", pruned_count)
                } else {
                    "No stale tasks found".to_string()
                },
                Instant::now(),
            ));
        }

        /// Check if a task's process is still running
        fn is_task_process_running(task_id: &str) -> bool {
            use std::process::Command;

            // Check for doodoori process with task ID
            let output = Command::new("pgrep")
                .args(["-f", &format!("doodoori.*{}", &task_id[..8])])
                .output();

            if let Ok(output) = output {
                if output.status.success() && !output.stdout.is_empty() {
                    return true;
                }
            }

            // Check for any claude process
            let output = Command::new("pgrep")
                .args(["-f", "claude.*--print"])
                .output();

            if let Ok(output) = output {
                output.status.success() && !output.stdout.is_empty()
            } else {
                false
            }
        }

        /// Set status message (used in tests)
        #[allow(dead_code)]
        pub fn set_status(&mut self, message: String) {
            self.status_message = Some((message, Instant::now()));
        }

        /// Prepare to restart the selected task
        pub fn prepare_restart(&mut self) {
            if let Some(task) = self.tasks.get(self.selected_task) {
                // Check if task is in a restartable state
                match task.status {
                    crate::state::TaskStatus::Running | crate::state::TaskStatus::Pending => {
                        self.status_message = Some((
                            format!("Task {} is still running", &task.task_id[..8]),
                            Instant::now(),
                        ));
                        return;
                    }
                    _ => {}
                }

                // Save restart info
                self.restart_task = Some(RestartInfo {
                    prompt: task.prompt.clone(),
                    model: task.model.clone(),
                    max_iterations: task.max_iterations,
                    working_dir: task.working_dir.clone(),
                });

                // Set quit flag to exit dashboard and restart
                self.should_quit = true;
                self.status_message = Some((
                    format!("Restarting task {}...", &task.task_id[..8]),
                    Instant::now(),
                ));
            }
        }

        /// Take the restart info (consumes it)
        pub fn take_restart(&mut self) -> Option<RestartInfo> {
            self.restart_task.take()
        }
    }

    pub async fn run_dashboard(refresh_ms: u64, active_only: bool) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create app state
        let mut app = App::new(active_only);

        let tick_rate = Duration::from_millis(refresh_ms);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| ui(f, &app))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match app.view_mode {
                            ViewMode::TaskList => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Tab | KeyCode::Right if app.tab_index == 0 => {}
                                KeyCode::Tab => app.next_tab(),
                                KeyCode::BackTab | KeyCode::Left => app.prev_tab(),
                                KeyCode::Up => app.prev_task(),
                                KeyCode::Down => app.next_task(),
                                KeyCode::Enter => app.view_task_detail(),
                                KeyCode::Char('l') => app.view_logs(),
                                KeyCode::Char('k') => app.kill_selected_task(),
                                KeyCode::Char('p') => app.prune_stale_tasks(),
                                KeyCode::Char('r') => app.prepare_restart(),
                                _ => {}
                            },
                            ViewMode::TaskDetail => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.back_to_list(),
                                KeyCode::Char('l') => app.view_logs(),
                                KeyCode::Char('k') => app.kill_selected_task(),
                                KeyCode::Char('r') => app.prepare_restart(),
                                _ => {}
                            },
                            ViewMode::LogView => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.back_to_list(),
                                KeyCode::Char('f') => app.toggle_auto_scroll(),
                                KeyCode::Tab => app.cycle_log_filter(),
                                KeyCode::Up => app.scroll_log_up(),
                                KeyCode::Down => app.scroll_log_down(),
                                KeyCode::PageUp => app.scroll_log_page_up(),
                                KeyCode::PageDown => app.scroll_log_page_down(),
                                _ => {}
                            },
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                app.refresh();
                last_tick = Instant::now();
            }

            if app.should_quit {
                break;
            }
        }

        // Check for restart before restoring terminal
        let restart_info = app.take_restart();

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        // Execute restart if requested
        if let Some(info) = restart_info {
            println!("Restarting task...\n");

            // Build command arguments
            let args = vec![
                "run".to_string(),
                info.prompt,
                "--model".to_string(),
                info.model,
                "--max-iterations".to_string(),
                info.max_iterations.to_string(),
            ];

            // Change to working directory if specified
            if let Some(ref dir) = info.working_dir {
                if let Err(e) = std::env::set_current_dir(dir) {
                    eprintln!("Warning: Could not change to working directory {}: {}", dir, e);
                }
            }

            // Execute doodoori run
            let status = std::process::Command::new("doodoori")
                .args(&args)
                .status();

            match status {
                Ok(exit_status) => {
                    if !exit_status.success() {
                        eprintln!("\nTask exited with status: {}", exit_status);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to restart task: {}", e);
                    eprintln!("You can manually run:");
                    eprintln!("  doodoori run \"{}\" --model {} --max-iterations {}",
                        args[1], args[3], args[5]);
                }
            }
        }

        Ok(())
    }

    fn ui(f: &mut Frame, app: &App) {
        // Different layouts based on view mode
        match app.view_mode {
            ViewMode::TaskList => render_task_list_ui(f, app),
            ViewMode::TaskDetail => render_task_detail_ui(f, app),
            ViewMode::LogView => render_log_view_ui(f, app),
        }
    }

    fn render_task_list_ui(f: &mut Frame, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(f.area());

        // Tabs
        let titles: Vec<Line> = app
            .tabs
            .iter()
            .map(|t| Line::from(Span::styled(*t, Style::default().fg(Color::White))))
            .collect();
        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Doodoori Dashboard"),
            )
            .select(app.tab_index)
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            );
        f.render_widget(tabs, chunks[0]);

        // Content based on selected tab
        match app.tab_index {
            0 => render_tasks_tab(f, chunks[1], app),
            1 => render_cost_tab(f, chunks[1], app),
            2 => render_help_tab(f, chunks[1]),
            _ => {}
        }

        // Footer - show status message if available, otherwise show shortcuts
        let footer_text = if let Some((msg, _)) = &app.status_message {
            msg.clone()
        } else {
            "'q' quit, ↑/↓ navigate, Enter details, 'l' logs, 'r' restart, 'k' kill, 'p' prune".to_string()
        };
        let footer_style = if app.status_message.is_some() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let footer = Paragraph::new(footer_text)
            .style(footer_style)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }

    fn render_tasks_tab(f: &mut Frame, area: Rect, app: &App) {
        let block = Block::default()
            .title(format!("Tasks ({})", app.tasks.len()))
            .borders(Borders::ALL);

        if app.tasks.is_empty() {
            let text = Paragraph::new("No tasks found")
                .style(Style::default().fg(Color::DarkGray))
                .block(block);
            f.render_widget(text, area);
            return;
        }

        let rows: Vec<Row> = app
            .tasks
            .iter()
            .enumerate()
            .map(|(idx, task)| {
                let style = if idx == app.selected_task {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let status_color = match task.status {
                    crate::state::TaskStatus::Running => Color::Green,
                    crate::state::TaskStatus::Completed => Color::Blue,
                    crate::state::TaskStatus::Failed => Color::Red,
                    crate::state::TaskStatus::Interrupted => Color::Yellow,
                    crate::state::TaskStatus::Pending => Color::Cyan,
                };

                Row::new(vec![
                    Cell::from(task.short_id().to_string()),
                    Cell::from(task.status.to_string()).style(Style::default().fg(status_color)),
                    Cell::from(format!(
                        "{}/{}",
                        task.current_iteration, task.max_iterations
                    )),
                    Cell::from(format!("${:.4}", task.total_cost_usd)),
                    Cell::from(task.model.clone()),
                ])
                .style(style)
            })
            .collect();

        let header = Row::new(vec!["ID", "Status", "Iter", "Cost", "Model"])
            .style(Style::default().fg(Color::Yellow))
            .bottom_margin(1);

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(15),
                Constraint::Percentage(20),
                Constraint::Percentage(15),
                Constraint::Percentage(20),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(block);

        f.render_widget(table, area);
    }

    fn render_cost_tab(f: &mut Frame, area: Rect, app: &App) {
        let block = Block::default().title("Cost Summary").borders(Borders::ALL);

        if let Some(ref cost_manager) = app.cost_manager {
            let history = cost_manager.history();
            let total = history.get_total_cost();
            let monthly = history.get_monthly_total();
            let (input, output) = history.get_total_tokens();

            // Determine budget status and colors
            let (monthly_color, budget_warning) = if let Some(limit) = app.budget_limit {
                let usage_pct = (monthly / limit) * 100.0;
                if monthly >= limit {
                    (Color::Red, Some(format!("⚠ BUDGET EXCEEDED ({:.1}%)", usage_pct)))
                } else if usage_pct >= 80.0 {
                    (Color::Yellow, Some(format!("⚠ Budget warning: {:.1}% used", usage_pct)))
                } else {
                    (Color::Cyan, None)
                }
            } else {
                (Color::Cyan, None)
            };

            let mut text = vec![
                Line::from(vec![
                    Span::raw("All Time: "),
                    Span::styled(format!("${:.4}", total), Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::raw("This Month: "),
                    Span::styled(format!("${:.4}", monthly), Style::default().fg(monthly_color)),
                ]),
            ];

            // Add budget info
            if let Some(limit) = app.budget_limit {
                text.push(Line::from(vec![
                    Span::raw("Budget:     "),
                    Span::styled(format!("${:.2}", limit), Style::default().fg(Color::White)),
                ]));
            }

            // Add budget warning if any
            if let Some(warning) = budget_warning {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    warning,
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )));
            }

            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::raw("Input Tokens: "),
                Span::styled(format!("{}", input), Style::default().fg(Color::Yellow)),
            ]));
            text.push(Line::from(vec![
                Span::raw("Output Tokens: "),
                Span::styled(format!("{}", output), Style::default().fg(Color::Yellow)),
            ]));

            let paragraph = Paragraph::new(text).block(block);
            f.render_widget(paragraph, area);
        } else {
            let text = Paragraph::new("Cost manager not available")
                .style(Style::default().fg(Color::Red))
                .block(block);
            f.render_widget(text, area);
        }
    }

    fn render_task_detail_ui(f: &mut Frame, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        if let Some(task) = app.tasks.get(app.selected_task) {
            let duration_secs = task.duration_ms / 1000;
            let duration_str = if duration_secs < 60 {
                format!("{}s", duration_secs)
            } else if duration_secs < 3600 {
                format!("{}m {}s", duration_secs / 60, duration_secs % 60)
            } else {
                format!("{}h {}m", duration_secs / 3600, (duration_secs % 3600) / 60)
            };

            let prompt_preview = task.prompt.lines().take(10).collect::<Vec<_>>().join("\n");
            let prompt_preview = if task.prompt.lines().count() > 10 {
                format!("{}...", prompt_preview)
            } else {
                prompt_preview
            };

            let text = vec![
                Line::from(vec![
                    Span::styled("ID:       ", Style::default().fg(Color::Yellow)),
                    Span::raw(&task.task_id),
                ]),
                Line::from(vec![
                    Span::styled("Status:   ", Style::default().fg(Color::Yellow)),
                    Span::raw(task.status.to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Model:    ", Style::default().fg(Color::Yellow)),
                    Span::raw(&task.model),
                ]),
                Line::from(vec![
                    Span::styled("Started:  ", Style::default().fg(Color::Yellow)),
                    Span::raw(task.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Duration: ", Style::default().fg(Color::Yellow)),
                    Span::raw(duration_str),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Progress: ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!(
                        "{}/{} iterations",
                        task.current_iteration, task.max_iterations
                    )),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Tokens:   ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!(
                        "Input: {}  Output: {}",
                        task.usage.input_tokens, task.usage.output_tokens
                    )),
                ]),
                Line::from(vec![
                    Span::raw("          "),
                    Span::raw(format!(
                        "Cache Write: {}  Cache Read: {}",
                        task.usage.cache_creation_tokens, task.usage.cache_read_tokens
                    )),
                ]),
                Line::from(vec![
                    Span::styled("Cost:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("${:.4}", task.total_cost_usd),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Prompt:",
                    Style::default().fg(Color::Yellow),
                )]),
                Line::from("─────────────────────────────────────────────────────"),
            ];

            let mut all_lines = text;
            for line in prompt_preview.lines() {
                all_lines.push(Line::from(line));
            }

            let block = Block::default().title("Task Details").borders(Borders::ALL);
            let paragraph = Paragraph::new(all_lines).block(block);
            f.render_widget(paragraph, chunks[0]);
        }

        let footer = Paragraph::new("'l' logs, 'r' restart, 'k' kill, Esc back, 'q' quit")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[1]);
    }

    fn render_log_view_ui(f: &mut Frame, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        if let Some(task) = app.tasks.get(app.selected_task) {
            let is_running = matches!(
                task.status,
                crate::state::TaskStatus::Running | crate::state::TaskStatus::Pending
            );

            let auto_scroll_text = if app.log_auto_scroll { "ON" } else { "OFF" };
            let filter_text = app.log_filter.name();
            let status_text = if is_running {
                format!("Running | Filter: {} | Auto-scroll: {}", filter_text, auto_scroll_text)
            } else {
                format!("Filter: {} | Auto-scroll: {}", filter_text, auto_scroll_text)
            };

            let title = format!("Logs: {} ({})", task.short_id(), status_text);

            // Filter log content based on current filter
            let filtered_content: Vec<&String> = app.log_content
                .iter()
                .filter(|line| app.log_filter.matches(line))
                .collect();

            // Calculate visible window
            let visible_height = chunks[0].height.saturating_sub(2) as usize; // -2 for borders
            let start_idx = app.log_scroll.min(filtered_content.len().saturating_sub(1));
            let end_idx = (start_idx + visible_height).min(filtered_content.len());

            let log_lines: Vec<Line> = filtered_content[start_idx..end_idx]
                .iter()
                .map(|line| {
                    // Simple syntax highlighting
                    if line.contains("[ERROR]") {
                        Line::from(Span::styled(line.as_str(), Style::default().fg(Color::Red)))
                    } else if line.contains("[INFO]") {
                        Line::from(Span::styled(
                            line.as_str(),
                            Style::default().fg(Color::Green),
                        ))
                    } else if line.contains("[CLAUDE]") {
                        Line::from(Span::styled(
                            line.as_str(),
                            Style::default().fg(Color::Cyan),
                        ))
                    } else if line.contains("[TOOL]") {
                        Line::from(Span::styled(
                            line.as_str(),
                            Style::default().fg(Color::Yellow),
                        ))
                    } else {
                        Line::from(line.as_str())
                    }
                })
                .collect();

            let block = Block::default().title(title).borders(Borders::ALL);
            let paragraph = Paragraph::new(log_lines).block(block);
            f.render_widget(paragraph, chunks[0]);
        }

        let footer = Paragraph::new(
            "'f' auto-scroll, Tab filter, ↑/↓ scroll, PgUp/PgDn pages, Esc back",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[1]);
    }

    fn render_help_tab(f: &mut Frame, area: Rect) {
        let text = vec![
            Line::from("Keyboard Shortcuts:"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Task List View:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  ↑/↓       ", Style::default().fg(Color::Cyan)),
                Span::raw("Navigate task list"),
            ]),
            Line::from(vec![
                Span::styled("  Enter     ", Style::default().fg(Color::Cyan)),
                Span::raw("View task details"),
            ]),
            Line::from(vec![
                Span::styled("  l         ", Style::default().fg(Color::Cyan)),
                Span::raw("View logs"),
            ]),
            Line::from(vec![
                Span::styled("  r         ", Style::default().fg(Color::Cyan)),
                Span::raw("Restart task"),
            ]),
            Line::from(vec![
                Span::styled("  k         ", Style::default().fg(Color::Cyan)),
                Span::raw("Kill running task"),
            ]),
            Line::from(vec![
                Span::styled("  p         ", Style::default().fg(Color::Cyan)),
                Span::raw("Prune stale tasks"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Log View:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  f         ", Style::default().fg(Color::Cyan)),
                Span::raw("Toggle auto-scroll"),
            ]),
            Line::from(vec![
                Span::styled("  Tab       ", Style::default().fg(Color::Cyan)),
                Span::raw("Cycle log filter (ALL/INFO/ERROR/CLAUDE/TOOL)"),
            ]),
            Line::from(vec![
                Span::styled("  ↑/↓       ", Style::default().fg(Color::Cyan)),
                Span::raw("Scroll logs"),
            ]),
            Line::from(vec![
                Span::styled("  PgUp/PgDn ", Style::default().fg(Color::Cyan)),
                Span::raw("Scroll pages"),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Global:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                Span::styled("  q         ", Style::default().fg(Color::Cyan)),
                Span::raw("Quit dashboard"),
            ]),
            Line::from(vec![
                Span::styled("  Esc       ", Style::default().fg(Color::Cyan)),
                Span::raw("Go back"),
            ]),
            Line::from(vec![
                Span::styled("  Tab       ", Style::default().fg(Color::Cyan)),
                Span::raw("Next tab"),
            ]),
        ];

        let block = Block::default().title("Help").borders(Borders::ALL);
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }
}

#[cfg(feature = "dashboard")]
pub use tui::run_dashboard;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_args_default() {
        let args = DashboardArgs {
            refresh: 500,
            active_only: false,
        };

        assert_eq!(args.refresh, 500);
        assert!(!args.active_only);
    }

    #[test]
    fn test_dashboard_args_custom() {
        let args = DashboardArgs {
            refresh: 1000,
            active_only: true,
        };

        assert_eq!(args.refresh, 1000);
        assert!(args.active_only);
    }

    #[cfg(feature = "dashboard")]
    mod tui_tests {
        use super::super::tui::*;

        #[test]
        fn test_view_mode_enum() {
            let mode = ViewMode::TaskList;
            assert_eq!(mode, ViewMode::TaskList);

            let mode2 = ViewMode::TaskDetail;
            assert_eq!(mode2, ViewMode::TaskDetail);

            let mode3 = ViewMode::LogView;
            assert_eq!(mode3, ViewMode::LogView);
        }

        #[test]
        fn test_app_new() {
            let app = App::new(false);

            assert_eq!(app.tab_index, 0);
            assert_eq!(app.tabs.len(), 3);
            assert_eq!(app.tabs[0], "Tasks");
            assert_eq!(app.tabs[1], "Cost");
            assert_eq!(app.tabs[2], "Help");
            assert!(!app.should_quit);
            assert!(!app.active_only);
            assert_eq!(app.view_mode, ViewMode::TaskList);
            assert_eq!(app.selected_task, 0);
            assert!(app.log_auto_scroll);
            assert_eq!(app.log_scroll, 0);
        }

        #[test]
        fn test_app_new_active_only() {
            let app = App::new(true);
            assert!(app.active_only);
        }

        #[test]
        fn test_tab_navigation() {
            let mut app = App::new(false);

            assert_eq!(app.tab_index, 0);

            app.next_tab();
            assert_eq!(app.tab_index, 1);

            app.next_tab();
            assert_eq!(app.tab_index, 2);

            // Should wrap around
            app.next_tab();
            assert_eq!(app.tab_index, 0);
        }

        #[test]
        fn test_tab_navigation_previous() {
            let mut app = App::new(false);

            assert_eq!(app.tab_index, 0);

            // Should wrap to last tab
            app.prev_tab();
            assert_eq!(app.tab_index, 2);

            app.prev_tab();
            assert_eq!(app.tab_index, 1);

            app.prev_tab();
            assert_eq!(app.tab_index, 0);
        }

        #[test]
        fn test_view_mode_transitions() {
            let mut app = App::new(false);

            // Ensure tasks are empty for this test
            app.tasks.clear();

            assert_eq!(app.view_mode, ViewMode::TaskList);

            // Can't view details when no tasks
            app.view_task_detail();
            assert_eq!(app.view_mode, ViewMode::TaskList);

            // Can't view logs when no tasks
            app.view_logs();
            assert_eq!(app.view_mode, ViewMode::TaskList);
        }

        #[test]
        fn test_back_to_list() {
            let mut app = App::new(false);

            app.view_mode = ViewMode::TaskDetail;
            app.back_to_list();
            assert_eq!(app.view_mode, ViewMode::TaskList);

            app.view_mode = ViewMode::LogView;
            app.back_to_list();
            assert_eq!(app.view_mode, ViewMode::TaskList);
        }

        #[test]
        fn test_task_navigation_empty() {
            let mut app = App::new(false);

            // Should handle empty task list gracefully
            app.next_task();
            assert_eq!(app.selected_task, 0);

            app.prev_task();
            assert_eq!(app.selected_task, 0);
        }

        #[test]
        fn test_log_auto_scroll_toggle() {
            let mut app = App::new(false);

            assert!(app.log_auto_scroll);

            app.toggle_auto_scroll();
            assert!(!app.log_auto_scroll);

            app.toggle_auto_scroll();
            assert!(app.log_auto_scroll);
        }

        #[test]
        fn test_log_scroll_operations() {
            let mut app = App::new(false);

            // Setup log content
            app.log_content = vec![
                "Line 1".to_string(),
                "Line 2".to_string(),
                "Line 3".to_string(),
                "Line 4".to_string(),
                "Line 5".to_string(),
            ];
            app.log_scroll = 0;

            // Scroll down
            app.scroll_log_down();
            assert_eq!(app.log_scroll, 1);
            assert!(!app.log_auto_scroll); // Should disable auto-scroll

            // Scroll up
            app.scroll_log_up();
            assert_eq!(app.log_scroll, 0);

            // Scroll up at top - should stay at 0
            app.scroll_log_up();
            assert_eq!(app.log_scroll, 0);
        }

        #[test]
        fn test_log_page_scroll() {
            let mut app = App::new(false);

            // Setup log content
            app.log_content = (0..50).map(|i| format!("Line {}", i)).collect();
            app.log_scroll = 20;

            // Page down
            app.scroll_log_page_down();
            assert_eq!(app.log_scroll, 30);
            assert!(!app.log_auto_scroll);

            // Page up
            app.scroll_log_page_up();
            assert_eq!(app.log_scroll, 20);
        }

        #[test]
        fn test_scroll_to_bottom() {
            let mut app = App::new(false);

            app.log_content = vec![
                "Line 1".to_string(),
                "Line 2".to_string(),
                "Line 3".to_string(),
            ];
            app.log_scroll = 0;

            app.scroll_to_bottom();
            assert_eq!(app.log_scroll, 2); // Last line index
        }

        #[test]
        fn test_scroll_to_bottom_empty_log() {
            let mut app = App::new(false);

            app.log_content = vec![];
            app.scroll_to_bottom();
            assert_eq!(app.log_scroll, 0);
        }

        #[test]
        fn test_status_message() {
            let mut app = App::new(false);

            assert!(app.status_message.is_none());

            app.set_status("Test message".to_string());
            assert!(app.status_message.is_some());

            let (msg, _) = app.status_message.as_ref().unwrap();
            assert_eq!(msg, "Test message");
        }

        #[test]
        fn test_kill_selected_task_not_running() {
            use crate::state::{TaskState, TaskStatus, TokenUsage};
            use chrono::Utc;

            let mut app = App::new(false);

            // Create a completed task
            let task = TaskState {
                task_id: "test-task-12345678".to_string(),
                prompt: "Test prompt".to_string(),
                model: "sonnet".to_string(),
                max_iterations: 5,
                current_iteration: 3,
                status: TaskStatus::Completed,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                duration_ms: 1000,
                usage: TokenUsage::default(),
                total_cost_usd: 0.0,
                session_id: None,
                error: None,
                final_output: None,
                working_dir: Some(".".to_string()),
            };
            app.tasks = vec![task];
            app.selected_task = 0;

            // Try to kill a completed task - should set error message
            app.kill_selected_task();

            assert!(app.status_message.is_some());
            let (msg, _) = app.status_message.as_ref().unwrap();
            assert!(msg.contains("not running"));
        }

        #[test]
        fn test_kill_selected_task_empty_list() {
            let mut app = App::new(false);
            app.tasks = vec![];

            // Should not panic with empty task list
            app.kill_selected_task();
        }

        #[test]
        fn test_prune_stale_tasks_sets_message() {
            let mut app = App::new(false);

            // Prune should always set a status message
            app.prune_stale_tasks();

            assert!(app.status_message.is_some());
        }

        #[test]
        fn test_restart_info_struct() {
            let info = RestartInfo {
                prompt: "Test prompt".to_string(),
                model: "sonnet".to_string(),
                max_iterations: 10,
                working_dir: Some("/tmp".to_string()),
            };

            assert_eq!(info.prompt, "Test prompt");
            assert_eq!(info.model, "sonnet");
            assert_eq!(info.max_iterations, 10);
            assert_eq!(info.working_dir, Some("/tmp".to_string()));
        }

        #[test]
        fn test_prepare_restart_running_task() {
            use crate::state::{TaskState, TaskStatus, TokenUsage};
            use chrono::Utc;

            let mut app = App::new(false);

            // Create a running task
            let task = TaskState {
                task_id: "test-task-12345678".to_string(),
                prompt: "Test prompt".to_string(),
                model: "sonnet".to_string(),
                max_iterations: 5,
                current_iteration: 3,
                status: TaskStatus::Running,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                duration_ms: 1000,
                usage: TokenUsage::default(),
                total_cost_usd: 0.0,
                session_id: None,
                error: None,
                final_output: None,
                working_dir: Some(".".to_string()),
            };
            app.tasks = vec![task];
            app.selected_task = 0;

            // Try to restart a running task - should not set restart_task
            app.prepare_restart();

            assert!(app.restart_task.is_none());
            assert!(!app.should_quit);
            assert!(app.status_message.is_some());
            let (msg, _) = app.status_message.as_ref().unwrap();
            assert!(msg.contains("still running"));
        }

        #[test]
        fn test_prepare_restart_failed_task() {
            use crate::state::{TaskState, TaskStatus, TokenUsage};
            use chrono::Utc;

            let mut app = App::new(false);

            // Create a failed task
            let task = TaskState {
                task_id: "test-task-12345678".to_string(),
                prompt: "Test prompt".to_string(),
                model: "opus".to_string(),
                max_iterations: 10,
                current_iteration: 5,
                status: TaskStatus::Failed,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                duration_ms: 2000,
                usage: TokenUsage::default(),
                total_cost_usd: 0.5,
                session_id: None,
                error: Some("Test error".to_string()),
                final_output: None,
                working_dir: Some("/project".to_string()),
            };
            app.tasks = vec![task];
            app.selected_task = 0;

            // Restart a failed task - should set restart_task
            app.prepare_restart();

            assert!(app.restart_task.is_some());
            assert!(app.should_quit);

            let info = app.restart_task.as_ref().unwrap();
            assert_eq!(info.prompt, "Test prompt");
            assert_eq!(info.model, "opus");
            assert_eq!(info.max_iterations, 10);
            assert_eq!(info.working_dir, Some("/project".to_string()));
        }

        #[test]
        fn test_take_restart() {
            use crate::state::{TaskState, TaskStatus, TokenUsage};
            use chrono::Utc;

            let mut app = App::new(false);

            // Create a completed task
            let task = TaskState {
                task_id: "test-task-12345678".to_string(),
                prompt: "Test prompt".to_string(),
                model: "haiku".to_string(),
                max_iterations: 3,
                current_iteration: 3,
                status: TaskStatus::Completed,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                duration_ms: 500,
                usage: TokenUsage::default(),
                total_cost_usd: 0.1,
                session_id: None,
                error: None,
                final_output: Some("Done".to_string()),
                working_dir: None,
            };
            app.tasks = vec![task];
            app.selected_task = 0;

            // Prepare restart
            app.prepare_restart();
            assert!(app.restart_task.is_some());

            // Take restart - should consume it
            let info = app.take_restart();
            assert!(info.is_some());
            assert!(app.restart_task.is_none());

            // Second take should return None
            let info2 = app.take_restart();
            assert!(info2.is_none());
        }

        #[test]
        fn test_log_filter_default() {
            let filter = LogFilter::default();
            assert_eq!(filter, LogFilter::All);
            assert_eq!(filter.name(), "ALL");
        }

        #[test]
        fn test_log_filter_matches() {
            // All filter matches everything
            assert!(LogFilter::All.matches("[INFO] test"));
            assert!(LogFilter::All.matches("[ERROR] test"));
            assert!(LogFilter::All.matches("random text"));

            // Specific filters
            assert!(LogFilter::Info.matches("[INFO] test message"));
            assert!(!LogFilter::Info.matches("[ERROR] test message"));

            assert!(LogFilter::Error.matches("[ERROR] test message"));
            assert!(!LogFilter::Error.matches("[INFO] test message"));

            assert!(LogFilter::Claude.matches("[CLAUDE] response"));
            assert!(!LogFilter::Claude.matches("[TOOL] call"));

            assert!(LogFilter::Tool.matches("[TOOL] call"));
            assert!(!LogFilter::Tool.matches("[CLAUDE] response"));
        }

        #[test]
        fn test_log_filter_cycle() {
            let filter = LogFilter::All;
            assert_eq!(filter.next(), LogFilter::Info);
            assert_eq!(filter.next().next(), LogFilter::Error);
            assert_eq!(filter.next().next().next(), LogFilter::Claude);
            assert_eq!(filter.next().next().next().next(), LogFilter::Tool);
            assert_eq!(filter.next().next().next().next().next(), LogFilter::All);
        }

        #[test]
        fn test_app_cycle_log_filter() {
            let mut app = App::new(false);

            assert_eq!(app.log_filter, LogFilter::All);

            app.cycle_log_filter();
            assert_eq!(app.log_filter, LogFilter::Info);

            app.cycle_log_filter();
            assert_eq!(app.log_filter, LogFilter::Error);

            app.cycle_log_filter();
            assert_eq!(app.log_filter, LogFilter::Claude);

            app.cycle_log_filter();
            assert_eq!(app.log_filter, LogFilter::Tool);

            app.cycle_log_filter();
            assert_eq!(app.log_filter, LogFilter::All);
        }

        #[test]
        fn test_cycle_log_filter_resets_scroll() {
            let mut app = App::new(false);
            app.log_scroll = 10;

            app.cycle_log_filter();

            assert_eq!(app.log_scroll, 0);
        }
    }
}
