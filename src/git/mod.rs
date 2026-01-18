//! Git workflow and worktree management module
#![allow(dead_code)]
//!
//! Provides comprehensive git operations including:
//! - Repository management (init, open, status)
//! - Branch management (create, checkout, delete)
//! - Worktree management for parallel task isolation
//! - Conventional commit support
//! - Pull request creation via gh CLI

pub mod branch;
pub mod commit;
pub mod pr;
pub mod repo;
pub mod worktree;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Git workflow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Enable git workflow
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Auto-initialize git repo if not present
    #[serde(default = "default_true")]
    pub auto_init: bool,

    /// Branch prefix for feature branches
    #[serde(default = "default_feature_prefix")]
    pub branch_prefix_feature: String,

    /// Branch prefix for fix branches
    #[serde(default = "default_fix_prefix")]
    pub branch_prefix_fix: String,

    /// Commit message style
    #[serde(default)]
    pub commit_style: CommitStyle,

    /// Auto-commit on task completion
    #[serde(default)]
    pub auto_commit: bool,

    /// Auto-create PR on task completion
    #[serde(default)]
    pub auto_pr: bool,

    /// Use git worktrees for task isolation
    #[serde(default)]
    pub use_worktrees: bool,

    /// Worktrees directory (relative to repo root)
    #[serde(default = "default_worktrees_dir")]
    pub worktrees_dir: PathBuf,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_init: true,
            branch_prefix_feature: "feature/".to_string(),
            branch_prefix_fix: "fix/".to_string(),
            commit_style: CommitStyle::default(),
            auto_commit: false,
            auto_pr: false,
            use_worktrees: false,
            worktrees_dir: PathBuf::from(".doodoori/worktrees"),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_feature_prefix() -> String {
    "feature/".to_string()
}

fn default_fix_prefix() -> String {
    "fix/".to_string()
}

fn default_worktrees_dir() -> PathBuf {
    PathBuf::from(".doodoori/worktrees")
}

/// Commit message style
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CommitStyle {
    /// Conventional Commits (feat:, fix:, etc.)
    #[default]
    Conventional,
    /// Simple commit messages
    Simple,
}

/// Git operation errors
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Not a git repository: {0}")]
    NotARepository(PathBuf),

    #[error("Git repository already exists: {0}")]
    RepositoryExists(PathBuf),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Branch already exists: {0}")]
    BranchExists(String),

    #[error("Worktree not found: {0}")]
    WorktreeNotFound(String),

    #[error("Worktree already exists: {0}")]
    WorktreeExists(String),

    #[error("Working directory not clean")]
    NotClean,

    #[error("No staged changes")]
    NoStagedChanges,

    #[error("Git operation failed: {0}")]
    OperationFailed(String),

    #[error("gh CLI not available")]
    GhCliNotAvailable,

    #[error("gh CLI error: {0}")]
    GhCliError(String),

    #[error("Git2 error: {0}")]
    Git2(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GitError>;

/// Sanitize a task name for use in branch names
/// Converts spaces to hyphens, removes special characters, converts to lowercase
pub fn sanitize_branch_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(|c| c == '-' || c == '_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_branch_name() {
        assert_eq!(sanitize_branch_name("My Feature"), "my-feature");
        assert_eq!(sanitize_branch_name("fix-bug-123"), "fix-bug-123");
        assert_eq!(
            sanitize_branch_name("Add user authentication"),
            "add-user-authentication"
        );
        assert_eq!(sanitize_branch_name("  spaced  "), "spaced");
        assert_eq!(
            sanitize_branch_name("special!@#$chars"),
            "special____chars"
        );
    }

    #[test]
    fn test_git_config_default() {
        let config = GitConfig::default();
        assert!(config.enabled);
        assert!(config.auto_init);
        assert_eq!(config.branch_prefix_feature, "feature/");
        assert_eq!(config.branch_prefix_fix, "fix/");
        assert_eq!(config.commit_style, CommitStyle::Conventional);
        assert!(!config.auto_commit);
        assert!(!config.auto_pr);
        assert!(!config.use_worktrees);
    }
}
