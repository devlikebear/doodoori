use anyhow::Result;
use clap::{Args, Subcommand};
use std::str::FromStr;

use crate::git::{
    branch::BranchManager,
    commit::{CommitManager, CommitType, ConventionalCommit},
    pr::{PrManager, PullRequest},
    repo::GitRepository,
    worktree::WorktreeManager,
};

/// Git workflow management commands
#[derive(Args, Debug)]
pub struct GitArgs {
    #[command(subcommand)]
    pub command: GitCommand,
}

#[derive(Subcommand, Debug)]
pub enum GitCommand {
    /// Initialize git workflow in current directory
    Init,

    /// Manage git worktrees for parallel tasks
    Worktree(WorktreeArgs),

    /// Create a conventional commit
    Commit(CommitArgs),

    /// Create and manage pull requests
    Pr(PrArgs),

    /// Manage branches
    Branch(BranchArgs),

    /// Show git workflow status
    Status,
}

/// Worktree management commands
#[derive(Args, Debug)]
pub struct WorktreeArgs {
    #[command(subcommand)]
    pub command: WorktreeCommand,
}

#[derive(Subcommand, Debug)]
pub enum WorktreeCommand {
    /// List all worktrees
    List,

    /// Add a new worktree for a task
    Add {
        /// Task name (will be sanitized for branch name)
        task_name: String,

        /// Branch prefix (default: "task/")
        #[arg(long, default_value = "task/")]
        prefix: String,
    },

    /// Remove a worktree
    Remove {
        /// Task ID to remove
        task_id: String,

        /// Also delete the branch
        #[arg(long)]
        delete_branch: bool,
    },

    /// Prune stale worktrees
    Prune,
}

/// Commit commands
#[derive(Args, Debug)]
pub struct CommitArgs {
    /// Commit message (if not using conventional commit flags)
    #[arg(short, long)]
    pub message: Option<String>,

    /// Commit type (feat, fix, refactor, docs, test, chore, style, perf, ci, build, revert)
    #[arg(short = 't', long)]
    pub commit_type: Option<String>,

    /// Scope of the commit (e.g., api, ui, core)
    #[arg(short, long)]
    pub scope: Option<String>,

    /// Mark as breaking change
    #[arg(long)]
    pub breaking: bool,

    /// Commit body (detailed description)
    #[arg(short, long)]
    pub body: Option<String>,

    /// Stage all changes before commit
    #[arg(short, long)]
    pub all: bool,

    /// Dry run - show what would be committed
    #[arg(long)]
    pub dry_run: bool,
}

/// Pull request commands
#[derive(Args, Debug)]
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCommand,
}

#[derive(Subcommand, Debug)]
pub enum PrCommand {
    /// Create a new pull request
    Create {
        /// PR title
        #[arg(short, long)]
        title: String,

        /// PR body/description
        #[arg(short, long)]
        body: Option<String>,

        /// Base branch (default: main)
        #[arg(long, default_value = "main")]
        base: String,

        /// Create as draft PR
        #[arg(long)]
        draft: bool,

        /// Labels to add
        #[arg(long)]
        labels: Vec<String>,
    },

    /// List pull requests
    List {
        /// Filter by state (open, closed, merged, all)
        #[arg(long, default_value = "open")]
        state: String,
    },

    /// View a pull request
    View {
        /// PR number
        number: u32,
    },

    /// Merge a pull request
    Merge {
        /// PR number
        number: u32,

        /// Use squash merge
        #[arg(long)]
        squash: bool,
    },

    /// Close a pull request without merging
    Close {
        /// PR number
        number: u32,
    },
}

/// Branch management commands
#[derive(Args, Debug)]
pub struct BranchArgs {
    #[command(subcommand)]
    pub command: BranchCommand,
}

#[derive(Subcommand, Debug)]
pub enum BranchCommand {
    /// List branches
    List,

    /// Create a new branch
    Create {
        /// Branch name
        name: String,

        /// Create from this branch
        #[arg(long)]
        from: Option<String>,

        /// Switch to the new branch after creation
        #[arg(long)]
        checkout: bool,
    },

