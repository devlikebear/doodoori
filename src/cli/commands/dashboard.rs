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
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
        Frame, Terminal,
    };
    use std::io;
    use std::time::{Duration, Instant};

    use crate::pricing::CostHistoryManager;
    use crate::state::StateManager;

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
    }

    impl App {
        pub fn new(active_only: bool) -> Self {
            let project_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state_manager = StateManager::new(&project_dir).ok();
            let cost_manager = CostHistoryManager::for_project(&project_dir).ok();

            Self {
                tab_index: 0,
                tabs: vec!["Tasks", "Cost", "Help"],
                state_manager,
                cost_manager,
                should_quit: false,
                active_only,
            }
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
                        match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Tab | KeyCode::Right => app.next_tab(),
                            KeyCode::BackTab | KeyCode::Left => app.prev_tab(),
                            _ => {}
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
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
            .block(Block::default().borders(Borders::ALL).title("Doodoori Dashboard"))
            .select(app.tab_index)
            .style(Style::default().fg(Color::Cyan))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow));
        f.render_widget(tabs, chunks[0]);

        // Content based on selected tab
        match app.tab_index {
            0 => render_tasks_tab(f, chunks[1], app),
            1 => render_cost_tab(f, chunks[1], app),
            2 => render_help_tab(f, chunks[1]),
            _ => {}
        }

        // Footer
        let footer = Paragraph::new("Press 'q' to quit, Tab/Arrow keys to switch tabs")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }

    fn render_tasks_tab(f: &mut Frame, area: Rect, app: &App) {
        let block = Block::default()
            .title("Active Tasks")
            .borders(Borders::ALL);

        if let Some(ref state_manager) = app.state_manager {
            if let Ok(Some(state)) = state_manager.load_state() {
                let status_str = format!("{:?}", state.status);
                let rows = vec![
                    Row::new(vec![
                        Cell::from(state.task_id[..8.min(state.task_id.len())].to_string()),
                        Cell::from(status_str),
                        Cell::from(format!("{}/{}", state.current_iteration, state.max_iterations)),
                        Cell::from(format!("${:.4}", state.total_cost_usd)),
                    ]),
                ];

                let header = Row::new(vec!["Task ID", "Status", "Iteration", "Cost"])
                    .style(Style::default().fg(Color::Yellow))
                    .bottom_margin(1);

                let table = Table::new(
                    rows,
                    [
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ],
                )
                .header(header)
                .block(block);

                f.render_widget(table, area);
            } else {
                let text = Paragraph::new("No active tasks")
                    .style(Style::default().fg(Color::DarkGray))
                    .block(block);
                f.render_widget(text, area);
            }
        } else {
            let text = Paragraph::new("State manager not available")
                .style(Style::default().fg(Color::Red))
                .block(block);
            f.render_widget(text, area);
        }
    }

    fn render_cost_tab(f: &mut Frame, area: Rect, app: &App) {
        let block = Block::default()
            .title("Cost Summary")
            .borders(Borders::ALL);

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

    fn render_help_tab(f: &mut Frame, area: Rect) {
        let text = vec![
            Line::from("Keyboard Shortcuts:"),
            Line::from(""),
            Line::from(vec![
                Span::styled("  q     ", Style::default().fg(Color::Yellow)),
                Span::raw("Quit dashboard"),
            ]),
            Line::from(vec![
                Span::styled("  Tab   ", Style::default().fg(Color::Yellow)),
                Span::raw("Next tab"),
            ]),
            Line::from(vec![
                Span::styled("  Shift+Tab   ", Style::default().fg(Color::Yellow)),
                Span::raw("Previous tab"),
            ]),
            Line::from(vec![
                Span::styled("  ←/→   ", Style::default().fg(Color::Yellow)),
                Span::raw("Switch tabs"),
            ]),
        ];

        let block = Block::default().title("Help").borders(Borders::ALL);
        let paragraph = Paragraph::new(text).block(block);
        f.render_widget(paragraph, area);
    }
}

#[cfg(feature = "dashboard")]
pub use tui::run_dashboard;
