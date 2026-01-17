# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.0] - 2026-01-17

### Added

- **Hooks System**: Execute custom scripts at various execution points
  - `pre_run`: Before task execution starts
  - `post_run`: After task execution completes (success or failure)
  - `on_error`: When an error occurs during execution
  - `on_iteration`: After each loop iteration
  - `on_complete`: When task completes successfully

- **Hook Configuration**: Configure hooks via doodoori.toml
  - `[hooks]` section with script paths
  - Configurable timeout per hook (default: 60 seconds)
  - `continue_on_failure` option for non-blocking hooks
  - Environment variables passed to hooks (DOODOORI_TASK_ID, DOODOORI_MODEL, etc.)

- **Notifications System**: Send notifications to Slack, Discord, or webhooks
  - Slack notifications with rich message formatting (attachments)
  - Discord notifications with embed support
  - Generic webhook notifications (JSON payload via HTTP POST)
  - Configurable events: `started`, `completed`, `error`, `budget_exceeded`, `max_iterations`
  - Auto-detection of webhook type from URL

- **Notification Configuration**: Configure via doodoori.toml
  - `[notifications]` section with webhook URLs
  - `slack_webhook`, `discord_webhook`, `webhook_url` options
  - `events` array to specify which events trigger notifications
  - Default events: `completed`, `error`

- **CLI Flags**:
  - `--no-hooks`: Disable hooks execution
  - `--notify [url]`: Enable notifications (optional URL for direct webhook)
  - `--no-notify`: Disable notifications
  - `--verbose`: Shows hook execution status

### Changed

- `LoopConfig`: Added `hooks`, `disable_hooks`, `notifications`, `disable_notifications` fields
- `LoopEvent`: Added `HookExecuted` variant for hook events
- `DoodooriConfig`: Added `HooksConfigFile` and `NotificationsConfigFile` for TOML configuration
- Added `tokio/time` feature for hook timeouts
- Added `async-trait` and `reqwest` dependencies for notifications

### Tests

- 208 unit tests (20 new tests for hooks and notifications modules)

## [0.10.0] - 2026-01-17

### Added

- **Workflow Resume**: Resume interrupted or failed workflows
  - `doodoori workflow resume <id>` to continue a workflow
  - `doodoori workflow resume --list` to show resumable workflows
  - Prefix-based workflow ID lookup (e.g., `resume abc` finds `abc123...`)
  - `--from-step` option to resume from a specific step
  - `--yolo` and `--sandbox` flags for resume execution

- **WorkflowStateManager**: Persistent workflow state tracking
  - Workflow states saved to `.doodoori/workflow_states/`
  - Automatic state saving during workflow execution
  - Step-by-step status tracking (Pending, Running, Completed, Failed, Skipped)
  - Completed step preservation during resume
  - Cleanup utilities for old workflow states

### Changed

- `workflow/mod.rs`: Added `workflow_file` field to WorkflowState
- `workflow/mod.rs`: Added `short_id()` and `get_completed_steps()` methods
- `cli/commands/workflow.rs`: Integrated state persistence in run command
- `cli/commands/workflow.rs`: Implemented full resume functionality

### Tests

- 188 unit tests (3 new tests for WorkflowStateManager)

## [0.9.0] - 2026-01-17

### Added

- **Loop Engine Integration**: Full execution pipeline connected
  - `run` command now executes tasks using Loop Engine
  - `resume` command now continues interrupted tasks with Loop Engine
  - `parallel` command uses Loop Engine through ParallelExecutor
  - `workflow` command executes DAG-based workflows with Loop Engine

- **Real-time Progress Display**
  - Progress bar with iteration count and cost tracking
  - Emoji-based status indicators (✅, ⚠️, ❌, etc.)
  - Colored output for success/warning/error states

- **Spec File Execution**
  - `--spec` flag loads spec file and extracts prompt
  - Spec model and max_iterations used as defaults
  - CLI arguments override spec values

- **Config Command**: Display current configuration
  - `doodoori config` shows all settings
  - Shows config file status (loaded or using defaults)
  - Displays [General], [Git], [Logging], [Parallel] sections

- **Price Command**: Display model pricing
  - `doodoori price` shows all model prices
  - `doodoori price --model <name>` shows detailed info
  - Cost examples with token estimates

- **Verbose Output Mode**
  - `--verbose` flag for run command
  - Shows Claude events in real-time (tool calls, results)
  - Session info, assistant messages, tool results

### Changed

- `run.rs`: Integrated with LoopEngine for actual execution
- `resume.rs`: Integrated with LoopEngine for task resumption
- `cli/mod.rs`: Implemented config and price commands
- Added `console` and `indicatif` progress display integration

### Tests

- 185 unit tests (all passing)

## [0.8.0] - 2026-01-17

### Added

- **Git Module**: Comprehensive git workflow automation
  - `GitRepository` for repository management (open, init, status)
  - `BranchManager` for branch operations (create, checkout, delete, list)
  - `WorktreeManager` for git worktree support
  - `CommitManager` for conventional commits
  - `PrManager` for GitHub PR automation via gh CLI

