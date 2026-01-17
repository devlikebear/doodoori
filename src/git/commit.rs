//! Conventional Commits support
//!
//! Implements the Conventional Commits specification for structured commit messages.
//! https://www.conventionalcommits.org/

use super::{GitError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

/// Commit types following Conventional Commits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitType {
    /// A new feature
    Feat,
    /// A bug fix
    Fix,
    /// Code refactoring
    Refactor,
    /// Documentation changes
    Docs,
    /// Tests
    Test,
    /// Build/CI changes
    Chore,
    /// Code style changes (formatting, etc.)
    Style,
    /// Performance improvements
    Perf,
    /// Continuous integration
    Ci,
    /// Build system changes
    Build,
    /// Reverts a previous commit
    Revert,
}

impl fmt::Display for CommitType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommitType::Feat => write!(f, "feat"),
            CommitType::Fix => write!(f, "fix"),
            CommitType::Refactor => write!(f, "refactor"),
            CommitType::Docs => write!(f, "docs"),
            CommitType::Test => write!(f, "test"),
            CommitType::Chore => write!(f, "chore"),
            CommitType::Style => write!(f, "style"),
            CommitType::Perf => write!(f, "perf"),
            CommitType::Ci => write!(f, "ci"),
            CommitType::Build => write!(f, "build"),
            CommitType::Revert => write!(f, "revert"),
        }
    }
}

impl FromStr for CommitType {
    type Err = GitError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "feat" | "feature" => Ok(CommitType::Feat),
            "fix" | "bugfix" => Ok(CommitType::Fix),
            "refactor" => Ok(CommitType::Refactor),
            "docs" | "doc" => Ok(CommitType::Docs),
            "test" | "tests" => Ok(CommitType::Test),
            "chore" => Ok(CommitType::Chore),
            "style" => Ok(CommitType::Style),
            "perf" | "performance" => Ok(CommitType::Perf),
            "ci" => Ok(CommitType::Ci),
            "build" => Ok(CommitType::Build),
            "revert" => Ok(CommitType::Revert),
            _ => Err(GitError::OperationFailed(format!(
                "Unknown commit type: {}",
                s
            ))),
        }
    }
}

/// A conventional commit message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConventionalCommit {
    /// The type of commit
    pub commit_type: CommitType,
    /// Optional scope of the change
    pub scope: Option<String>,
    /// Short description
    pub description: String,
    /// Optional longer body
    pub body: Option<String>,
    /// Whether this is a breaking change
    pub breaking: bool,
    /// Optional footer (e.g., "Closes #123")
    pub footer: Option<String>,
}

impl ConventionalCommit {
    /// Create a new conventional commit
    pub fn new(commit_type: CommitType, description: impl Into<String>) -> Self {
        Self {
            commit_type,
            scope: None,
            description: description.into(),
            body: None,
            breaking: false,
            footer: None,
        }
    }

    /// Set the scope
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Set the body
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Mark as breaking change
    pub fn breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Set the footer
    pub fn with_footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// Format the commit message
    pub fn format(&self) -> String {
        let mut message = String::new();

        // Type
        message.push_str(&self.commit_type.to_string());

        // Scope
        if let Some(ref scope) = self.scope {
            message.push('(');
            message.push_str(scope);
            message.push(')');
        }

        // Breaking change indicator
        if self.breaking {
            message.push('!');
        }

        // Description
        message.push_str(": ");
        message.push_str(&self.description);

        // Body
        if let Some(ref body) = self.body {
            message.push_str("\n\n");
            message.push_str(body);
        }

        // Footer
        if let Some(ref footer) = self.footer {
            message.push_str("\n\n");
            message.push_str(footer);
        }

        message
    }
}

impl fmt::Display for ConventionalCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

/// Commit manager for executing git commit operations
pub struct CommitManager {
    /// Working directory
    work_dir: std::path::PathBuf,
}

impl CommitManager {
    /// Create a new commit manager
    pub fn new(work_dir: &Path) -> Self {
        Self {
            work_dir: work_dir.to_path_buf(),
        }
    }

    /// Stage all changes
    pub fn stage_all(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to stage changes: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Stage specific files
    pub fn stage<P: AsRef<Path>>(&self, paths: &[P]) -> Result<()> {
        let path_strs: Vec<&str> = paths
            .iter()
            .map(|p| p.as_ref().to_str().unwrap_or(""))
            .collect();

        let mut args = vec!["add"];
        args.extend(path_strs);

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to stage files: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Check if there are staged changes
    pub fn has_staged_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to check staged changes: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Check if there are any uncommitted changes
    pub fn has_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to check changes: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Create a commit with a conventional commit message
    pub fn commit(&self, message: &ConventionalCommit) -> Result<String> {
        if !self.has_staged_changes()? {
            return Err(GitError::NoStagedChanges);
        }

        let formatted_message = message.format();

        let output = Command::new("git")
            .args(["commit", "-m", &formatted_message])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to commit: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Get the commit hash
        let hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.work_dir)
            .output()?;

        let hash = String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string();

        Ok(hash)
    }

    /// Create a commit with a simple message
    pub fn commit_simple(&self, message: &str) -> Result<String> {
        if !self.has_staged_changes()? {
            return Err(GitError::NoStagedChanges);
        }

        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to commit: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Get the commit hash
        let hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.work_dir)
            .output()?;

        let hash = String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string();

        Ok(hash)
    }

    /// Stage all changes and commit
    pub fn stage_and_commit(&self, message: &ConventionalCommit) -> Result<String> {
        self.stage_all()?;
        self.commit(message)
    }

    /// Get diff summary for staged changes
    pub fn get_staged_diff_summary(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--stat"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to get diff: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get list of staged files
    pub fn get_staged_files(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            return Err(GitError::OperationFailed(format!(
                "Failed to get staged files: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_type_display() {
        assert_eq!(CommitType::Feat.to_string(), "feat");
        assert_eq!(CommitType::Fix.to_string(), "fix");
        assert_eq!(CommitType::Refactor.to_string(), "refactor");
    }

    #[test]
    fn test_commit_type_from_str() {
        assert_eq!(CommitType::from_str("feat").unwrap(), CommitType::Feat);
        assert_eq!(CommitType::from_str("feature").unwrap(), CommitType::Feat);
        assert_eq!(CommitType::from_str("fix").unwrap(), CommitType::Fix);
        assert_eq!(CommitType::from_str("DOCS").unwrap(), CommitType::Docs);
    }

    #[test]
    fn test_conventional_commit_format() {
        let commit = ConventionalCommit::new(CommitType::Feat, "add user authentication");
        assert_eq!(commit.format(), "feat: add user authentication");

        let commit_with_scope =
            ConventionalCommit::new(CommitType::Fix, "handle null case").with_scope("api");
        assert_eq!(commit_with_scope.format(), "fix(api): handle null case");

        let breaking_commit =
            ConventionalCommit::new(CommitType::Feat, "change API response format")
                .with_scope("api")
                .breaking();
        assert_eq!(
            breaking_commit.format(),
            "feat(api)!: change API response format"
        );
    }

    #[test]
    fn test_conventional_commit_with_body() {
        let commit = ConventionalCommit::new(CommitType::Feat, "add login feature")
            .with_body("This adds a new login feature with OAuth support.")
            .with_footer("Closes #123");

        let expected = "feat: add login feature\n\nThis adds a new login feature with OAuth support.\n\nCloses #123";
        assert_eq!(commit.format(), expected);
    }
}