    /// Delete a branch
    Delete {
        /// Branch name
        name: String,

        /// Force delete even if not merged
        #[arg(short, long)]
        force: bool,
    },

    /// Create a task branch with proper naming
    Task {
        /// Task name (will be sanitized)
        name: String,

        /// Branch prefix (default: "feature/")
        #[arg(long, default_value = "feature/")]
        prefix: String,

        /// Switch to the new branch after creation
        #[arg(long)]
        checkout: bool,
    },
}

impl GitArgs {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            GitCommand::Init => execute_init().await,
            GitCommand::Worktree(args) => args.execute().await,
            GitCommand::Commit(args) => args.execute().await,
            GitCommand::Pr(args) => args.execute().await,
            GitCommand::Branch(args) => args.execute().await,
            GitCommand::Status => execute_status().await,
        }
    }
}

async fn execute_init() -> Result<()> {
    let cwd = std::env::current_dir()?;

    if GitRepository::is_git_repo(&cwd) {
        println!("✓ Already a git repository");
    } else {
        GitRepository::init(&cwd)?;
        println!("✓ Initialized git repository");
    }

    // Create .doodoori directory structure
    let doodoori_dir = cwd.join(".doodoori");
    if !doodoori_dir.exists() {
        std::fs::create_dir_all(&doodoori_dir)?;
        println!("✓ Created .doodoori directory");
    }

    let worktrees_dir = doodoori_dir.join("worktrees");
    if !worktrees_dir.exists() {
        std::fs::create_dir_all(&worktrees_dir)?;
        println!("✓ Created worktrees directory");
    }

    // Add .doodoori to .gitignore if not already there
    let gitignore = cwd.join(".gitignore");
    let needs_update = if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore)?;
        !content.lines().any(|line| line.trim() == ".doodoori/")
    } else {
        true
    };

    if needs_update {
        let mut content = if gitignore.exists() {
            std::fs::read_to_string(&gitignore)?
        } else {
            String::new()
        };

        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        content.push_str("\n# Doodoori working directory\n.doodoori/\n");
        std::fs::write(&gitignore, content)?;
        println!("✓ Added .doodoori/ to .gitignore");
    }

    println!("\nGit workflow initialized successfully!");
    println!("You can now use:");
    println!("  doodoori git worktree add <task-name>  - Create isolated worktree for a task");
    println!("  doodoori git commit -t feat -m \"message\"  - Create conventional commit");
    println!("  doodoori git pr create --title \"PR title\"  - Create pull request");

    Ok(())
}

async fn execute_status() -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Check if git repo
    if !GitRepository::is_git_repo(&cwd) {
        println!("Not a git repository. Run 'doodoori git init' first.");
        return Ok(());
    }

    let repo = GitRepository::open(&cwd)?;

    println!("=== Git Workflow Status ===\n");

    // Repository info
    println!("[Repository]");
    if let Ok(branch) = repo.current_branch() {
        println!("  Current branch: {}", branch);
    }
    if let Ok(default) = repo.default_branch() {
        println!("  Default branch: {}", default);
    }
    if let Ok(remote) = repo.default_remote() {
        if let Some(r) = remote {
            println!("  Remote: {}", r);
        } else {
            println!("  Remote: (none)");
        }
    }

    // Working directory status
    println!("\n[Working Directory]");
    if let Ok(clean) = repo.is_clean() {
        if clean {
            println!("  Status: Clean");
        } else {
            if let Ok(count) = repo.uncommitted_count() {
                println!("  Status: {} uncommitted changes", count);
            }
            if let Ok(files) = repo.modified_files() {
                for file in files.iter().take(5) {
                    println!("    - {}", file);
                }
                if files.len() > 5 {
                    println!("    ... and {} more", files.len() - 5);
                }
            }
        }
    }

    // Worktrees
    let worktrees_dir = cwd.join(".doodoori").join("worktrees");
    let worktree_manager = WorktreeManager::new(&cwd, &worktrees_dir)?;
    if let Ok(worktrees) = worktree_manager.list() {
        if !worktrees.is_empty() {
            println!("\n[Worktrees]");
            for wt in &worktrees {
                println!("  {} (branch: {})", wt.task_id, wt.branch);
                println!("    Path: {}", wt.path.display());
            }
        } else {
            println!("\n[Worktrees]");
            println!("  No active worktrees");
        }
    }

    // gh CLI status
    println!("\n[GitHub CLI]");
    if PrManager::is_gh_available() {
        println!("  Status: Available");
        let pr_manager = PrManager::new(&cwd);
        if let Ok(auth) = pr_manager.is_authenticated() {
            println!("  Authenticated: {}", if auth { "Yes" } else { "No" });
        }
    } else {
        println!("  Status: Not installed");
        println!("  Install: https://cli.github.com/");
    }

    Ok(())
}

