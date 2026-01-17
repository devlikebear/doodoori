# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
