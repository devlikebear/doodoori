//! Git repository management
#![allow(dead_code)]

use super::{GitError, Result};
use git2::{Repository, StatusOptions};
use std::path::{Path, PathBuf};

/// Git repository wrapper
pub struct GitRepository {
    /// Path to the repository root
    path: PathBuf,
    /// libgit2 repository handle
    repo: Repository,
}

impl GitRepository {
    /// Open an existing git repository
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::open(path)?;
        let path = repo
            .workdir()
            .ok_or_else(|| GitError::NotARepository(path.to_path_buf()))?
            .to_path_buf();

        Ok(Self { path, repo })
    }

    /// Initialize a new git repository
    pub fn init(path: &Path) -> Result<Self> {
        if Self::is_git_repo(path) {
            return Err(GitError::RepositoryExists(path.to_path_buf()));
        }

        let repo = Repository::init(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            repo,
        })
    }

    /// Open or initialize a git repository
    pub fn open_or_init(path: &Path) -> Result<Self> {
        if Self::is_git_repo(path) {
            Self::open(path)
        } else {
            Self::init(path)
        }
    }

    /// Check if a path is a git repository
    pub fn is_git_repo(path: &Path) -> bool {
        Repository::open(path).is_ok()
    }

    /// Get the repository root path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the underlying git2 repository
    pub fn inner(&self) -> &Repository {
        &self.repo
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<String> {
        let head = self.repo.head()?;
        let shorthand = head
            .shorthand()
            .ok_or_else(|| GitError::OperationFailed("Cannot get branch name".to_string()))?;
        Ok(shorthand.to_string())
    }

    /// Check if the working directory is clean (no uncommitted changes)
    pub fn is_clean(&self) -> Result<bool> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        Ok(statuses.is_empty())
    }

    /// Check if a remote exists
    pub fn has_remote(&self, name: &str) -> Result<bool> {
        Ok(self.repo.find_remote(name).is_ok())
    }

    /// Get the default remote name (usually "origin")
    pub fn default_remote(&self) -> Result<Option<String>> {
        let remotes = self.repo.remotes()?;
        if remotes.is_empty() {
            return Ok(None);
        }

        // Try common remote names
        for name in &["origin", "upstream"] {
            if remotes.iter().any(|r| r == Some(*name)) {
                return Ok(Some(name.to_string()));
            }
        }

        // Return the first remote
        Ok(remotes.get(0).map(|s| s.to_string()))
    }

    /// Get the default branch name (usually "main" or "master")
    pub fn default_branch(&self) -> Result<String> {
        // Try to find the default branch from remote
        if let Ok(Some(remote_name)) = self.default_remote() {
            if let Ok(remote) = self.repo.find_remote(&remote_name) {
                if let Ok(default_branch_buf) = remote.default_branch() {
                    if let Some(branch_name) = default_branch_buf.as_str() {
                        // Remove refs/heads/ prefix if present
                        let branch_name = branch_name
                            .strip_prefix("refs/heads/")
                            .unwrap_or(branch_name);
                        return Ok(branch_name.to_string());
                    }
                }
            }
        }

        // Check if common branch names exist
        for name in &["main", "master"] {
            if self
                .repo
                .find_branch(name, git2::BranchType::Local)
                .is_ok()
            {
                return Ok(name.to_string());
            }
        }

        // Default to "main"
        Ok("main".to_string())
    }

    /// Get uncommitted file count
    pub fn uncommitted_count(&self) -> Result<usize> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        Ok(statuses.len())
    }

    /// Get a list of modified files
    pub fn modified_files(&self) -> Result<Vec<String>> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                files.push(path.to_string());
            }
        }

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!GitRepository::is_git_repo(temp_dir.path()));

        // Initialize repo
        Repository::init(temp_dir.path()).unwrap();
        assert!(GitRepository::is_git_repo(temp_dir.path()));
    }

    #[test]
    fn test_open_or_init() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().canonicalize().unwrap();

        // Should init
        let repo = GitRepository::open_or_init(temp_dir.path()).unwrap();
        let repo_path = repo.path().canonicalize().unwrap();
        assert_eq!(repo_path, temp_path);

        // Should open existing
        let repo2 = GitRepository::open_or_init(temp_dir.path()).unwrap();
        let repo2_path = repo2.path().canonicalize().unwrap();
        assert_eq!(repo2_path, temp_path);
    }

    #[test]
    fn test_is_clean() {
        let temp_dir = TempDir::new().unwrap();
        let repo = GitRepository::init(temp_dir.path()).unwrap();

        // Empty repo should be clean
        assert!(repo.is_clean().unwrap());

        // Create a file
        std::fs::write(temp_dir.path().join("test.txt"), "hello").unwrap();

        // Now it should be dirty
        assert!(!repo.is_clean().unwrap());
    }

    #[test]
    fn test_default_branch() {
        let temp_dir = TempDir::new().unwrap();
        let repo = GitRepository::init(temp_dir.path()).unwrap();

        // Default should be "main"
        let default = repo.default_branch().unwrap();
        assert!(default == "main" || default == "master");
    }
}