impl WorktreeArgs {
    pub async fn execute(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let worktrees_dir = cwd.join(".doodoori").join("worktrees");
        let manager = WorktreeManager::new(&cwd, &worktrees_dir)?;

        match self.command {
            WorktreeCommand::List => {
                let worktrees = manager.list()?;

                if worktrees.is_empty() {
                    println!("No worktrees found.");
                    println!("\nCreate one with: doodoori git worktree add <task-name>");
                } else {
                    println!("=== Worktrees ({}) ===\n", worktrees.len());
                    for wt in worktrees {
                        println!("Task ID: {}", wt.task_id);
                        println!("  Branch: {}", wt.branch);
                        println!("  Path: {}", wt.path.display());
                        println!("  Created: {}", wt.created_at.format("%Y-%m-%d %H:%M:%S"));
                        println!();
                    }
                }
            }
            WorktreeCommand::Add { task_name, prefix } => {
                let task_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
                println!("Creating worktree for task: {}", task_name);

                let worktree = manager.create_for_task(&task_id, &task_name, &prefix)?;

                println!("\n✓ Worktree created successfully!");
                println!("  Task ID: {}", worktree.task_id);
                println!("  Branch: {}", worktree.branch);
                println!("  Path: {}", worktree.path.display());
                println!("\nTo work in this worktree:");
                println!("  cd {}", worktree.path.display());
            }
            WorktreeCommand::Remove { task_id, delete_branch } => {
                println!("Removing worktree: {}", task_id);

                // First get worktree info
                if let Some(wt) = manager.get(&task_id)? {
                    manager.remove(&task_id)?;
                    println!("✓ Worktree removed");

                    if delete_branch {
                        let repo = GitRepository::open(&cwd)?;
                        let branch_manager = BranchManager::new(&repo);
                        if let Err(e) = branch_manager.delete(&wt.branch, true) {
                            println!("Warning: Could not delete branch '{}': {}", wt.branch, e);
                        } else {
                            println!("✓ Branch '{}' deleted", wt.branch);
                        }
                    }
                } else {
                    println!("Worktree not found: {}", task_id);
                }
            }
            WorktreeCommand::Prune => {
                println!("Pruning stale worktrees...");
                manager.prune()?;
                println!("✓ Done");
            }
        }

        Ok(())
    }
}

impl CommitArgs {
    pub async fn execute(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let commit_manager = CommitManager::new(&cwd);

        // Stage all if requested
        if self.all {
            commit_manager.stage_all()?;
            println!("✓ Staged all changes");
        }

        // Check for staged changes
        if !commit_manager.has_staged_changes()? {
            if commit_manager.has_changes()? {
                println!("No staged changes. Use -a to stage all, or stage changes manually.");
            } else {
                println!("No changes to commit.");
            }
            return Ok(());
        }

        // Show what will be committed
        if self.dry_run {
            println!("=== Dry Run - Would commit: ===\n");
            let files = commit_manager.get_staged_files()?;
            for file in &files {
                println!("  {}", file);
            }
            println!();
        }

        // Build commit message
        let commit_msg = if let Some(ref commit_type_str) = self.commit_type {
            // Conventional commit
            let commit_type = CommitType::from_str(commit_type_str)
                .unwrap_or(CommitType::Chore);

            let description = self.message.clone()
                .unwrap_or_else(|| "update".to_string());

            let mut commit = ConventionalCommit::new(commit_type, description);

            if let Some(ref scope) = self.scope {
                commit = commit.with_scope(scope.clone());
            }
            if self.breaking {
                commit = commit.breaking();
            }
            if let Some(ref body) = self.body {
                commit = commit.with_body(body.clone());
            }

            commit
        } else if let Some(ref message) = self.message {
            // Simple message - wrap as chore
            ConventionalCommit::new(CommitType::Chore, message.clone())
        } else {
            println!("Please provide a commit message with -m or use --commit-type for conventional commits.");
            return Ok(());
        };

        let formatted = commit_msg.format();

        if self.dry_run {
            println!("Commit message:\n  {}", formatted);
            return Ok(());
        }

        // Create the commit
        let oid = commit_manager.commit(&commit_msg)?;
        println!("✓ Created commit: {} ({})", &oid[..8], formatted);

        Ok(())
    }
}

