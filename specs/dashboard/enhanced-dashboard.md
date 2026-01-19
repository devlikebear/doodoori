# Enhanced Dashboard with Task Details and Log Viewer

## Objective

Enhance the TUI dashboard to support:
1. Task detail view with all stored information
2. Log viewing for completed tasks (historical)
3. Real-time log streaming for running tasks (tail -f style)

## Requirements

### 1. Log Storage (src/claude/runner.rs)

Store execution logs to `.doodoori/logs/{task_id}.log`:

```rust
// In ClaudeRunner, add log file writing
fn write_to_log(&self, task_id: &str, line: &str) -> Result<()> {
    let log_dir = PathBuf::from(".doodoori/logs");
    fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!("{}.log", task_id));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}
```

Log format:
```
[2026-01-18T12:30:00Z] [INFO] Starting task...
[2026-01-18T12:30:01Z] [CLAUDE] I'll analyze the codebase...
[2026-01-18T12:30:05Z] [TOOL] Read: src/main.rs
[2026-01-18T12:30:10Z] [CLAUDE] Based on my analysis...
```

### 2. Dashboard Enhancement (src/cli/commands/dashboard.rs)

#### 2.1 New App State

```rust
pub struct App {
    // Existing fields...

    /// Selected task index (for navigation)
    pub selected_task: usize,
    /// List of all tasks (active + recent)
    pub tasks: Vec<TaskState>,
    /// Current view mode
    pub view_mode: ViewMode,
    /// Log content for selected task
    pub log_content: Vec<String>,
    /// Log scroll position
    pub log_scroll: usize,
    /// Is log auto-scrolling (for real-time)
    pub log_auto_scroll: bool,
}

pub enum ViewMode {
    TaskList,
    TaskDetail,
    LogView,
}
```

#### 2.2 New Keyboard Controls

| Key | Action |
|-----|--------|
| `↑/↓` | Navigate task list |
| `Enter` | View task details |
| `l` | View logs |
| `Esc` | Back to list |
| `f` | Toggle auto-scroll (in log view) |
| `PgUp/PgDown` | Scroll logs |

#### 2.3 Task Detail View

```
┌─ Task Details ─────────────────────────────────────────┐
│ ID:       1e05b32b-c532-4a97-a30c-3a64dbb61e4a        │
│ Status:   Running                                      │
│ Model:    sonnet                                       │
│ Started:  2026-01-18 21:28:52                         │
│ Duration: 5m 32s                                       │
│                                                        │
│ Progress: [████████░░░░░░░░░░░░] 8/50 iterations      │
│                                                        │
│ Tokens:   Input: 18,791  Output: 13,146               │
│           Cache Write: 208,912  Cache Read: 2,185,066 │
│ Cost:     $1.7053                                      │
│                                                        │
│ Prompt:                                                │
│ ─────────────────────────────────────────────────────  │
│ # Task: Dead Code Cleanup                              │
│ ## Objective                                           │
│ Remove #![allow(dead_code)] module-level directives... │
│                                                        │
│ [Press 'l' for logs, 'Esc' to go back]                │
└────────────────────────────────────────────────────────┘
```

#### 2.4 Log View

```
┌─ Logs: 1e05b32b (Running - Auto-scroll ON) ────────────┐
│ [12:30:00] Starting task...                            │
│ [12:30:01] [CLAUDE] I'll analyze the codebase first.   │
│ [12:30:02] [TOOL] Read: src/main.rs                    │
│ [12:30:03] [TOOL] Read: src/lib.rs                     │
│ [12:30:05] [CLAUDE] Based on my analysis, I found...   │
│ [12:30:10] [TOOL] Edit: src/config/mod.rs              │
│ [12:30:12] [CLAUDE] Now let me verify the changes...   │
│ [12:30:15] [TOOL] Bash: cargo build                    │
│ ▼ (auto-scrolling)                                     │
│                                                        │
│ [Press 'f' to toggle auto-scroll, 'Esc' to go back]   │
└────────────────────────────────────────────────────────┘
```

### 3. Real-time Log Streaming

For running tasks, implement file watching:

```rust
fn load_log_content(&mut self, task_id: &str) -> Result<()> {
    let log_path = format!(".doodoori/logs/{}.log", task_id);
    if Path::new(&log_path).exists() {
        let content = fs::read_to_string(&log_path)?;
        self.log_content = content.lines().map(String::from).collect();
        if self.log_auto_scroll {
            self.log_scroll = self.log_content.len().saturating_sub(1);
        }
    }
    Ok(())
}
```

Refresh log content on each tick for running tasks.

### 4. Task History

Load recent tasks from `.doodoori/history/` directory:

```rust
fn load_task_history(&mut self) -> Result<Vec<TaskState>> {
    let history_dir = PathBuf::from(".doodoori/history");
    let mut tasks = Vec::new();

    if history_dir.exists() {
        for entry in fs::read_dir(&history_dir)? {
            let path = entry?.path();
            if path.extension() == Some("json".as_ref()) {
                let content = fs::read_to_string(&path)?;
                if let Ok(task) = serde_json::from_str::<TaskState>(&content) {
                    tasks.push(task);
                }
            }
        }
    }

    // Sort by created_at descending
    tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(tasks)
}
```

## Files to Modify

1. `src/claude/runner.rs` - Add log file writing
2. `src/cli/commands/dashboard.rs` - Enhanced TUI
3. `src/state/mod.rs` - Add history saving on completion

## Constraints

- Requires `dashboard` feature flag
- Log files should be rotated/cleaned up periodically
- Keep TUI responsive (non-blocking file I/O)
- Handle large log files gracefully (limit lines displayed)

## Tests

- Test log file creation and appending
- Test dashboard navigation
- Test log scrolling
- Test real-time log refresh
