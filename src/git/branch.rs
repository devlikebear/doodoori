//! Git branch management
#![allow(dead_code)]

use super::repo::GitRepository;
use super::{sanitize_branch_name, GitError, Result};
use git2::{BranchType, Signature};

/// Branch manager for git operations
pub struct BranchManager<'a> {
    repo: &'a GitRepository,
}

impl<'a> BranchManager<'a> {
    /// Create a new branch manager
    pub fn new(repo: &'a GitRepository) -> Self {
        Self { repo }
    }

    /// Create a new branch from the current HEAD
    pub fn create(&self, name: &str) -> Result<()> {
        self.create_from(name, None)
    }

    /// Create a new branch from a specific commit or branch
    pub fn create_from(&self, name: &str, from: Option<&str>) -> Result<()> {
        if self.exists(name) {
            return Err(GitError::BranchExists(name.to_string()));
        }

        let repo = self.repo.inner();

        // Get the commit to branch from
        let commit = if let Some(ref_name) = from {
            let reference = repo.find_reference(&format!("refs/heads/{}", ref_name))?;
            reference.peel_to_commit()?
        } else {
            // Use HEAD
            let head = repo.head()?;
            head.peel_to_commit()?
        };

        repo.branch(name, &commit, false)?;
        Ok(())
    }

    /// Checkout a branch
    pub fn checkout(&self, name: &str) -> Result<()> {
        if !self.exists(name) {
            return Err(GitError::BranchNotFound(name.to_string()));
        }

        let repo = self.repo.inner();
        let refname = format!("refs/heads/{}", name);

        // Set HEAD to the branch
        repo.set_head(&refname)?;

        // Reset working directory to match
        let obj = repo.revparse_single(&refname)?;
        repo.checkout_tree(&obj, None)?;

        Ok(())
    }

    /// Create and checkout a new branch
    pub fn create_and_checkout(&self, name: &str) -> Result<()> {
        self.create(name)?;
        self.checkout(name)
    }

    /// Delete a branch
    pub fn delete(&self, name: &str, force: bool) -> Result<()> {
        if !self.exists(name) {
            return Err(GitError::BranchNotFound(name.to_string()));
        }

        // Don't delete the current branch
        if self.repo.current_branch()? == name {
            return Err(GitError::OperationFailed(
                "Cannot delete current branch".to_string(),
            ));
        }

        let repo = self.repo.inner();
        let mut branch = repo.find_branch(name, BranchType::Local)?;

        if force {
            branch.delete()?;
        } else {
            // Check if the branch is merged
            let head = repo.head()?.peel_to_commit()?;
            let branch_commit = branch.get().peel_to_commit()?;

            if !repo.merge_base(head.id(), branch_commit.id()).is_ok() {
                return Err(GitError::OperationFailed(
                    "Branch not merged, use force to delete".to_string(),
                ));
            }
            branch.delete()?;
        }

        Ok(())
    }

    /// Check if a branch exists
    pub fn exists(&self, name: &str) -> bool {
        self.repo
            .inner()
            .find_branch(name, BranchType::Local)
            .is_ok()
    }

    /// List all local branches
    pub fn list(&self) -> Result<Vec<String>> {
        let repo = self.repo.inner();
        let branches = repo.branches(Some(BranchType::Local))?;

        let mut result = Vec::new();
        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                result.push(name.to_string());
            }
        }

        Ok(result)
    }

    /// Create a task branch with sanitized name
    pub fn create_task_branch(&self, task_name: &str, prefix: &str) -> Result<String> {
        let sanitized = sanitize_branch_name(task_name);
        let branch_name = format!("{}{}", prefix, sanitized);

        // If branch exists, add a suffix
        let final_name = if self.exists(&branch_name) {
            let mut counter = 1;
            loop {
                let name_with_suffix = format!("{}-{}", branch_name, counter);
                if !self.exists(&name_with_suffix) {
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

        self.create(&final_name)?;
        Ok(final_name)
    }

    /// Create a task branch and checkout
    pub fn create_and_checkout_task_branch(
        &self,
        task_name: &str,
        prefix: &str,
    ) -> Result<String> {
        let branch_name = self.create_task_branch(task_name, prefix)?;
        self.checkout(&branch_name)?;
        Ok(branch_name)
    }

    /// Ensure we have an initial commit (needed for branching in empty repos)
    pub fn ensure_initial_commit(&self) -> Result<()> {
        let repo = self.repo.inner();

        // Check if HEAD exists
        if repo.head().is_ok() {
            return Ok(());
        }

        // Create initial commit
        let sig = Signature::now("doodoori", "doodoori@localhost")?;
        let tree_id = repo.index()?.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, GitRepository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = GitRepository::init(temp_dir.path()).unwrap();

        // Create initial commit
        let manager = BranchManager::new(&repo);
        manager.ensure_initial_commit().unwrap();

        (temp_dir, repo)
    }

    #[test]
    fn test_create_branch() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        manager.create("test-branch").unwrap();
        assert!(manager.exists("test-branch"));
    }

    #[test]
    fn test_create_duplicate_branch() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        manager.create("test-branch").unwrap();
        let result = manager.create("test-branch");
        assert!(matches!(result, Err(GitError::BranchExists(_))));
    }

    #[test]
    fn test_checkout_branch() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        manager.create("test-branch").unwrap();
        manager.checkout("test-branch").unwrap();

        assert_eq!(repo.current_branch().unwrap(), "test-branch");
    }

    #[test]
    fn test_list_branches() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        manager.create("feature-a").unwrap();
        manager.create("feature-b").unwrap();

        let branches = manager.list().unwrap();
        assert!(branches.contains(&"feature-a".to_string()));
        assert!(branches.contains(&"feature-b".to_string()));
    }

    #[test]
    fn test_create_task_branch() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        let name = manager
            .create_task_branch("My New Feature", "feature/")
            .unwrap();
        assert_eq!(name, "feature/my-new-feature");
        assert!(manager.exists(&name));
    }

    #[test]
    fn test_create_task_branch_with_suffix() {
        let (_temp, repo) = setup_repo();
        let manager = BranchManager::new(&repo);

        let name1 = manager.create_task_branch("feature", "task/").unwrap();
        assert_eq!(name1, "task/feature");

        let name2 = manager.create_task_branch("feature", "task/").unwrap();
        assert_eq!(name2, "task/feature-1");

        let name3 = manager.create_task_branch("feature", "task/").unwrap();
        assert_eq!(name3, "task/feature-2");
    }
}