impl PrArgs {
    pub async fn execute(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let pr_manager = PrManager::new(&cwd);

        // Check gh availability
        if !PrManager::is_gh_available() {
            println!("GitHub CLI (gh) is not installed.");
            println!("Install it from: https://cli.github.com/");
            return Ok(());
        }

        match self.command {
            PrCommand::Create { title, body, base, draft, labels } => {
                let mut pr = PullRequest::new(&title, body.unwrap_or_default())
                    .with_base(&base);

                if draft {
                    pr = pr.as_draft();
                }
                if !labels.is_empty() {
                    pr = pr.with_labels(labels);
                }

                println!("Creating pull request...");

                // First push to remote
                println!("Pushing to remote...");
                pr_manager.push(true)?;

                let url = pr_manager.create(&pr).await?;
                println!("\n✓ Pull request created!");
                println!("  URL: {}", url);
            }
            PrCommand::List { state } => {
                let state_filter = if state == "all" { None } else { Some(state.as_str()) };
                let prs = pr_manager.list(state_filter).await?;

                if prs.is_empty() {
                    println!("No pull requests found.");
                } else {
                    println!("=== Pull Requests ({}) ===\n", prs.len());
                    for pr in prs {
                        let state_icon = match pr.state.as_str() {
                            "open" => "🟢",
                            "closed" => "🔴",
                            "merged" => "🟣",
                            _ => "⚪",
                        };
                        println!("{} #{} {}", state_icon, pr.number, pr.title);
                        println!("   {} -> {}", pr.head_branch, pr.base_branch);
                        if !pr.url.is_empty() {
                            println!("   {}", pr.url);
                        }
                        println!();
                    }
                }
            }
            PrCommand::View { number } => {
                let pr = pr_manager.view(number).await?;

                println!("=== PR #{} ===\n", pr.number);
                println!("Title: {}", pr.title);
                println!("State: {}", pr.state);
                println!("Branch: {} -> {}", pr.head_branch, pr.base_branch);
                if pr.draft {
                    println!("Draft: Yes");
                }
                if !pr.url.is_empty() {
                    println!("URL: {}", pr.url);
                }
            }
            PrCommand::Merge { number, squash } => {
                println!("Merging PR #{}...", number);
                pr_manager.merge(number, squash).await?;
                println!("✓ PR #{} merged successfully!", number);
            }
            PrCommand::Close { number } => {
                println!("Closing PR #{}...", number);
                pr_manager.close(number).await?;
                println!("✓ PR #{} closed.", number);
            }
        }

        Ok(())
    }
}

