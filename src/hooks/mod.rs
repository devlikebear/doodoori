//! Hooks system for pre/post execution scripts
//!
//! Hooks allow users to run custom scripts at various points in task execution:
//! - `pre_run`: Before task execution starts
//! - `post_run`: After task execution completes (success or failure)
//! - `on_error`: When an error occurs
//! - `on_iteration`: After each loop iteration
//! - `on_complete`: When task completes successfully

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Hook types that can be triggered during execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Before task execution starts
    PreRun,
    /// After task execution completes (success or failure)
    PostRun,
    /// When an error occurs
    OnError,
    /// After each loop iteration
    OnIteration,
    /// When task completes successfully
    OnComplete,
}

impl HookType {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookType::PreRun => "pre_run",
            HookType::PostRun => "post_run",
            HookType::OnError => "on_error",
            HookType::OnIteration => "on_iteration",
            HookType::OnComplete => "on_complete",
        }
    }

    pub fn env_prefix(&self) -> &'static str {
        match self {
            HookType::PreRun => "DOODOORI_PRE_RUN",
            HookType::PostRun => "DOODOORI_POST_RUN",
            HookType::OnError => "DOODOORI_ON_ERROR",
            HookType::OnIteration => "DOODOORI_ON_ITERATION",
            HookType::OnComplete => "DOODOORI_ON_COMPLETE",
        }
    }
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Context passed to hook scripts via environment variables
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookContext {
    /// Task ID (if available)
    pub task_id: Option<String>,
    /// Task prompt/description
    pub prompt: Option<String>,
    /// Current model being used
    pub model: Option<String>,
    /// Current iteration number (for on_iteration hook)
    pub iteration: Option<u32>,
    /// Total iterations completed
    pub total_iterations: Option<u32>,
    /// Current cost in USD
    pub cost_usd: Option<f64>,
    /// Task status (running, completed, failed, etc.)
    pub status: Option<String>,
    /// Error message (for on_error hook)
    pub error: Option<String>,
    /// Working directory
    pub working_dir: Option<PathBuf>,
    /// Additional custom variables
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

impl HookContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_iteration(mut self, iteration: u32) -> Self {
        self.iteration = Some(iteration);
        self
    }

    pub fn with_total_iterations(mut self, total: u32) -> Self {
        self.total_iterations = Some(total);
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }

    /// Convert context to environment variables
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        if let Some(ref task_id) = self.task_id {
            vars.insert("DOODOORI_TASK_ID".to_string(), task_id.clone());
        }
        if let Some(ref prompt) = self.prompt {
            // Truncate long prompts for env var
            let truncated = if prompt.len() > 1000 {
                format!("{}...", &prompt[..997])
            } else {
                prompt.clone()
            };
            vars.insert("DOODOORI_PROMPT".to_string(), truncated);
        }
        if let Some(ref model) = self.model {
            vars.insert("DOODOORI_MODEL".to_string(), model.clone());
        }
        if let Some(iteration) = self.iteration {
            vars.insert("DOODOORI_ITERATION".to_string(), iteration.to_string());
        }
        if let Some(total) = self.total_iterations {
            vars.insert("DOODOORI_TOTAL_ITERATIONS".to_string(), total.to_string());
        }
        if let Some(cost) = self.cost_usd {
            vars.insert("DOODOORI_COST_USD".to_string(), format!("{:.4}", cost));
        }
        if let Some(ref status) = self.status {
            vars.insert("DOODOORI_STATUS".to_string(), status.clone());
        }
        if let Some(ref error) = self.error {
            vars.insert("DOODOORI_ERROR".to_string(), error.clone());
        }
        if let Some(ref dir) = self.working_dir {
            vars.insert("DOODOORI_WORKING_DIR".to_string(), dir.display().to_string());
        }

        // Add custom variables
        for (key, value) in &self.custom {
            vars.insert(format!("DOODOORI_{}", key.to_uppercase()), value.clone());
        }

        vars
    }
}

