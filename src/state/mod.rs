//! State management module for task persistence and resume functionality

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::claude::ExecutionUsage;

/// Task execution status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is created but not started
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was interrupted (can be resumed)
    Interrupted,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Interrupted => write!(f, "interrupted"),
        }
    }
}

/// Token usage statistics (serializable version)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
}

impl From<&ExecutionUsage> for TokenUsage {
    fn from(usage: &ExecutionUsage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_creation_tokens: usage.cache_creation_tokens,
            cache_read_tokens: usage.cache_read_tokens,
        }
    }
}

/// Represents the state of a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    /// Unique task identifier (UUID v4)
    pub task_id: String,
    /// The prompt or spec file path used
    pub prompt: String,
    /// Model used for execution
    pub model: String,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Current status
    pub status: TaskStatus,
    /// Current iteration number
    pub current_iteration: u32,
    /// Maximum iterations allowed
    pub max_iterations: u32,
    /// Token usage statistics
    pub usage: TokenUsage,
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Claude session ID for resume
    pub session_id: Option<String>,
    /// Working directory
    pub working_dir: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Final output if completed
    pub final_output: Option<String>,
}

impl TaskState {
    /// Create a new task state
    pub fn new(prompt: String, model: String, max_iterations: u32) -> Self {
        let now = Utc::now();
        Self {
            task_id: Uuid::new_v4().to_string(),
            prompt,
            model,
            created_at: now,
            updated_at: now,
            status: TaskStatus::Pending,
            current_iteration: 0,
            max_iterations,
            usage: TokenUsage::default(),
            total_cost_usd: 0.0,
            duration_ms: 0,
            session_id: None,
            working_dir: None,
            error: None,
            final_output: None,
        }
    }

    /// Mark task as running
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.updated_at = Utc::now();
    }

    /// Update iteration count
    pub fn update_iteration(&mut self, iteration: u32) {
        self.current_iteration = iteration;
        self.updated_at = Utc::now();
    }

    /// Update usage statistics
    pub fn update_usage(&mut self, usage: &ExecutionUsage) {
        self.usage = TokenUsage::from(usage);
        self.total_cost_usd = usage.total_cost_usd;
        self.duration_ms = usage.duration_ms;
        self.updated_at = Utc::now();
    }

    /// Set session ID for resume
    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
        self.updated_at = Utc::now();
    }

    /// Mark task as completed
    pub fn complete(&mut self, output: Option<String>) {
        self.status = TaskStatus::Completed;
        self.final_output = output;
        self.updated_at = Utc::now();
    }

    /// Mark task as failed
    pub fn fail(&mut self, error: String) {
        self.status = TaskStatus::Failed;
        self.error = Some(error);
        self.updated_at = Utc::now();
    }

    /// Mark task as interrupted (resumable)
    pub fn interrupt(&mut self) {
        self.status = TaskStatus::Interrupted;
        self.updated_at = Utc::now();
    }

    /// Check if task can be resumed
    pub fn can_resume(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Running | TaskStatus::Interrupted | TaskStatus::Failed
        )
    }

    /// Get short task ID (first 8 chars)
    pub fn short_id(&self) -> &str {
        &self.task_id[..8.min(self.task_id.len())]
    }
}

/// State manager for persisting task states
pub struct StateManager {
    /// Base directory for state files (.doodoori)
    base_dir: PathBuf,
}

impl StateManager {
    /// Create a new state manager for the given directory
    pub fn new(project_dir: &Path) -> Result<Self> {
        let base_dir = project_dir.join(".doodoori");
        fs::create_dir_all(&base_dir).context("Failed to create .doodoori directory")?;
        fs::create_dir_all(base_dir.join("history"))
            .context("Failed to create history directory")?;
        Ok(Self { base_dir })
    }

    /// Get the path to the current state file
    fn state_file_path(&self) -> PathBuf {
        self.base_dir.join("state.json")
    }

    /// Get the path to a task history file
    fn history_file_path(&self, task_id: &str) -> PathBuf {
        self.base_dir.join("history").join(format!("{}.json", task_id))
    }

    /// Save the current task state
    pub fn save_state(&self, state: &TaskState) -> Result<()> {
        let path = self.state_file_path();
        let json = serde_json::to_string_pretty(state)?;
        fs::write(&path, json).context("Failed to write state file")?;
        tracing::debug!("Saved task state to {:?}", path);
        Ok(())
    }

    /// Load the current task state
    pub fn load_state(&self) -> Result<Option<TaskState>> {
        let path = self.state_file_path();
        if !path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(&path).context("Failed to read state file")?;
        let state: TaskState = serde_json::from_str(&json).context("Failed to parse state file")?;
        Ok(Some(state))
    }

    /// Archive a completed task to history
    pub fn archive_task(&self, state: &TaskState) -> Result<()> {
        let path = self.history_file_path(&state.task_id);
        let json = serde_json::to_string_pretty(state)?;
        fs::write(&path, json).context("Failed to write history file")?;
        tracing::debug!("Archived task {} to history", state.short_id());

        // Remove current state file
        let state_path = self.state_file_path();
        if state_path.exists() {
            fs::remove_file(&state_path).ok();
        }
        Ok(())
    }

