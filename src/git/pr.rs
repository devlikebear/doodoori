//! Pull Request management using gh CLI
//!
//! Provides PR operations via GitHub CLI (gh).

use super::{GitError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Pull request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    /// PR title
    pub title: String,
    /// PR body/description
    pub body: String,
    /// Base branch (e.g., "main")
    pub base: String,
    /// Head branch (e.g., "feature/my-feature")
    pub head: String,
    /// Whether to create as draft
    pub draft: bool,
    /// Labels to add
    pub labels: Vec<String>,
}

impl PullRequest {
    /// Create a new pull request
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            base: "main".to_string(),
            head: String::new(),
            draft: false,
            labels: Vec::new(),
        }
    }

    /// Set the base branch
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// Set the head branch
    pub fn with_head(mut self, head: impl Into<String>) -> Self {
        self.head = head.into();
        self
    }

    /// Mark as draft
    pub fn as_draft(mut self) -> Self {
        self.draft = true;
        self
    }

    /// Add labels
    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }
}

/// PR information from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestInfo {
    /// PR number
    pub number: u32,
    /// PR title
    pub title: String,
    /// PR state (open, closed, merged)
    pub state: String,
    /// PR URL
    pub url: String,
    /// Head branch
    pub head_branch: String,
    /// Base branch
    pub base_branch: String,
    /// Whether it's a draft
    pub draft: bool,
}

/// PR manager using gh CLI
pub struct PrManager {
    /// Working directory (git repo root)
    work_dir: std::path::PathBuf,
}

impl PrManager {
    /// Create a new PR manager
    pub fn new(work_dir: &Path) -> Self {
        Self {
            work_dir: work_dir.to_path_buf(),
        }
    }

    /// Check if gh CLI is available
    pub fn is_gh_available() -> bool {
        Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if authenticated with GitHub
    pub fn is_authenticated(&self) -> Result<bool> {
        let output = Command::new("gh")
            .args(["auth", "status"])
            .current_dir(&self.work_dir)
            .output()?;

        Ok(output.status.success())
    }

    /// Create a pull request
    pub async fn create(&self, pr: &PullRequest) -> Result<String> {
        if !Self::is_gh_available() {
            return Err(GitError::GhCliNotAvailable);
        }

        let mut args = vec![
            "pr".to_string(),
            "create".to_string(),
            "--title".to_string(),
            pr.title.clone(),
            "--body".to_string(),
            pr.body.clone(),
            "--base".to_string(),
            pr.base.clone(),
        ];

        if !pr.head.is_empty() {
            args.push("--head".to_string());
            args.push(pr.head.clone());
        }

        if pr.draft {
            args.push("--draft".to_string());
        }

        for label in &pr.labels {
            args.push("--label".to_string());
            args.push(label.clone());
        }

        let output = Command::new("gh")
            .args(&args)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GhCliError(stderr.to_string()));
        }