/// Configuration for a single hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    /// Path to the script or command to execute
    pub command: String,
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Timeout in seconds (default: 60)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Whether to continue execution if hook fails (default: false for pre_run, true for others)
    #[serde(default)]
    pub continue_on_failure: bool,
    /// Working directory for the hook (default: task working directory)
    pub working_dir: Option<PathBuf>,
    /// Additional environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    60
}

impl HookDefinition {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            timeout_secs: default_timeout(),
            continue_on_failure: false,
            working_dir: None,
            env: HashMap::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn continue_on_failure(mut self, value: bool) -> Self {
        self.continue_on_failure = value;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Hooks configuration containing all hook definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Pre-run hook
    pub pre_run: Option<HookDefinition>,
    /// Post-run hook
    pub post_run: Option<HookDefinition>,
    /// On-error hook
    pub on_error: Option<HookDefinition>,
    /// On-iteration hook
    pub on_iteration: Option<HookDefinition>,
    /// On-complete hook
    pub on_complete: Option<HookDefinition>,
    /// Global timeout for all hooks (can be overridden per hook)
    #[serde(default = "default_timeout")]
    pub default_timeout_secs: u64,
    /// Whether hooks are enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            pre_run: None,
            post_run: None,
            on_error: None,
            on_iteration: None,
            on_complete: None,
            default_timeout_secs: default_timeout(),
            enabled: true, // Enabled by default
        }
    }
}

impl HooksConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pre_run(mut self, hook: HookDefinition) -> Self {
        self.pre_run = Some(hook);
        self
    }

    pub fn with_post_run(mut self, hook: HookDefinition) -> Self {
        self.post_run = Some(hook);
        self
    }

    pub fn with_on_error(mut self, hook: HookDefinition) -> Self {
        self.on_error = Some(hook);
        self
    }

    pub fn with_on_iteration(mut self, hook: HookDefinition) -> Self {
        self.on_iteration = Some(hook);
        self
    }

    pub fn with_on_complete(mut self, hook: HookDefinition) -> Self {
        self.on_complete = Some(hook);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Get hook definition by type
    pub fn get(&self, hook_type: HookType) -> Option<&HookDefinition> {
        match hook_type {
            HookType::PreRun => self.pre_run.as_ref(),
            HookType::PostRun => self.post_run.as_ref(),
            HookType::OnError => self.on_error.as_ref(),
            HookType::OnIteration => self.on_iteration.as_ref(),
            HookType::OnComplete => self.on_complete.as_ref(),
        }
    }

    /// Check if any hooks are configured
    pub fn has_hooks(&self) -> bool {
        self.pre_run.is_some()
            || self.post_run.is_some()
            || self.on_error.is_some()
            || self.on_iteration.is_some()
            || self.on_complete.is_some()
    }

    /// Load hooks config from simple string paths (for doodoori.toml compatibility)
    pub fn from_paths(
        pre_run: Option<&str>,
        post_run: Option<&str>,
        on_error: Option<&str>,
        on_iteration: Option<&str>,
        on_complete: Option<&str>,
    ) -> Self {
        Self {
            pre_run: pre_run.map(|p| HookDefinition::new(p)),
            post_run: post_run.map(|p| HookDefinition::new(p).continue_on_failure(true)),
            on_error: on_error.map(|p| HookDefinition::new(p).continue_on_failure(true)),
            on_iteration: on_iteration.map(|p| HookDefinition::new(p).continue_on_failure(true)),
            on_complete: on_complete.map(|p| HookDefinition::new(p).continue_on_failure(true)),
            default_timeout_secs: default_timeout(),
            enabled: true,
        }
    }
}

/// Result of hook execution
#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_type: HookType,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl HookResult {
    pub fn success(hook_type: HookType, exit_code: i32, stdout: String, stderr: String, duration_ms: u64) -> Self {
        Self {
            hook_type,
            success: exit_code == 0,
            exit_code: Some(exit_code),
            stdout,
            stderr,
            duration_ms,
            error: None,
        }
    }

    pub fn failure(hook_type: HookType, error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            hook_type,
            success: false,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
            error: Some(error.into()),
        }
    }

    pub fn skipped(hook_type: HookType) -> Self {
        Self {
            hook_type,
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            error: None,
        }
    }
}

