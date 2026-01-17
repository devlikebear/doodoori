# Doodoori

> Autonomous CLI tool powered by Claude Code for completing goals without user intervention

Named after **Doodoori (두두리, 豆豆里)**, the Silla dynasty's blacksmith deity - just as the ancient craftsman forged metal into tools, Doodoori forges code through persistent iteration until completion.

## Features

- **Self-Improvement Loop**: Runs Claude Code repeatedly until task completion using the Loop Engine
- **Model Selection**: Support for haiku, sonnet, and opus model aliases
- **Budget Tracking**: Track costs with embedded price.toml pricing data
- **Cost History**: Persistent cost tracking with daily/monthly summaries
- **State Management**: Task state persistence for resume capability
- **Resume Support**: Resume interrupted or failed tasks
- **Secrets Management**: .env file support with keychain integration
- **Secret Masking**: Automatic masking of API keys and tokens in logs
- **Configurable**: Project-level configuration via `doodoori.toml`
- **Dry Run Mode**: Preview what would be executed without running
- **Permission Control**: YOLO mode, read-only mode, and custom allowed tools
- **Spec File System**: Markdown-based task specifications with validation
- **Sandbox Mode**: Docker-based isolated execution environment
- **Parallel Execution**: Run multiple tasks concurrently with worker pool
- **Workflow System**: YAML-based complex workflow definitions with DAG scheduling
- **TUI Dashboard**: Real-time monitoring dashboard (optional feature)
- **Git Workflow**: Git worktree support, conventional commits, and PR automation
- **Hooks System**: Execute custom scripts at execution points (pre_run, post_run, on_error, etc.)
- **Notifications**: Send notifications to Slack, Discord, or webhooks on task events

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
```

### First-time Setup (Authentication)

Sandbox mode uses a Docker volume for Claude credentials. This is required for subscription-based authentication (macOS Keychain is not accessible from Docker containers):

```bash
# Login to Claude in sandbox (one-time setup)
doodoori sandbox login

# Or with custom options
doodoori sandbox login --image doodoori/sandbox:latest --volume my-credentials
```

### Running in Sandbox

```bash
# Run in sandbox (after login)
doodoori run --sandbox "Your task here"

# Custom Docker image
doodoori run --sandbox --image my-custom-image:v1 "Task"

# Network modes
doodoori run --sandbox --network bridge "Default networking"
doodoori run --sandbox --network none "No network access"
doodoori run --sandbox --network host "Host networking"
```

### Sandbox Management

```bash
# Check sandbox status
doodoori sandbox status

# Cleanup resources
doodoori sandbox cleanup --volumes    # Remove credentials volume
doodoori sandbox cleanup --containers # Remove containers
doodoori sandbox cleanup --all        # Remove everything
```

### Build the Sandbox Image

```bash
docker build -t doodoori/sandbox:latest -f docker/Dockerfile.sandbox .

# Or using make
make docker-build
```

**Sandbox features:**
- Isolated filesystem (only mounted workspace)
- Docker volume-based Claude credentials (secure for subscription auth)
- Environment variable passing (ANTHROPIC_API_KEY for API key users)
- Optional network isolation
- Non-root execution for security

## Parallel Execution

Run multiple tasks concurrently for faster completion:

```bash
# Run multiple tasks in parallel
doodoori parallel --task "Task A" --task "Task B" --task "Task C"

# With specific worker count
doodoori parallel -w 5 --task "Task 1" --task "Task 2"

# With task isolation (separate workspace per task)
doodoori parallel --isolate --task "Task A" --task "Task B"

# Fail-fast mode (stop all on first failure)
doodoori parallel --fail-fast --task "Critical A" --task "Critical B"

# With model override and budget
doodoori parallel -m opus --budget 10.0 --task "Complex A" --task "Complex B"

# Preview execution plan
doodoori parallel --dry-run --task "Task A" --task "Task B"
```

**Parallel features:**
- Semaphore-based worker pool for controlled concurrency
- Task isolation with separate workspaces
- Real-time progress tracking
- Aggregated cost and result reporting
- Fail-fast mode for critical tasks
- Budget limit across all tasks
- Git worktree mode for branch-based isolation

## Git Workflow

Doodoori supports Git workflow automation with worktrees for parallel development:

```bash
# Initialize git workflow in current directory
doodoori git init

# Create isolated worktrees for parallel tasks
doodoori parallel --git-worktree --task "Backend API" --task "Frontend UI"

# Each task gets its own worktree and branch:
# - .doodoori/worktrees/task-1 (branch: task/backend-api)
# - .doodoori/worktrees/task-2 (branch: task/frontend-ui)

# Run multiple spec files in parallel with git worktrees
doodoori parallel --specs "specs/*.md" --git-worktree --branch-prefix "feature/"