    /// Load a task from history
    pub fn load_from_history(&self, task_id: &str) -> Result<Option<TaskState>> {
        // Try exact match first
        let path = self.history_file_path(task_id);
        if path.exists() {
            let json = fs::read_to_string(&path)?;
            let state: TaskState = serde_json::from_str(&json)?;
            return Ok(Some(state));
        }

        // Try prefix match
        let history_dir = self.base_dir.join("history");
        if history_dir.exists() {
            for entry in fs::read_dir(&history_dir)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with(task_id) && name.ends_with(".json") {
                    let json = fs::read_to_string(entry.path())?;
                    let state: TaskState = serde_json::from_str(&json)?;
                    return Ok(Some(state));
                }
            }
        }

        Ok(None)
    }

    /// List all resumable tasks (current + interrupted in history)
    pub fn list_resumable_tasks(&self) -> Result<Vec<TaskState>> {
        let mut tasks = Vec::new();

        // Check current state
        if let Some(state) = self.load_state()? {
            if state.can_resume() {
                tasks.push(state);
            }
        }

        // Check history for interrupted tasks
        let history_dir = self.base_dir.join("history");
        if history_dir.exists() {
            for entry in fs::read_dir(&history_dir)? {
                let entry = entry?;
                if entry.path().extension().map_or(false, |e| e == "json") {
                    let json = fs::read_to_string(entry.path())?;
                    if let Ok(state) = serde_json::from_str::<TaskState>(&json) {
                        if state.can_resume() {
                            tasks.push(state);
                        }
                    }
                }
            }
        }

        // Sort by updated_at descending
        tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(tasks)
    }

    /// List recent task history
    pub fn list_history(&self, limit: usize) -> Result<Vec<TaskState>> {
        let mut tasks = Vec::new();
        let history_dir = self.base_dir.join("history");

        if history_dir.exists() {
            for entry in fs::read_dir(&history_dir)? {
                let entry = entry?;
                if entry.path().extension().map_or(false, |e| e == "json") {
                    let json = fs::read_to_string(entry.path())?;
                    if let Ok(state) = serde_json::from_str::<TaskState>(&json) {
                        tasks.push(state);
                    }
                }
            }
        }

        // Sort by updated_at descending
        tasks.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        tasks.truncate(limit);
        Ok(tasks)
    }

    /// Clear current state
    pub fn clear_state(&self) -> Result<()> {
        let path = self.state_file_path();
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// Get the base directory path
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_task_state_new() {
        let state = TaskState::new(
            "Test prompt".to_string(),
            "sonnet".to_string(),
            50,
        );
        assert_eq!(state.status, TaskStatus::Pending);
        assert_eq!(state.current_iteration, 0);
        assert_eq!(state.max_iterations, 50);
        assert!(state.task_id.len() == 36); // UUID format
    }

    #[test]
    fn test_task_state_lifecycle() {
        let mut state = TaskState::new(
            "Test prompt".to_string(),
            "sonnet".to_string(),
            50,
        );

        state.start();
        assert_eq!(state.status, TaskStatus::Running);

        state.update_iteration(5);
        assert_eq!(state.current_iteration, 5);

        state.complete(Some("Done".to_string()));
        assert_eq!(state.status, TaskStatus::Completed);
        assert_eq!(state.final_output, Some("Done".to_string()));
    }

    #[test]
    fn test_task_state_can_resume() {
        let mut state = TaskState::new("test".to_string(), "sonnet".to_string(), 50);
        assert!(!state.can_resume()); // Pending

        state.start();
        assert!(state.can_resume()); // Running

        state.interrupt();
        assert!(state.can_resume()); // Interrupted

        state.complete(None);
        assert!(!state.can_resume()); // Completed
    }

    #[test]
    fn test_state_manager_save_load() {
        let dir = tempdir().unwrap();
        let manager = StateManager::new(dir.path()).unwrap();

        let state = TaskState::new(
            "Test prompt".to_string(),
            "sonnet".to_string(),
            50,
        );

        manager.save_state(&state).unwrap();
        let loaded = manager.load_state().unwrap().unwrap();

        assert_eq!(loaded.task_id, state.task_id);
        assert_eq!(loaded.prompt, state.prompt);
    }

    #[test]
    fn test_state_manager_archive() {
        let dir = tempdir().unwrap();
        let manager = StateManager::new(dir.path()).unwrap();

        let mut state = TaskState::new(
            "Test prompt".to_string(),
            "sonnet".to_string(),
            50,
        );
        state.complete(Some("Done".to_string()));

        manager.save_state(&state).unwrap();
        manager.archive_task(&state).unwrap();

        // Current state should be cleared
        assert!(manager.load_state().unwrap().is_none());

        // Should be in history
        let loaded = manager.load_from_history(&state.task_id).unwrap().unwrap();
        assert_eq!(loaded.task_id, state.task_id);
    }

    #[test]
    fn test_state_manager_prefix_lookup() {
        let dir = tempdir().unwrap();
        let manager = StateManager::new(dir.path()).unwrap();

        let mut state = TaskState::new(
            "Test prompt".to_string(),
            "sonnet".to_string(),
            50,
        );
        state.complete(None);
        manager.archive_task(&state).unwrap();

        // Should find by prefix
        let short_id = state.short_id().to_string();
        let loaded = manager.load_from_history(&short_id).unwrap().unwrap();
        assert_eq!(loaded.task_id, state.task_id);
    }
}
