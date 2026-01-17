# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