/// Hook executor that runs hook scripts
#[derive(Debug, Clone)]
pub struct HookExecutor {
    config: HooksConfig,
    base_dir: PathBuf,
}

impl HookExecutor {
    pub fn new(config: HooksConfig, base_dir: impl Into<PathBuf>) -> Self {
        Self {
            config,
            base_dir: base_dir.into(),
        }
    }

    pub fn with_default_config(base_dir: impl Into<PathBuf>) -> Self {
        Self::new(HooksConfig::default(), base_dir)
    }

    /// Check if hooks are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && self.config.has_hooks()
    }

    /// Execute a hook by type
    pub async fn execute(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult> {
        if !self.config.enabled {
            tracing::debug!("Hooks disabled, skipping {:?}", hook_type);
            return Ok(HookResult::skipped(hook_type));
        }

        let hook_def = match self.config.get(hook_type) {
            Some(def) => def,
            None => {
                tracing::debug!("No hook configured for {:?}", hook_type);
                return Ok(HookResult::skipped(hook_type));
            }
        };

        tracing::info!("Executing {} hook: {}", hook_type, hook_def.command);

        let start = std::time::Instant::now();

        // Resolve command path
        let command_path = self.resolve_path(&hook_def.command);

        // Check if command exists
        if !command_path.exists() && !self.is_system_command(&hook_def.command) {
            let error = format!("Hook script not found: {}", command_path.display());
            tracing::warn!("{}", error);
            return Ok(HookResult::failure(hook_type, error, start.elapsed().as_millis() as u64));
        }

        // Prepare working directory
        let working_dir = hook_def
            .working_dir
            .clone()
            .or_else(|| context.working_dir.clone())
            .unwrap_or_else(|| self.base_dir.clone());

        // Build command
        let mut cmd = Command::new(&command_path);
        cmd.args(&hook_def.args)
            .current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables from context
        for (key, value) in context.to_env_vars() {
            cmd.env(&key, &value);
        }

        // Set hook-specific env vars
        cmd.env("DOODOORI_HOOK_TYPE", hook_type.as_str());
        cmd.env("DOODOORI_HOOK_COMMAND", &hook_def.command);

        // Set additional env vars from hook definition
        for (key, value) in &hook_def.env {
            cmd.env(key, value);
        }

        // Execute with timeout
        let timeout_duration = Duration::from_secs(hook_def.timeout_secs);

        match timeout(timeout_duration, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if exit_code != 0 {
                    tracing::warn!(
                        "{} hook failed with exit code {}: {}",
                        hook_type,
                        exit_code,
                        stderr.trim()
                    );
                } else {
                    tracing::debug!("{} hook completed successfully", hook_type);
                }

                Ok(HookResult::success(hook_type, exit_code, stdout, stderr, duration_ms))
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let error = format!("Failed to execute hook: {}", e);
                tracing::error!("{}", error);
                Ok(HookResult::failure(hook_type, error, duration_ms))
            }
            Err(_) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let error = format!("Hook timed out after {} seconds", hook_def.timeout_secs);
                tracing::error!("{}", error);
                Ok(HookResult::failure(hook_type, error, duration_ms))
            }
        }
    }

    /// Execute hook and handle failure based on continue_on_failure setting
    pub async fn execute_with_policy(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Result<HookResult> {
        let result = self.execute(hook_type, context).await?;

        if !result.success {
            if let Some(hook_def) = self.config.get(hook_type) {
                if !hook_def.continue_on_failure {
                    return Err(anyhow::anyhow!(
                        "{} hook failed: {}",
                        hook_type,
                        result.error.as_deref().unwrap_or("unknown error")
                    ));
                }
            }
        }

        Ok(result)
    }

    /// Resolve a path relative to base directory
    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_dir.join(path)
        }
    }

    /// Check if a command is a system command (not a file path)
    fn is_system_command(&self, command: &str) -> bool {
        // Check for common shell commands or commands without path separators
        !command.contains('/') && !command.contains('\\')
    }

    /// Get the hooks configuration
    pub fn config(&self) -> &HooksConfig {
        &self.config
    }

    /// Update hooks configuration
    pub fn set_config(&mut self, config: HooksConfig) {
        self.config = config;
    }
}

