//! Cost history persistence and tracking

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A single cost entry for a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEntry {
    /// Task ID
    pub task_id: String,
    /// When this cost was recorded
    pub timestamp: DateTime<Utc>,
    /// Model used
    pub model: String,
    /// Input tokens
    pub input_tokens: u64,
    /// Output tokens
    pub output_tokens: u64,
    /// Cache read tokens
    pub cache_read_tokens: u64,
    /// Cache creation tokens
    pub cache_creation_tokens: u64,
    /// Total cost in USD
    pub cost_usd: f64,
    /// Task status when recorded
    pub status: String,
    /// Optional description or prompt summary
    pub description: Option<String>,
}

/// Daily cost summary
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailySummary {
    /// Date (YYYY-MM-DD format)
    pub date: String,
    /// Total cost for the day
    pub total_cost_usd: f64,
    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,
    /// Number of tasks
    pub task_count: u32,
    /// Cost by model
    pub by_model: HashMap<String, f64>,
}

/// Cost history container
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostHistory {
    /// All cost entries
    pub entries: Vec<CostEntry>,
    /// Daily summaries (cached)
    #[serde(default)]
    pub daily_summaries: HashMap<String, DailySummary>,
    /// Last updated timestamp
    pub updated_at: Option<DateTime<Utc>>,
}

impl CostHistory {
    /// Create a new empty cost history
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cost entry
    pub fn add_entry(&mut self, entry: CostEntry) {
        let date = entry.timestamp.format("%Y-%m-%d").to_string();

        // Update daily summary
        let summary = self.daily_summaries.entry(date.clone()).or_insert_with(|| {
            DailySummary {
                date,
                ..Default::default()
            }
        });

        summary.total_cost_usd += entry.cost_usd;
        summary.total_input_tokens += entry.input_tokens;
        summary.total_output_tokens += entry.output_tokens;
        summary.task_count += 1;
        *summary.by_model.entry(entry.model.clone()).or_insert(0.0) += entry.cost_usd;

        self.entries.push(entry);
        self.updated_at = Some(Utc::now());
    }

    /// Get entries for a specific task
    pub fn get_task_entries(&self, task_id: &str) -> Vec<&CostEntry> {
        self.entries
            .iter()
            .filter(|e| e.task_id == task_id || e.task_id.starts_with(task_id))
            .collect()
    }

    /// Get total cost for a task
    pub fn get_task_total(&self, task_id: &str) -> f64 {
        self.get_task_entries(task_id)
            .iter()
            .map(|e| e.cost_usd)
            .sum()
    }

    /// Get daily summary for a date
    pub fn get_daily_summary(&self, date: &str) -> Option<&DailySummary> {
        self.daily_summaries.get(date)
    }

    /// Get today's summary
    pub fn get_today_summary(&self) -> Option<&DailySummary> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.get_daily_summary(&today)
    }

    /// Get this month's total cost
    pub fn get_monthly_total(&self) -> f64 {
        let now = Utc::now();
        let year = now.year();
        let month = now.month();
        let prefix = format!("{:04}-{:02}", year, month);

        self.daily_summaries
            .iter()
            .filter(|(date, _)| date.starts_with(&prefix))
            .map(|(_, summary)| summary.total_cost_usd)
            .sum()
    }

    /// Get total cost across all time
    pub fn get_total_cost(&self) -> f64 {
        self.entries.iter().map(|e| e.cost_usd).sum()
    }

    /// Get recent entries (last N)
    pub fn get_recent_entries(&self, limit: usize) -> Vec<&CostEntry> {
        let start = if self.entries.len() > limit {
            self.entries.len() - limit
        } else {
            0
        };
        self.entries[start..].iter().collect()
    }

    /// Get daily summaries for the last N days
    pub fn get_recent_daily_summaries(&self, days: usize) -> Vec<&DailySummary> {
        let mut dates: Vec<&String> = self.daily_summaries.keys().collect();
        dates.sort();
        dates.reverse();

        dates.into_iter()
            .take(days)
            .filter_map(|date| self.daily_summaries.get(date))
            .collect()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
        self.daily_summaries.clear();
        self.updated_at = Some(Utc::now());
    }

    /// Get total token count
    pub fn get_total_tokens(&self) -> (u64, u64) {
        let input: u64 = self.entries.iter().map(|e| e.input_tokens).sum();
        let output: u64 = self.entries.iter().map(|e| e.output_tokens).sum();
        (input, output)
    }
}

/// Cost history manager for persistence
pub struct CostHistoryManager {
    /// Path to the cost history file
    file_path: PathBuf,
    /// Cached history
    history: CostHistory,
}

impl CostHistoryManager {
    /// Create a new cost history manager
    pub fn new(base_dir: &Path) -> Result<Self> {
        let file_path = base_dir.join("cost_history.json");
        let history = if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .context("Failed to read cost history file")?;
            serde_json::from_str(&content)
                .context("Failed to parse cost history file")?
        } else {
            CostHistory::new()
        };