# Example: specs/ folder structure
# specs/
# ├── backend-api.md   → worktree: feature/backend-api
# ├── frontend-ui.md   → worktree: feature/frontend-ui
# └── database.md      → worktree: feature/database

# Manage worktrees manually
doodoori git worktree list
doodoori git worktree add my-feature --prefix "feature/"
doodoori git worktree remove task-123 --delete-branch
doodoori git worktree prune

# Create conventional commits
doodoori git commit -t feat -m "add user authentication" -s api
doodoori git commit -t fix -m "resolve login bug" --breaking

# Manage pull requests (requires gh CLI)
doodoori git pr create --title "Add authentication" --draft
doodoori git pr list --state open
doodoori git pr view 123
doodoori git pr merge 123 --squash

# Branch management
doodoori git branch list
doodoori git branch create feature/new-api --checkout
doodoori git branch task "My New Feature" --prefix "feature/" --checkout

# Check git workflow status
doodoori git status
```

**Git workflow features:**
- Git worktree support for parallel task isolation
- Automatic branch naming with sanitization
- Conventional Commits (feat, fix, refactor, docs, test, chore)
- GitHub PR automation via gh CLI
- Branch management with task-friendly naming

## Workflows

Define complex multi-step workflows with YAML:

```yaml
# workflow.yaml
name: "Full Stack Development"

global:
  default_model: sonnet
  max_parallel_workers: 4
  budget_usd: 20.00

steps:
  - name: "Project Setup"
    prompt: "Initialize TypeScript project"
    model: haiku
    parallel_group: 0
    budget_usd: 1.00

  - name: "Backend API"
    prompt: "Implement REST API"
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Frontend UI"
    prompt: "Create React frontend"
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Integration"
    prompt: "Connect frontend to backend"
    model: haiku
    parallel_group: 2
    depends_on: ["Backend API", "Frontend UI"]
```

```bash
# Run a workflow
doodoori workflow run workflow.yaml

# Preview execution plan
doodoori workflow run --dry-run workflow.yaml

# Validate a workflow
doodoori workflow validate workflow.yaml

# Show workflow details
doodoori workflow info workflow.yaml
```

**Workflow features:**
- YAML-based workflow definitions
- DAG-based dependency resolution
- Parallel group execution
- Per-step model and budget settings
- Circular dependency detection
- Execution plan preview

## TUI Dashboard

Monitor running tasks with the TUI dashboard (requires `dashboard` feature):

```bash
# Build with dashboard support
cargo build --release --features dashboard

# Launch dashboard
doodoori dashboard

# With custom refresh interval
doodoori dashboard --refresh 1000
```

**Dashboard features:**
- Real-time task monitoring
- Cost summary view
- Keyboard navigation (Tab, q to quit)

## Hooks

Execute custom scripts at various points during task execution:

```bash
# Create hook scripts
mkdir -p scripts

# Pre-run hook (runs before task starts)
cat > scripts/pre_run.sh << 'EOF'
#!/bin/bash
echo "Starting task: $DOODOORI_TASK_ID"
echo "Model: $DOODOORI_MODEL"
# Run any setup: backup, lint check, etc.
EOF

# Post-run hook (runs after task completes)
cat > scripts/post_run.sh << 'EOF'
#!/bin/bash
echo "Task finished with status: $DOODOORI_STATUS"
echo "Total cost: $DOODOORI_COST_USD"
# Run any cleanup: notifications, deploy, etc.
EOF

# On-error hook
cat > scripts/on_error.sh << 'EOF'
#!/bin/bash
echo "Error occurred: $DOODOORI_ERROR"
# Send notification, rollback, etc.
EOF

