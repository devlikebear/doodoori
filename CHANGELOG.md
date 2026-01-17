# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