- **Git Worktree Support**: Isolated workspaces for parallel tasks
  - Each parallel task can run in its own git worktree
  - Automatic branch creation with sanitized naming
  - Worktree stored in `.doodoori/worktrees/<task-id>/`
  - `--git-worktree` flag for parallel command
  - `--branch-prefix` for custom branch prefixes
  - `--auto-commit` and `--auto-pr` flags

- **Conventional Commits**: Structured commit messages
  - Support for all commit types (feat, fix, refactor, docs, test, chore, etc.)
  - Scope and breaking change support
  - Body and footer for detailed descriptions

- **PR Automation**: GitHub integration via gh CLI
  - Create, list, view, merge, and close PRs
  - Draft PR support
  - Labels and base branch configuration

- **Git CLI Commands**
  - `doodoori git init` - Initialize git workflow
  - `doodoori git status` - Show workflow status
  - `doodoori git worktree list/add/remove/prune` - Worktree management
  - `doodoori git commit -t <type> -m <msg>` - Conventional commits
  - `doodoori git pr create/list/view/merge/close` - PR management
  - `doodoori git branch list/create/delete/task` - Branch management

- **Spec Files + Git Worktree Integration**
  - `--specs "pattern"` flag for loading multiple spec files via glob pattern
  - Each spec file becomes a separate task with its own worktree
  - Spec file title used for branch naming (sanitized)
  - `--spec` flag for single spec file with multi-task support
  - Automatic branch name generation from spec titles

### Changed

- Added `git2` dependency for native git operations
- Added `glob` dependency for spec file pattern matching
- Extended `ParallelConfig` with git worktree settings
- Extended `TaskDefinition` with optional task name and git branch
- Extended `WorkspaceManager` with worktree support

### Tests

- 185 unit tests (35 new tests for git and specs integration)

## [0.7.0] - 2026-01-17

### Added

- **Workflow System**: YAML-based complex workflow definitions
  - `WorkflowDefinition` with global settings and steps
  - `WorkflowStep` with dependencies, models, and budgets
  - DAG-based scheduler with topological ordering
  - Circular dependency detection
  - Step validation and parallel group execution

- **DAG Scheduler**: Dependency-based task orchestration
  - `DagScheduler` for managing workflow execution order
  - `topological_order()` for correct execution sequence
  - `get_ready_steps()` for parallel-safe step retrieval
  - `get_execution_groups()` for group-based execution

- **Workflow CLI Commands**
  - `doodoori workflow run <file.yaml>` to execute workflows
  - `doodoori workflow run --dry-run` for execution preview
  - `doodoori workflow validate <file.yaml>` for validation
  - `doodoori workflow info <file.yaml>` for details
  - `doodoori workflow resume <id>` for resuming (placeholder)

- **TUI Dashboard** (optional `dashboard` feature)
  - Real-time task monitoring
  - Cost summary view
  - Help tab with keyboard shortcuts
  - Requires `cargo build --features dashboard`

### Changed

- Added `serde_yaml` dependency for workflow parsing

### Tests

- 148 unit tests (10 new tests for workflow module)

## [0.6.0] - 2026-01-17

### Added

- **Parallel Execution**: Run multiple tasks concurrently
  - `ParallelExecutor` with semaphore-based worker pool
  - `TaskDefinition` for configuring individual tasks
  - `ParallelConfig` for executor settings (workers, budget, fail-fast)
  - `ParallelResult` with aggregated results and statistics
  - `ParallelEvent` for real-time progress monitoring

- **Task Isolation**: Separate workspaces for parallel tasks
  - `WorkspaceManager` for creating/cleaning task workspaces
  - Workspaces stored in `.doodoori/workspaces/<task-id>/`
  - `--isolate` flag for enabling task isolation

- **Parallel CLI Improvements**
  - `doodoori parallel --task "A" --task "B"` for multiple tasks
  - `--workers` to control concurrency (default: 3)
  - `--isolate` for task workspace isolation
  - `--fail-fast` to stop on first failure
  - `--max-iterations` per task limit
  - `--yolo` mode for parallel tasks
  - Real-time progress display with task status

### Changed

- Parallel command now uses the new `ParallelExecutor`
- Progress events shown during parallel execution

### Tests

- 138 unit tests (7 new tests for parallel executor)

## [0.5.0] - 2026-01-17

### Added

- **State Management**: Task state persistence for resume capability
  - `TaskState` with UUID-based task identification
  - `StateManager` for persisting state to `.doodoori/state.json`
  - Task history stored in `.doodoori/history/`
  - Automatic state saving at each iteration

- **Resume Command**: Resume interrupted or failed tasks
  - `doodoori resume <task-id>` to continue a task
  - `doodoori resume --list` to show resumable tasks
  - `doodoori resume <task-id> --info` for task details
  - Support for resuming Running, Interrupted, and Failed tasks

- **Secrets Management**: Unified secrets handling
  - `EnvLoader` for .env file support with dotenvy integration
  - Priority order: CLI > .env > System environment
  - `SecretValue` wrapper with automatic masking in debug output
  - `SecretsManager` combining env, .env, and keychain sources

