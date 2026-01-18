//! Git worktree management for parallel task isolation
//!
//! Provides worktree operations for isolating parallel tasks into
//! separate working directories with their own branches.
#![allow(dead_code)]

use super::branch::BranchManager;
use super::repo::GitRepository;
use super::{sanitize_branch_name, GitError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git worktree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Branch name associated with this worktree
    pub branch: String,
    /// Task ID associated with this worktree
    pub task_id: String,
    /// When the worktree was created
    pub created_at: DateTime<Utc>,
}

/// Manager for git worktrees
pub struct WorktreeManager {
    /// Path to the main repository
    main_repo_path: PathBuf,
    /// Directory where worktrees are stored
    worktrees_dir: PathBuf,
}

impl WorktreeManager {
    /// Create a new worktree manager
    pub fn new(repo_path: &Path, worktrees_dir: &Path) -> Result<Self> {
        if !GitRepository::is_git_repo(repo_path) {
            return Err(GitError::NotARepository(repo_path.to_path_buf()));
        }

        Ok(Self {
            main_repo_path: repo_path.to_path_buf(),
            worktrees_dir: worktrees_dir.to_path_buf(),
        })
    }

    /// Create a new worktree manager with default worktrees directory
    pub fn with_default_dir(repo_path: &Path) -> Result<Self> {
        let worktrees_dir = repo_path.join(".doodoori").join("worktrees");
        Self::new(repo_path, &worktrees_dir)
    }