impl BranchArgs {
    pub async fn execute(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let repo = GitRepository::open(&cwd)?;
        let branch_manager = BranchManager::new(&repo);

        match self.command {
            BranchCommand::List => {
                let branches = branch_manager.list()?;

                if branches.is_empty() {
                    println!("No branches found.");
                } else {
                    let current = repo.current_branch().unwrap_or_default();
                    println!("=== Branches ({}) ===\n", branches.len());
                    for branch in branches {
                        if branch == current {
                            println!("* {} (current)", branch);
                        } else {
                            println!("  {}", branch);
                        }
                    }
                }
            }
            BranchCommand::Create { name, from, checkout } => {
                if checkout {
                    branch_manager.create_and_checkout(&name)?;
                    println!("✓ Created and switched to branch '{}'", name);
                } else {
                    branch_manager.create_from(&name, from.as_deref())?;
                    println!("✓ Created branch '{}'", name);
                }
            }
            BranchCommand::Delete { name, force } => {
                branch_manager.delete(&name, force)?;
                println!("✓ Deleted branch '{}'", name);
            }
            BranchCommand::Task { name, prefix, checkout } => {
                let branch_name = if checkout {
                    branch_manager.create_and_checkout_task_branch(&name, &prefix)?
                } else {
                    branch_manager.create_task_branch(&name, &prefix)?
                };
                println!("✓ Created task branch '{}'", branch_name);
                if checkout {
                    println!("  Switched to branch");
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: TestCommand,
    }

    #[derive(Subcommand)]
    enum TestCommand {
        Git(GitArgs),
    }

    #[test]
    fn test_git_init() {
        let cli = TestCli::try_parse_from(["test", "git", "init"]).unwrap();
        match cli.command {
            TestCommand::Git(args) => {
                matches!(args.command, GitCommand::Init);
            }
        }
    }

    #[test]
    fn test_git_worktree_list() {
        let cli = TestCli::try_parse_from(["test", "git", "worktree", "list"]).unwrap();
        match cli.command {
            TestCommand::Git(args) => {
                match args.command {
                    GitCommand::Worktree(wt) => {
                        matches!(wt.command, WorktreeCommand::List);
                    }
                    _ => panic!("Expected Worktree command"),
                }
            }
        }
    }

    #[test]
    fn test_git_worktree_add() {
        let cli = TestCli::try_parse_from([
            "test", "git", "worktree", "add", "my-task", "--prefix", "feature/"
        ]).unwrap();

        match cli.command {
            TestCommand::Git(args) => {
                match args.command {
                    GitCommand::Worktree(wt) => {
                        match wt.command {
                            WorktreeCommand::Add { task_name, prefix } => {
                                assert_eq!(task_name, "my-task");
                                assert_eq!(prefix, "feature/");
                            }
                            _ => panic!("Expected Add command"),
                        }
                    }
                    _ => panic!("Expected Worktree command"),
                }
            }
        }
    }

    #[test]
    fn test_git_commit_conventional() {
        let cli = TestCli::try_parse_from([
            "test", "git", "commit",
            "-t", "feat",
            "-m", "add new feature",
            "-s", "api",
            "--breaking"
        ]).unwrap();

        match cli.command {
            TestCommand::Git(args) => {
                match args.command {
                    GitCommand::Commit(commit) => {
                        assert_eq!(commit.commit_type, Some("feat".to_string()));
                        assert_eq!(commit.message, Some("add new feature".to_string()));
                        assert_eq!(commit.scope, Some("api".to_string()));
                        assert!(commit.breaking);
                    }
                    _ => panic!("Expected Commit command"),
                }
            }
        }
    }

    #[test]
    fn test_git_pr_create() {
        let cli = TestCli::try_parse_from([
            "test", "git", "pr", "create",
            "--title", "My PR",
            "--body", "Description",
            "--draft"
        ]).unwrap();

        match cli.command {
            TestCommand::Git(args) => {
                match args.command {
                    GitCommand::Pr(pr) => {
                        match pr.command {
                            PrCommand::Create { title, body, draft, .. } => {
                                assert_eq!(title, "My PR");
                                assert_eq!(body, Some("Description".to_string()));
                                assert!(draft);
                            }
                            _ => panic!("Expected Create command"),
                        }
                    }
                    _ => panic!("Expected Pr command"),
                }
            }
        }
    }

    #[test]
    fn test_git_branch_task() {
        let cli = TestCli::try_parse_from([
            "test", "git", "branch", "task", "My Feature Task", "--checkout"
        ]).unwrap();

        match cli.command {
            TestCommand::Git(args) => {
                match args.command {
                    GitCommand::Branch(branch) => {
                        match branch.command {
                            BranchCommand::Task { name, prefix, checkout } => {
                                assert_eq!(name, "My Feature Task");
                                assert_eq!(prefix, "feature/");
                                assert!(checkout);
                            }
                            _ => panic!("Expected Task command"),
                        }
                    }
                    _ => panic!("Expected Branch command"),
                }
            }
        }
    }
}
