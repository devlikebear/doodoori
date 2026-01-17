use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::output::{CostOutput, OutputFormat, OutputWriter};
use crate::pricing::{format_cost, format_tokens, CostHistoryManager};

/// View cost history and tracking
#[derive(Args, Debug)]
pub struct CostArgs {
    /// Show full history
    #[arg(long)]
    pub history: bool,

    /// Show cost for specific task
    #[arg(long)]
    pub task_id: Option<String>,

    /// Show daily summary
    #[arg(long)]
    pub daily: bool,

    /// Reset cost tracking
    #[arg(long)]
    pub reset: bool,

    /// Number of entries to show (default: 10)
    #[arg(short = 'n', long, default_value = "10")]
    pub limit: usize,

    /// Project directory (default: current directory)
    #[arg(long)]
    pub project: Option<PathBuf>,

    /// Output format (text, json, json-pretty, yaml, markdown)
    #[arg(long, short = 'f', default_value = "text")]
    pub format: String,

    /// Output file path (default: stdout)
    #[arg(long, short = 'o')]
    pub output: Option<String>,
}

impl CostArgs {
    pub async fn execute(self) -> Result<()> {
        let project_dir = self.project.clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let mut manager = match CostHistoryManager::for_project(&project_dir) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Warning: Could not load cost history: {}", e);
                return Ok(());
            }
        };

        if self.reset {
            println!("Resetting cost tracking...");
            manager.reset()?;
            println!("Cost history cleared.");
            return Ok(());
        }

        if let Some(ref task_id) = self.task_id {
            self.show_task_cost(&manager, task_id)?;
            return Ok(());
        }

        if self.daily {
            self.show_daily_summary(&manager)?;
            return Ok(());
        }

        if self.history {
            self.show_full_history(&manager)?;
            return Ok(());
        }

        // Default: show current summary
        self.show_summary(&manager)?;

        Ok(())
    }

    fn show_summary(&self, manager: &CostHistoryManager) -> Result<()> {
        let history = manager.history();

        // Parse output format
        let output_format: OutputFormat = self.format.parse().unwrap_or_default();

        // Get summary data
        let today_cost = history.get_today_summary()
            .map(|t| t.total_cost_usd)
            .unwrap_or(0.0);
        let monthly = history.get_monthly_total();
        let total = history.get_total_cost();
        let (input_tokens, output_tokens) = history.get_total_tokens();
        let task_count = history.entries.len() as u64;

        if output_format == OutputFormat::Text {
            println!("=== Cost Summary ===\n");

            // Today's cost
            if let Some(today) = history.get_today_summary() {
                println!("Today:       {} ({} tasks)", format_cost(today.total_cost_usd), today.task_count);
            } else {
                println!("Today:       $0.00 (0 tasks)");
            }

            // This month's cost
            println!("This month:  {}", format_cost(monthly));

            // All time
            println!("All time:    {}", format_cost(total));
            println!();

            // Token summary
            println!("Total tokens:");
            println!("  Input:  {}", format_tokens(input_tokens));
            println!("  Output: {}", format_tokens(output_tokens));

            // Recent activity
            let recent = history.get_recent_entries(5);
            if !recent.is_empty() {
                println!("\nRecent tasks:");
                for entry in recent {
                    let short_id = &entry.task_id[..8.min(entry.task_id.len())];
                    let desc = entry.description.as_deref().unwrap_or("-");
                    let desc_short = if desc.len() > 30 {
                        format!("{}...", &desc[..27])
                    } else {
                        desc.to_string()
                    };
                    println!(
                        "  {} │ {} │ {} │ {}",
                        short_id,
                        entry.model,
                        format_cost(entry.cost_usd),
                        desc_short
                    );
                }
            }
        } else {
            // Build structured output
            let cost_output = CostOutput::new()
                .with_today(today_cost)
                .with_month(monthly)
                .with_total(total)
                .with_task_count(task_count)
                .with_tokens(input_tokens, output_tokens);

            // Write output
            let writer = if let Some(ref path) = self.output {
                OutputWriter::new(output_format).with_file(path)
            } else {
                OutputWriter::new(output_format)
            };

            if let Err(e) = writer.write_cost(&cost_output) {
                tracing::error!("Failed to write output: {}", e);
            }
        }

        Ok(())
    }

    fn show_task_cost(&self, manager: &CostHistoryManager, task_id: &str) -> Result<()> {
        let history = manager.history();
        let entries = history.get_task_entries(task_id);

        if entries.is_empty() {
            println!("No cost entries found for task: {}", task_id);
            return Ok(());
        }

        println!("=== Cost for Task {} ===\n", task_id);

        let total = history.get_task_total(task_id);
        println!("Total cost: {}", format_cost(total));
        println!("Entries: {}\n", entries.len());

        for entry in entries {
            println!(
                "{} │ {} │ {} │ in:{} out:{}",
                entry.timestamp.format("%Y-%m-%d %H:%M"),
                entry.model,
                format_cost(entry.cost_usd),
                format_tokens(entry.input_tokens),
                format_tokens(entry.output_tokens),
            );
        }

        Ok(())
    }

    fn show_daily_summary(&self, manager: &CostHistoryManager) -> Result<()> {
        let history = manager.history();
        let summaries = history.get_recent_daily_summaries(self.limit);

        if summaries.is_empty() {
            println!("No daily summaries available.");
            return Ok(());
        }

        println!("=== Daily Cost Summary ===\n");
        println!("{:<12} {:>10} {:>8} {:>12} {:>12}", "Date", "Cost", "Tasks", "Input", "Output");
        println!("{}", "-".repeat(58));

        for summary in summaries {
            println!(
                "{:<12} {:>10} {:>8} {:>12} {:>12}",
                summary.date,
                format_cost(summary.total_cost_usd),
                summary.task_count,
                format_tokens(summary.total_input_tokens),
                format_tokens(summary.total_output_tokens),
            );
        }

        Ok(())
    }

    fn show_full_history(&self, manager: &CostHistoryManager) -> Result<()> {
        let history = manager.history();
        let recent = history.get_recent_entries(self.limit);

        if recent.is_empty() {
            println!("No cost history available.");
            return Ok(());
        }

        println!("=== Cost History (last {}) ===\n", self.limit);
        println!("{:<8} {:<16} {:>8} {:>10} {:>10} {:>10}",
            "Task", "Timestamp", "Model", "Cost", "In", "Out");
        println!("{}", "-".repeat(72));

        for entry in recent {
            let short_id = &entry.task_id[..8.min(entry.task_id.len())];
            println!(
                "{:<8} {:<16} {:>8} {:>10} {:>10} {:>10}",
                short_id,
                entry.timestamp.format("%m-%d %H:%M"),
                entry.model,
                format_cost(entry.cost_usd),
                format_tokens(entry.input_tokens),
                format_tokens(entry.output_tokens),
            );
        }

        let total = history.get_total_cost();
        println!("{}", "-".repeat(72));
        println!("Total: {}", format_cost(total));

        Ok(())
    }
}