    /// Create a worktree for a task
    ///
    /// Creates a new branch and worktree for the given task.
    /// The worktree is created at `worktrees_dir/{task_id}` with branch `{prefix}{sanitized_name}`.
    pub fn create_for_task(
        &self,
        task_id: &str,
        task_name: &str,
        branch_prefix: &str,
    ) -> Result<Worktree> {
        let worktree_path = self.worktrees_dir.join(task_id);

        // Check if worktree already exists
        if worktree_path.exists() {
            return Err(GitError::WorktreeExists(task_id.to_string()));
        }

        // Ensure worktrees directory exists
        std::fs::create_dir_all(&self.worktrees_dir)?;

        // Create sanitized branch name
        let sanitized_name = sanitize_branch_name(task_name);
        let branch_name = format!("{}{}", branch_prefix, sanitized_name);

        // Ensure unique branch name
        let repo = GitRepository::open(&self.main_repo_path)?;
        let branch_manager = BranchManager::new(&repo);

        // Ensure initial commit exists
        branch_manager.ensure_initial_commit()?;

        let final_branch_name = if branch_manager.exists(&branch_name) {
            // Find unique name
            let mut counter = 1;
            loop {
                let name_with_suffix = format!("{}-{}", branch_name, counter);
                if !branch_manager.exists(&name_with_suffix) {
                    break name_with_suffix;
                }
                counter += 1;
                if counter > 100 {
                    return Err(GitError::OperationFailed(
                        "Too many branches with same name".to_string(),
                    ));
                }
            }
        } else {
            branch_name
        };

        // Use git CLI for worktree operations (more reliable than libgit2)
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &final_branch_name,
                worktree_path.to_str().unwrap(),
            ])
            .current_dir(&self.main_repo_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(format!(
                "Failed to create worktree: {}",
                stderr
            )));
        }

        Ok(Worktree {
            path: worktree_path,
            branch: final_branch_name,
            task_id: task_id.to_string(),
            created_at: Utc::now(),
        })
    }

    /// List all worktrees managed by this manager
    pub fn list(&self) -> Result<Vec<Worktree>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&self.main_repo_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(format!(
                "Failed to list worktrees: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut worktrees = Vec::new();
        let mut current_path: Option<PathBuf> = None;
        let mut current_branch: Option<String> = None;

        // Canonicalize worktrees_dir for consistent comparison
        let canonical_worktrees_dir = self.worktrees_dir.canonicalize().unwrap_or_else(|_| self.worktrees_dir.clone());

        for line in stdout.lines() {
            if line.starts_with("worktree ") {
                // Save previous worktree if any
                if let (Some(path), Some(branch)) = (current_path.take(), current_branch.take()) {
                    // Only include worktrees in our worktrees directory
                    // Canonicalize for consistent comparison (handles macOS /private symlinks)
                    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
                    if canonical_path.starts_with(&canonical_worktrees_dir) {
                        let task_id = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        worktrees.push(Worktree {
                            path,
                            branch,
                            task_id,
                            created_at: Utc::now(), // We don't have actual creation time
                        });
                    }
                }
                current_path = Some(PathBuf::from(line.strip_prefix("worktree ").unwrap()));
            } else if line.starts_with("branch ") {
                let branch = line.strip_prefix("branch refs/heads/").unwrap_or(
                    line.strip_prefix("branch ").unwrap_or(line),
                );
                current_branch = Some(branch.to_string());
            }
        }

        // Don't forget the last one
        if let (Some(path), Some(branch)) = (current_path, current_branch) {
            let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if canonical_path.starts_with(&canonical_worktrees_dir) {
                let task_id = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                worktrees.push(Worktree {
                    path,
                    branch,
                    task_id,
                    created_at: Utc::now(),
                });
            }
        }

        Ok(worktrees)
    }

    /// Remove a worktree by task ID
    pub fn remove(&self, task_id: &str) -> Result<()> {
        let worktree_path = self.worktrees_dir.join(task_id);

        if !worktree_path.exists() {
            return Err(GitError::WorktreeNotFound(task_id.to_string()));
        }

        // Remove the worktree
        let output = Command::new("git")
            .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
            .current_dir(&self.main_repo_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(format!(
                "Failed to remove worktree: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Remove a worktree and optionally delete its branch
    pub fn remove_with_branch(&self, task_id: &str, delete_branch: bool) -> Result<()> {
        // Get branch name before removing
        let branch_name = if delete_branch {
            self.list()?
                .into_iter()
                .find(|w| w.task_id == task_id)
                .map(|w| w.branch)
        } else {
            None
        };

        // Remove worktree
        self.remove(task_id)?;

        // Delete branch if requested
        if let Some(branch) = branch_name {
            let output = Command::new("git")
                .args(["branch", "-D", &branch])
                .current_dir(&self.main_repo_path)
                .output()?;

            if !output.status.success() {
                // Log but don't fail
                tracing::warn!(
                    "Failed to delete branch {}: {}",
                    branch,
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        Ok(())
    }

    /// Prune stale worktree entries
    pub fn prune(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(&self.main_repo_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(format!(
                "Failed to prune worktrees: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get a specific worktree by task ID
    pub fn get(&self, task_id: &str) -> Result<Option<Worktree>> {
        Ok(self.list()?.into_iter().find(|w| w.task_id == task_id))
    }

    /// Check if a worktree exists for a task ID
    pub fn exists(&self, task_id: &str) -> bool {
        self.worktrees_dir.join(task_id).exists()
    }

    /// Get the path where a worktree would be created
    pub fn worktree_path(&self, task_id: &str) -> PathBuf {
        self.worktrees_dir.join(task_id)
    }

    /// Cleanup all worktrees managed by this manager
    pub fn cleanup_all(&self) -> Result<()> {
        let worktrees = self.list()?;

        for worktree in worktrees {
            if let Err(e) = self.remove(&worktree.task_id) {
                tracing::warn!("Failed to remove worktree {}: {}", worktree.task_id, e);
            }
        }

        // Prune any stale entries
        self.prune()?;

        // Try to remove the worktrees directory if empty
        if self.worktrees_dir.exists() {
            let _ = std::fs::remove_dir(&self.worktrees_dir);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_create_worktree() {
        let (_temp, repo_path) = setup_repo();
        let manager = WorktreeManager::with_default_dir(&repo_path).unwrap();

        let worktree = manager
            .create_for_task("task-123", "My Feature", "feature/")
            .unwrap();

        assert!(worktree.path.exists());
        assert_eq!(worktree.branch, "feature/my-feature");
        assert_eq!(worktree.task_id, "task-123");
    }

    #[test]
    fn test_list_worktrees() {
        let (_temp, repo_path) = setup_repo();
        let manager = WorktreeManager::with_default_dir(&repo_path).unwrap();

        manager
            .create_for_task("task-1", "Feature A", "feature/")
            .unwrap();
        manager
            .create_for_task("task-2", "Feature B", "feature/")
            .unwrap();

        let worktrees = manager.list().unwrap();
        assert_eq!(worktrees.len(), 2);
    }

    #[test]
    fn test_remove_worktree() {
        let (_temp, repo_path) = setup_repo();
        let manager = WorktreeManager::with_default_dir(&repo_path).unwrap();

        let worktree = manager
            .create_for_task("task-123", "My Feature", "feature/")
            .unwrap();

        assert!(worktree.path.exists());

        manager.remove("task-123").unwrap();

        assert!(!manager.worktree_path("task-123").exists());
    }

    #[test]
    fn test_exists() {
        let (_temp, repo_path) = setup_repo();
        let manager = WorktreeManager::with_default_dir(&repo_path).unwrap();

        assert!(!manager.exists("task-123"));

        manager
            .create_for_task("task-123", "My Feature", "feature/")
            .unwrap();

        assert!(manager.exists("task-123"));
    }
}