/// Builder for creating HookExecutor with fluent API
pub struct HookExecutorBuilder {
    config: HooksConfig,
    base_dir: PathBuf,
}

impl HookExecutorBuilder {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            config: HooksConfig::default(),
            base_dir: base_dir.into(),
        }
    }

    pub fn pre_run(mut self, command: impl Into<String>) -> Self {
        self.config.pre_run = Some(HookDefinition::new(command));
        self
    }

    pub fn post_run(mut self, command: impl Into<String>) -> Self {
        self.config.post_run = Some(HookDefinition::new(command).continue_on_failure(true));
        self
    }

    pub fn on_error(mut self, command: impl Into<String>) -> Self {
        self.config.on_error = Some(HookDefinition::new(command).continue_on_failure(true));
        self
    }

    pub fn on_iteration(mut self, command: impl Into<String>) -> Self {
        self.config.on_iteration = Some(HookDefinition::new(command).continue_on_failure(true));
        self
    }

    pub fn on_complete(mut self, command: impl Into<String>) -> Self {
        self.config.on_complete = Some(HookDefinition::new(command).continue_on_failure(true));
        self
    }

    pub fn config(mut self, config: HooksConfig) -> Self {
        self.config = config;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.config.enabled = false;
        self
    }

    pub fn build(self) -> HookExecutor {
        HookExecutor::new(self.config, self.base_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_hook_type_display() {
        assert_eq!(HookType::PreRun.as_str(), "pre_run");
        assert_eq!(HookType::PostRun.as_str(), "post_run");
        assert_eq!(HookType::OnError.as_str(), "on_error");
        assert_eq!(HookType::OnIteration.as_str(), "on_iteration");
        assert_eq!(HookType::OnComplete.as_str(), "on_complete");
    }

    #[test]
    fn test_hook_context_to_env_vars() {
        let context = HookContext::new()
            .with_task_id("task-123")
            .with_model("sonnet")
            .with_iteration(5)
            .with_cost(1.25)
            .with_status("running")
            .with_custom("branch", "feature/test");

        let vars = context.to_env_vars();

        assert_eq!(vars.get("DOODOORI_TASK_ID"), Some(&"task-123".to_string()));
        assert_eq!(vars.get("DOODOORI_MODEL"), Some(&"sonnet".to_string()));
        assert_eq!(vars.get("DOODOORI_ITERATION"), Some(&"5".to_string()));
        assert_eq!(vars.get("DOODOORI_COST_USD"), Some(&"1.2500".to_string()));
        assert_eq!(vars.get("DOODOORI_STATUS"), Some(&"running".to_string()));
        assert_eq!(vars.get("DOODOORI_BRANCH"), Some(&"feature/test".to_string()));
    }

    #[test]
    fn test_hooks_config_from_paths() {
        let config = HooksConfig::from_paths(
            Some("scripts/pre.sh"),
            Some("scripts/post.sh"),
            None,
            None,
            None,
        );

        assert!(config.pre_run.is_some());
        assert!(config.post_run.is_some());
        assert!(config.on_error.is_none());
        assert!(config.has_hooks());
    }

    #[test]
    fn test_hooks_config_builder() {
        let config = HooksConfig::new()
            .with_pre_run(HookDefinition::new("./pre.sh").with_timeout(30))
            .with_post_run(HookDefinition::new("./post.sh"));

        assert!(config.pre_run.is_some());
        assert_eq!(config.pre_run.as_ref().unwrap().timeout_secs, 30);
        assert!(config.post_run.is_some());
    }

    #[tokio::test]
    async fn test_hook_executor_no_hooks() {
        let dir = tempdir().unwrap();
        let executor = HookExecutor::with_default_config(dir.path());

        let context = HookContext::new();
        let result = executor.execute(HookType::PreRun, &context).await.unwrap();

        // Should skip when no hook is configured
        assert!(result.success);
        assert!(result.exit_code.is_none());
    }

    #[tokio::test]
    async fn test_hook_executor_simple_command() {
        let dir = tempdir().unwrap();

        // Create a simple test script
        let script_path = dir.path().join("test.sh");
        fs::write(&script_path, "#!/bin/bash\necho \"Hello from hook\"\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let config = HooksConfig::new()
            .with_pre_run(HookDefinition::new("./test.sh"));

        let executor = HookExecutor::new(config, dir.path());
        let context = HookContext::new().with_task_id("test-task");

        let result = executor.execute(HookType::PreRun, &context).await.unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("Hello from hook"));
    }

    #[tokio::test]
    async fn test_hook_executor_with_env_vars() {
        let dir = tempdir().unwrap();

        // Create a script that echoes env vars
        let script_path = dir.path().join("env_test.sh");
        fs::write(
            &script_path,
            "#!/bin/bash\necho \"Task: $DOODOORI_TASK_ID, Model: $DOODOORI_MODEL\"\n",
        )
        .unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let config = HooksConfig::new()
            .with_pre_run(HookDefinition::new("./env_test.sh"));

        let executor = HookExecutor::new(config, dir.path());
        let context = HookContext::new()
            .with_task_id("my-task")
            .with_model("opus");

        let result = executor.execute(HookType::PreRun, &context).await.unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("Task: my-task"));
        assert!(result.stdout.contains("Model: opus"));
    }

    #[tokio::test]
    async fn test_hook_executor_failure() {
        let dir = tempdir().unwrap();

        // Create a script that fails
        let script_path = dir.path().join("fail.sh");
        fs::write(&script_path, "#!/bin/bash\nexit 1\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let config = HooksConfig::new()
            .with_pre_run(HookDefinition::new("./fail.sh"));

        let executor = HookExecutor::new(config, dir.path());
        let context = HookContext::new();

        let result = executor.execute(HookType::PreRun, &context).await.unwrap();

        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_hook_executor_continue_on_failure() {
        let dir = tempdir().unwrap();

        let script_path = dir.path().join("fail.sh");
        fs::write(&script_path, "#!/bin/bash\nexit 1\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        // pre_run with continue_on_failure = false (should error)
        let config = HooksConfig::new()
            .with_pre_run(HookDefinition::new("./fail.sh").continue_on_failure(false));

        let executor = HookExecutor::new(config, dir.path());
        let context = HookContext::new();

        let result = executor.execute_with_policy(HookType::PreRun, &context).await;
        assert!(result.is_err());

        // post_run with continue_on_failure = true (should succeed)
        let config = HooksConfig::new()
            .with_post_run(HookDefinition::new("./fail.sh").continue_on_failure(true));

        let executor = HookExecutor::new(config, dir.path());

        let result = executor.execute_with_policy(HookType::PostRun, &context).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_hook_executor_builder() {
        let dir = tempdir().unwrap();

        let executor = HookExecutorBuilder::new(dir.path())
            .pre_run("./pre.sh")
            .post_run("./post.sh")
            .on_error("./error.sh")
            .build();

        assert!(executor.config().pre_run.is_some());
        assert!(executor.config().post_run.is_some());
        assert!(executor.config().on_error.is_some());
        assert!(executor.config().on_iteration.is_none());
    }

    #[test]
    fn test_hook_definition_builder() {
        let hook = HookDefinition::new("./script.sh")
            .with_args(vec!["--verbose".to_string()])
            .with_timeout(120)
            .continue_on_failure(true)
            .with_env("MY_VAR", "my_value");

        assert_eq!(hook.command, "./script.sh");
        assert_eq!(hook.args, vec!["--verbose"]);
        assert_eq!(hook.timeout_secs, 120);
        assert!(hook.continue_on_failure);
        assert_eq!(hook.env.get("MY_VAR"), Some(&"my_value".to_string()));
    }
}
