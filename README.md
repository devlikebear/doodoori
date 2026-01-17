# Doodoori

> Autonomous CLI tool powered by Claude Code for completing goals without user intervention

Named after **Doodoori (두두리, 豆豆里)**, the Silla dynasty's blacksmith deity - just as the ancient craftsman forged metal into tools, Doodoori forges code through persistent iteration until completion.

## Features

- **Self-Improvement Loop**: Runs Claude Code repeatedly until task completion using the Loop Engine
- **Model Selection**: Support for haiku, sonnet, and opus model aliases
- **Budget Tracking**: Track costs with embedded price.toml pricing data
- **Configurable**: Project-level configuration via `doodoori.toml`
- **Dry Run Mode**: Preview what would be executed without running
- **Permission Control**: YOLO mode, read-only mode, and custom allowed tools
- **Spec File System**: Markdown-based task specifications with validation
- **Sandbox Mode**: Docker-based isolated execution environment

## Installation

```bash
# Build from source
cargo build --release

# Install globally
cargo install --path .
```

## Quick Start

```bash
# Run a simple task
doodoori run "Create a hello world REST API in Rust"

# Use a specific model
doodoori run -m opus "Complex architecture design task"

# Dry run to preview
doodoori run --dry-run "Refactor the authentication module"

# With budget limit
doodoori run --budget 5.0 "Implement user dashboard"

# YOLO mode (skip all permissions)
doodoori run --yolo "Quick task with full permissions"

# Run in Docker sandbox (requires --features sandbox)
doodoori run --sandbox "Potentially risky operation"

# Sandbox with network isolation
doodoori run --sandbox --network none "Completely isolated execution"
```

## Sandbox Mode

Sandbox mode runs Claude Code inside a Docker container for isolated execution:

```bash
# Build with sandbox support
cargo build --release --features sandbox

# Run in sandbox
doodoori run --sandbox "Your task here"

# Custom Docker image
doodoori run --sandbox --image my-custom-image:v1 "Task"

# Network modes
doodoori run --sandbox --network bridge "Default networking"
doodoori run --sandbox --network none "No network access"
doodoori run --sandbox --network host "Host networking"
```

**Build the sandbox image:**
```bash
docker build -t doodoori/sandbox:latest -f docker/Dockerfile.sandbox .
```

**Sandbox features:**
- Isolated filesystem (only mounted workspace)
- Automatic Claude credentials mounting (~/.claude)
- Environment variable passing (ANTHROPIC_API_KEY)
- Optional network isolation
- Non-root execution for security

## Configuration

Create a `doodoori.toml` in your project root:

```toml
# Default model: haiku, sonnet, or opus
default_model = "sonnet"

# Maximum iterations for the loop engine
max_iterations = 50

# Budget limit in USD (optional)
# budget_limit = 10.0

# Enable YOLO mode by default
yolo_mode = false

[git]
enabled = true
auto_branch = true
auto_commit = true

[parallel]
workers = 3
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `doodoori run <prompt>` | Run a task with Claude Code |
| `doodoori run --spec <file.md>` | Run from a spec file |
| `doodoori run --sandbox <prompt>` | Run in Docker sandbox |
| `doodoori run --dry-run <prompt>` | Preview execution plan |
| `doodoori spec <description>` | Generate a spec file |
| `doodoori spec --validate <file.md>` | Validate a spec file |
| `doodoori spec --info <file.md>` | Show parsed spec information |
| `doodoori cost` | View cost tracking |
| `doodoori config` | Show configuration |
| `doodoori price` | Show model pricing |

## Model Pricing

| Model | Input/MTok | Output/MTok | Best For |
|-------|------------|-------------|----------|
| Haiku | $1.00 | $5.00 | Quick tasks, simple operations |
| Sonnet | $3.00 | $15.00 | Balanced performance, general use |
| Opus | $5.00 | $25.00 | Complex reasoning, high-quality output |

## Spec Files

Spec files are markdown documents that define tasks for Doodoori:

```markdown
# Task: Build REST API

## Objective
Create a REST API for todo management

## Model
sonnet

## Requirements
- [ ] GET /todos endpoint
- [ ] POST /todos endpoint
- [ ] DELETE /todos endpoint

## Constraints
- Use Rust and Axum framework
- Include error handling

## Completion Criteria
All endpoints working with tests

## Max Iterations
30
```

Generate a spec file:
```bash
doodoori spec "Build a REST API for todos" -o api-spec.md
```

Validate a spec file:
```bash
doodoori spec --validate api-spec.md
```

## Roadmap

- [x] Phase 1: MVP - Basic execution with Loop Engine
- [x] Phase 2: Spec file system with markdown parsing
- [x] Phase 3: Sandbox mode with Docker
- [ ] Phase 4: State management and resume
- [ ] Phase 5: Parallel execution
- [ ] Phase 6: Workflows and TUI dashboard

## License

MIT

## Author

devlikebear <devlikebear@gmail.com>
