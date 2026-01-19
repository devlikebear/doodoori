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
    }

    impl App {
        pub fn new(active_only: bool) -> Self {
            let project_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state_manager = StateManager::new(&project_dir).ok();
            let cost_manager = CostHistoryManager::for_project(&project_dir).ok();

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
                                _ => {}
                            },
                            ViewMode::TaskDetail => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.back_to_list(),
                                KeyCode::Char('l') => app.view_logs(),
                                _ => {}
                            },
                            ViewMode::LogView => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.back_to_list(),
                                KeyCode::Char('f') => app.toggle_auto_scroll(),
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

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

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

        // Footer
        let footer =
            Paragraph::new("Press 'q' to quit, ↑/↓ to navigate, Enter for details, 'l' for logs")
                .style(Style::default().fg(Color::DarkGray))
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

            let text = vec![
                Line::from(vec![
                    Span::raw("All Time: "),
                    Span::styled(format!("${:.4}", total), Style::default().fg(Color::Green)),
                ]),
                Line::from(vec![
                    Span::raw("This Month: "),
                    Span::styled(format!("${:.4}", monthly), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Input Tokens: "),
                    Span::styled(format!("{}", input), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::raw("Output Tokens: "),
                    Span::styled(format!("{}", output), Style::default().fg(Color::Yellow)),
                ]),
            ];

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

        let footer = Paragraph::new("Press 'l' for logs, Esc to go back, 'q' to quit")
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
            let status_text = if is_running {
                format!("Running - Auto-scroll {}", auto_scroll_text)
            } else {
                format!("Auto-scroll {}", auto_scroll_text)
            };

            let title = format!("Logs: {} ({})", task.short_id(), status_text);

            // Calculate visible window
            let visible_height = chunks[0].height.saturating_sub(2) as usize; // -2 for borders
            let start_idx = app.log_scroll;
            let end_idx = (start_idx + visible_height).min(app.log_content.len());

            let log_lines: Vec<Line> = app.log_content[start_idx..end_idx]
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
            "Press 'f' to toggle auto-scroll, ↑/↓ to scroll, PgUp/PgDn for pages, Esc to go back",
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
    }
}