        // gh pr create returns the PR URL
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(url)
    }

    /// Merge a pull request
    pub async fn merge(&self, pr_number: u32, squash: bool) -> Result<()> {
        if !Self::is_gh_available() {
            return Err(GitError::GhCliNotAvailable);
        }

        let mut args = vec![
            "pr".to_string(),
            "merge".to_string(),
            pr_number.to_string(),
            "--delete-branch".to_string(),
        ];

        if squash {
            args.push("--squash".to_string());
        } else {
            args.push("--merge".to_string());
        }

        let output = Command::new("gh")
            .args(&args)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GhCliError(stderr.to_string()));
        }

        Ok(())
    }

    /// List pull requests
    pub async fn list(&self, state: Option<&str>) -> Result<Vec<PullRequestInfo>> {
        if !Self::is_gh_available() {
            return Err(GitError::GhCliNotAvailable);
        }

        let mut args = vec![
            "pr".to_string(),
            "list".to_string(),
            "--json".to_string(),
            "number,title,state,url,headRefName,baseRefName,isDraft".to_string(),
        ];

        if let Some(state) = state {
            args.push("--state".to_string());
            args.push(state.to_string());
        }

        let output = Command::new("gh")
            .args(&args)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GhCliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse JSON response
        #[derive(Deserialize)]
        struct GhPr {
            number: u32,
            title: String,
            state: String,
            url: String,
            #[serde(rename = "headRefName")]
            head_ref_name: String,
            #[serde(rename = "baseRefName")]
            base_ref_name: String,
            #[serde(rename = "isDraft")]
            is_draft: bool,
        }

        let prs: Vec<GhPr> = serde_json::from_str(&stdout).map_err(|e| {
            GitError::OperationFailed(format!("Failed to parse PR list: {}", e))
        })?;

        Ok(prs
            .into_iter()
            .map(|pr| PullRequestInfo {
                number: pr.number,
                title: pr.title,
                state: pr.state,
                url: pr.url,
                head_branch: pr.head_ref_name,
                base_branch: pr.base_ref_name,
                draft: pr.is_draft,
            })
            .collect())
    }

    /// View a specific pull request
    pub async fn view(&self, pr_number: u32) -> Result<PullRequestInfo> {
        if !Self::is_gh_available() {
            return Err(GitError::GhCliNotAvailable);
        }

        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                &pr_number.to_string(),
                "--json",
                "number,title,state,url,headRefName,baseRefName,isDraft",
            ])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GhCliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        #[derive(Deserialize)]
        struct GhPr {
            number: u32,
            title: String,
            state: String,
            url: String,
            #[serde(rename = "headRefName")]
            head_ref_name: String,
            #[serde(rename = "baseRefName")]
            base_ref_name: String,
            #[serde(rename = "isDraft")]
            is_draft: bool,
        }

        let pr: GhPr = serde_json::from_str(&stdout)
            .map_err(|e| GitError::OperationFailed(format!("Failed to parse PR: {}", e)))?;

        Ok(PullRequestInfo {
            number: pr.number,
            title: pr.title,
            state: pr.state,
            url: pr.url,
            head_branch: pr.head_ref_name,
            base_branch: pr.base_ref_name,
            draft: pr.is_draft,
        })
    }

    /// Close a pull request
    pub async fn close(&self, pr_number: u32) -> Result<()> {
        if !Self::is_gh_available() {
            return Err(GitError::GhCliNotAvailable);
        }

        let output = Command::new("gh")
            .args(["pr", "close", &pr_number.to_string()])
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::GhCliError(stderr.to_string()));
        }

        Ok(())
    }

    /// Push current branch to remote
    pub fn push(&self, set_upstream: bool) -> Result<()> {
        let mut args = vec!["push".to_string()];

        if set_upstream {
            args.push("-u".to_string());
            args.push("origin".to_string());
            args.push("HEAD".to_string());
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.work_dir)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::OperationFailed(format!(
                "Failed to push: {}",
                stderr
            )));
        }

        Ok(())
    }
}

/// Generate PR body from task information
pub fn generate_pr_body(task_name: &str, description: Option<&str>, changes: &[String]) -> String {
    let mut body = String::new();

    body.push_str("## Summary\n\n");
    body.push_str(&format!("Task: {}\n\n", task_name));

    if let Some(desc) = description {
        body.push_str(desc);
        body.push_str("\n\n");
    }

    if !changes.is_empty() {
        body.push_str("## Changes\n\n");
        for change in changes {
            body.push_str(&format!("- {}\n", change));
        }
        body.push('\n');
    }

    body.push_str("## Test Plan\n\n");
    body.push_str("- [ ] Manual testing\n");
    body.push_str("- [ ] Unit tests\n");

    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pull_request_builder() {
        let pr = PullRequest::new("Add feature", "Description")
            .with_base("main")
            .with_head("feature/test")
            .as_draft()
            .with_labels(vec!["enhancement".to_string()]);

        assert_eq!(pr.title, "Add feature");
        assert_eq!(pr.body, "Description");
        assert_eq!(pr.base, "main");
        assert_eq!(pr.head, "feature/test");
        assert!(pr.draft);
        assert_eq!(pr.labels, vec!["enhancement"]);
    }

    #[test]
    fn test_generate_pr_body() {
        let body = generate_pr_body(
            "Implement login",
            Some("Added OAuth support"),
            &["auth.rs".to_string(), "login.rs".to_string()],
        );

        assert!(body.contains("Task: Implement login"));
        assert!(body.contains("OAuth support"));
        assert!(body.contains("- auth.rs"));
        assert!(body.contains("- login.rs"));
    }

    #[test]
    fn test_gh_available() {
        // Just test that the function doesn't panic
        let _ = PrManager::is_gh_available();
    }
}