        Ok(Self { file_path, history })
    }

    /// Create a manager for the .doodoori directory in the given project
    pub fn for_project(project_dir: &Path) -> Result<Self> {
        let base_dir = project_dir.join(".doodoori");
        fs::create_dir_all(&base_dir)
            .context("Failed to create .doodoori directory")?;
        Self::new(&base_dir)
    }

    /// Add a cost entry and save
    pub fn record_cost(
        &mut self,
        task_id: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
        cost_usd: f64,
        status: &str,
        description: Option<String>,
    ) -> Result<()> {
        let entry = CostEntry {
            task_id: task_id.to_string(),
            timestamp: Utc::now(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            cost_usd,
            status: status.to_string(),
            description,
        };

        self.history.add_entry(entry);
        self.save()
    }

    /// Save history to file
    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.history)?;
        fs::write(&self.file_path, json)
            .context("Failed to write cost history file")?;
        Ok(())
    }

    /// Reload history from file
    pub fn reload(&mut self) -> Result<()> {
        if self.file_path.exists() {
            let content = fs::read_to_string(&self.file_path)?;
            self.history = serde_json::from_str(&content)?;
        }
        Ok(())
    }

    /// Get a reference to the history
    pub fn history(&self) -> &CostHistory {
        &self.history
    }

    /// Get a mutable reference to the history
    pub fn history_mut(&mut self) -> &mut CostHistory {
        &mut self.history
    }

    /// Reset (clear) the history
    pub fn reset(&mut self) -> Result<()> {
        self.history.clear();
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_entry(task_id: &str, cost: f64) -> CostEntry {
        CostEntry {
            task_id: task_id.to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: cost,
            status: "completed".to_string(),
            description: Some("Test task".to_string()),
        }
    }

    #[test]
    fn test_cost_history_add_entry() {
        let mut history = CostHistory::new();
        let entry = create_test_entry("task-1", 0.05);

        history.add_entry(entry);

        assert_eq!(history.entries.len(), 1);
        assert!((history.get_total_cost() - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_cost_history_task_total() {
        let mut history = CostHistory::new();

        history.add_entry(create_test_entry("task-1", 0.05));
        history.add_entry(create_test_entry("task-1", 0.03));
        history.add_entry(create_test_entry("task-2", 0.10));

        assert!((history.get_task_total("task-1") - 0.08).abs() < 0.001);
        assert!((history.get_task_total("task-2") - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_cost_history_daily_summary() {
        let mut history = CostHistory::new();

        history.add_entry(create_test_entry("task-1", 0.05));
        history.add_entry(create_test_entry("task-2", 0.10));

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let summary = history.get_daily_summary(&today).unwrap();

        assert!((summary.total_cost_usd - 0.15).abs() < 0.001);
        assert_eq!(summary.task_count, 2);
    }

    #[test]
    fn test_cost_history_manager_save_load() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join(".doodoori");
        fs::create_dir_all(&base_dir).unwrap();

        // Create and save
        {
            let mut manager = CostHistoryManager::new(&base_dir).unwrap();
            manager.record_cost(
                "task-1",
                "sonnet",
                1000,
                500,
                0,
                0,
                0.05,
                "completed",
                Some("Test".to_string()),
            ).unwrap();
        }

        // Load and verify
        {
            let manager = CostHistoryManager::new(&base_dir).unwrap();
            assert_eq!(manager.history().entries.len(), 1);
            assert!((manager.history().get_total_cost() - 0.05).abs() < 0.001);
        }
    }

    #[test]
    fn test_cost_history_manager_reset() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join(".doodoori");
        fs::create_dir_all(&base_dir).unwrap();

        let mut manager = CostHistoryManager::new(&base_dir).unwrap();
        manager.record_cost(
            "task-1", "sonnet", 1000, 500, 0, 0, 0.05, "completed", None
        ).unwrap();

        manager.reset().unwrap();

        assert!(manager.history().entries.is_empty());
        assert!((manager.history().get_total_cost()).abs() < 0.001);
    }

    #[test]
    fn test_cost_history_recent_entries() {
        let mut history = CostHistory::new();

        for i in 0..10 {
            history.add_entry(create_test_entry(&format!("task-{}", i), 0.01));
        }

        let recent = history.get_recent_entries(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].task_id, "task-7");
        assert_eq!(recent[2].task_id, "task-9");
    }

    #[test]
    fn test_cost_history_total_tokens() {
        let mut history = CostHistory::new();

        history.add_entry(create_test_entry("task-1", 0.05));
        history.add_entry(create_test_entry("task-2", 0.05));

        let (input, output) = history.get_total_tokens();
        assert_eq!(input, 2000);
        assert_eq!(output, 1000);
    }
}