chmod +x scripts/*.sh
```

Configure hooks in `doodoori.toml`:

```toml
[hooks]
enabled = true
timeout_secs = 60
pre_run = "scripts/pre_run.sh"
post_run = "scripts/post_run.sh"
on_error = "scripts/on_error.sh"
on_iteration = "scripts/on_iteration.sh"
on_complete = "scripts/on_complete.sh"
```

**Available hooks:**
- `pre_run`: Before task execution starts
- `post_run`: After task execution completes (success or failure)
- `on_error`: When an error occurs
- `on_iteration`: After each loop iteration
- `on_complete`: When task completes successfully

**Environment variables passed to hooks:**
- `DOODOORI_TASK_ID`: Unique task identifier
- `DOODOORI_PROMPT`: Task prompt (truncated if long)
- `DOODOORI_MODEL`: Model being used
- `DOODOORI_ITERATION`: Current iteration number
- `DOODOORI_TOTAL_ITERATIONS`: Maximum iterations
- `DOODOORI_COST_USD`: Current cost in USD
- `DOODOORI_STATUS`: Task status (starting, running, completed, error)
- `DOODOORI_ERROR`: Error message (for on_error hook)
- `DOODOORI_WORKING_DIR`: Working directory
- `DOODOORI_HOOK_TYPE`: Type of hook being executed

**Disable hooks:**

```bash
# Via CLI flag
doodoori run --no-hooks "Your task"

# Via config
[hooks]
enabled = false
```

## Notifications

Send notifications to Slack, Discord, or any webhook when tasks start, complete, or fail:

```bash
# Enable notifications via CLI (uses doodoori.toml config)
doodoori run --notify "Your task"

# Use a specific webhook URL
doodoori run --notify "https://hooks.slack.com/services/..." "Your task"

# Disable notifications
doodoori run --no-notify "Your task"
```

Configure notifications in `doodoori.toml`:

```toml
[notifications]
enabled = true
# Slack webhook
slack_webhook = "https://hooks.slack.com/services/..."
# Discord webhook
discord_webhook = "https://discord.com/api/webhooks/..."
# Generic webhook
webhook_url = "https://your-api.com/webhook"
# Events to notify on: started, completed, error, budget_exceeded, max_iterations
events = ["completed", "error"]
```

**Notification features:**
- **Slack**: Rich message formatting with attachments (color-coded by status)
- **Discord**: Embed messages with fields for task details
- **Generic Webhook**: JSON POST payload to any endpoint
- Auto-detection of webhook type from URL

**Notification payload:**
- Task ID, prompt, model
- Iterations, cost, duration
- Status (started, completed, error, etc.)
- Error message (if applicable)
- Timestamp

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

[hooks]
enabled = true
timeout_secs = 60
# pre_run = "scripts/pre_run.sh"
# post_run = "scripts/post_run.sh"
# on_error = "scripts/on_error.sh"

[notifications]
enabled = false
# slack_webhook = "https://hooks.slack.com/services/..."
# discord_webhook = "https://discord.com/api/webhooks/..."
# webhook_url = "https://your-api.com/webhook"
events = ["completed", "error"]
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `doodoori run <prompt>` | Run a task with Claude Code |
| `doodoori run --spec <file.md>` | Run from a spec file |
| `doodoori run --sandbox <prompt>` | Run in Docker sandbox |
| `doodoori run --dry-run <prompt>` | Preview execution plan |
| `doodoori parallel --task "A" --task "B"` | Run tasks in parallel |
| `doodoori parallel --specs "*.md"` | Run spec files as parallel tasks |
| `doodoori parallel --specs "*.md" --git-worktree` | Specs with git worktrees |
| `doodoori parallel --isolate --task "A"` | Parallel with task isolation |
| `doodoori parallel --dry-run --task "A"` | Preview parallel execution plan |
| `doodoori workflow run <file.yaml>` | Run a workflow |
| `doodoori workflow run --dry-run <file>` | Preview workflow execution |
| `doodoori workflow validate <file.yaml>` | Validate a workflow |
| `doodoori workflow info <file.yaml>` | Show workflow details |
| `doodoori dashboard` | Launch TUI dashboard |
| `doodoori spec <description>` | Generate a spec file |
| `doodoori spec --validate <file.md>` | Validate a spec file |
| `doodoori spec --info <file.md>` | Show parsed spec information |
| `doodoori sandbox login` | Login to Claude in sandbox |
| `doodoori sandbox status` | Show sandbox status |
| `doodoori sandbox cleanup` | Clean up sandbox resources |
| `doodoori resume --list` | List resumable tasks |
| `doodoori resume <task-id>` | Resume an interrupted task |
| `doodoori cost` | View cost summary |
| `doodoori cost --history` | View full cost history |
| `doodoori cost --daily` | View daily cost summary |
| `doodoori secret set <key>` | Store secret in keychain |
| `doodoori secret get <key>` | Retrieve secret from keychain |
| `doodoori secret list` | List stored secrets |
| `doodoori git init` | Initialize git workflow |
| `doodoori git status` | Show git workflow status |
| `doodoori git worktree list` | List worktrees |
| `doodoori git worktree add <name>` | Create worktree for task |
| `doodoori git worktree remove <id>` | Remove a worktree |
| `doodoori git commit -t <type> -m <msg>` | Create conventional commit |
| `doodoori git pr create` | Create pull request |
| `doodoori git pr list` | List pull requests |
| `doodoori git branch list` | List branches |
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
- [x] Phase 4: State management, secrets, and resume
- [x] Phase 5: Parallel execution
- [x] Phase 6: Workflows and TUI dashboard
- [x] Phase 7: Git workflow and worktree support
- [x] Phase 8: Loop Engine integration (run, resume, parallel, workflow)
- [x] Phase 9: Hooks system (pre_run, post_run, on_error, on_iteration, on_complete)
- [x] Phase 10: Notifications (Slack, Discord, Webhook)

## License

MIT

## Author

devlikebear <devlikebear@gmail.com>