- **Keychain Integration** (optional `keychain` feature)
  - `doodoori secret set/get/delete/list` commands
  - Secure storage using system keychain via keyring crate
  - Service name: "doodoori"

- **Secret Masking**: Automatic masking of sensitive data in logs
  - Patterns for Anthropic, OpenAI, GitHub, AWS API keys
  - Generic API key and Bearer token detection
  - Environment variable value masking

- **Cost History**: Persistent cost tracking
  - `CostEntry` for per-task cost records
  - `CostHistoryManager` saving to `.doodoori/cost_history.json`
  - Daily summaries with model breakdown
  - Monthly totals

- **Cost Command Improvements**
  - `doodoori cost` shows summary (today, month, all-time)
  - `doodoori cost --history` for full history
  - `doodoori cost --daily` for daily breakdown
  - `doodoori cost --task-id <id>` for task-specific costs
  - `doodoori cost --reset` to clear history

- **Loop Engine Integration**
  - Automatic state persistence during execution
  - Cost recording at task completion
  - Task archiving on completion/failure

### Changed

- .env files automatically loaded at startup
- Loop engine now tracks state and records costs

### Tests

- 131 unit tests (28 new tests for state, secrets, cost history)

## [0.4.1] - 2025-01-17

### Added

- **Sandbox Authentication**: Docker volume-based credential storage
  - `doodoori sandbox login` command for interactive authentication
  - `doodoori sandbox status` command to check sandbox state
  - `doodoori sandbox cleanup` command to remove volumes/containers
  - Docker volume `doodoori-claude-credentials` for secure credential storage
  - Support for subscription-based authentication (macOS Keychain workaround)

### Changed

- Sandbox now uses Docker volumes instead of host mount for Claude credentials
- Added `--verbose` flag to Claude CLI calls (required for stream-json output)
- Updated docker-compose.yml to use named volume for credentials
- Default sandbox config now uses Docker volume (recommended for subscription auth)

### Fixed

- Fixed authentication failure in Docker sandbox for subscription users
- Fixed read-only filesystem error with ~/.claude mount
- Fixed Claude CLI stream-json output format requirements

### Tests

- 103 unit tests (6 new tests for sandbox CLI commands)

## [0.4.0] - 2025-01-17

### Added

- **Sandbox Mode**: Docker-based isolated execution environment
  - ContainerManager for Docker API operations using bollard
  - SandboxConfig with builder pattern for configuration
  - SandboxRunner for executing Claude Code in containers
  - Network isolation options: bridge, none, host
  - Automatic credential mounting (~/.claude)
  - Environment variable passing (ANTHROPIC_API_KEY)
  - Dockerfile for sandbox image (docker/Dockerfile.sandbox)
  - docker-compose.yml for easy sandbox management
  - CLI integration: `doodoori run --sandbox`, `--image`, `--network`

### Changed

- Added `--image` and `--network` options to run command
- Updated dry-run to show sandbox configuration

### Tests

- 97 unit tests (5 new tests for sandbox module)

## [0.3.0] - 2025-01-17

### Added

- **Spec File System**: Markdown-based task specifications
  - SpecParser for parsing markdown spec files using pulldown-cmark
  - Support for spec sections: title, objective, model, requirements, constraints, completion criteria, max iterations, completion promise
  - Multi-task specs with Tasks section for parallel execution planning
  - Spec validation with circular dependency detection
  - Spec generator from prompts with to_markdown() serialization
  - CLI integration: `doodoori spec --validate`, `doodoori spec --info`

### Changed

- Updated CLI spec command with parser integration

### Tests

- 92 unit tests (20 new tests for spec module)

## [0.2.0] - 2025-01-17

### Added

- **CLI Skeleton**: Complete command-line interface with clap
  - `run` command for executing tasks
  - `parallel` command for concurrent execution
  - `spec` command for generating/validating spec files
  - `cost` command for tracking expenses
  - `config` and `price` commands for viewing settings

- **Claude CLI Wrapper**: Integration with Claude Code CLI
  - Stream-JSON output parsing
  - Event types: System, Assistant, ToolUse, ToolResult, Result
  - Usage tracking (tokens, cost, duration)
  - Support for YOLO mode, read-only mode, and custom allowed tools

- **Loop Engine**: Self-improvement loop implementation
  - Configurable completion detection strategies (Promise, AnyOf, Regex)
  - Iteration tracking with budget and max iteration limits
  - Progressive prompt building for continuation

- **Pricing Module**: Cost tracking with price.toml
  - Model alias resolution (haiku, sonnet, opus)
  - Cost calculation from token usage
  - Embedded default pricing data

- **Configuration System**: Project-level settings via doodoori.toml
  - Model, budget, and iteration settings
  - Git workflow automation options
  - Parallel execution configuration

### Tests

- 72 unit tests covering all modules

## [0.1.0] - 2025-01-17

### Added

- Initial project setup
- Project planning documentation (PLANNING.md)
- Price configuration (price.toml)
- Development guidelines (CLAUDE.md)
