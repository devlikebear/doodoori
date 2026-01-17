//! Sandbox runner for executing Claude Code in Docker

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use super::config::{NetworkMode, SandboxConfig};
use super::container::{ContainerManager, ContainerState, ExecOutput};

/// Result of a sandbox execution
#[derive(Debug)]
pub struct SandboxResult {
    /// Container ID
    pub container_id: String,
    /// Execution output
    pub output: ExecOutput,
    /// Whether the execution was successful
    pub success: bool,
}

/// Runner for executing Claude Code in a Docker sandbox
pub struct SandboxRunner {
    config: SandboxConfig,
    container_manager: ContainerManager,
    container_id: Option<String>,
}

impl SandboxRunner {
    /// Create a new sandbox runner with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(SandboxConfig::default())
    }

    /// Create a new sandbox runner with custom configuration
    pub fn with_config(config: SandboxConfig) -> Result<Self> {
        let container_manager = ContainerManager::new()?;
        Ok(Self {
            config,
            container_manager,
            container_id: None,
        })
    }

    /// Create a builder for SandboxRunner
    pub fn builder() -> SandboxRunnerBuilder {
        SandboxRunnerBuilder::default()
    }

    /// Check if Docker is available
    pub async fn check_docker(&self) -> Result<()> {
        if !self.container_manager.is_docker_available().await {
            anyhow::bail!(
                "Docker is not available. Please ensure Docker is installed and running.\n\
                 On macOS: Start Docker Desktop\n\
                 On Linux: Run 'sudo systemctl start docker'"
            );
        }
        Ok(())
    }

    /// Initialize the sandbox container
    pub async fn init(&mut self) -> Result<()> {
        // Check Docker availability
        self.check_docker().await?;

        // Generate unique container name
        let container_name = format!(
            "{}-{}",
            self.config.container_name_prefix,
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        // Create and start container
        let container_id = self
            .container_manager
            .create_container(&container_name, &self.config)
            .await?;

        self.container_manager.start_container(&container_id).await?;

        self.container_id = Some(container_id);

        tracing::info!("Sandbox initialized: {}", container_name);

        Ok(())
    }

    /// Execute Claude Code with the given prompt
    pub async fn run_claude(&self, prompt: &str, options: &ClaudeOptions) -> Result<SandboxResult> {
        let container_id = self
            .container_id
            .as_ref()
            .context("Sandbox not initialized. Call init() first.")?;

        // Build Claude command
        let mut cmd = vec!["claude", "-p", prompt, "--output-format", "stream-json"];

        // Add model if specified
        let model_arg;
        if let Some(ref model) = options.model {
            model_arg = format!("--model={}", model);
            cmd.push(&model_arg);
        }

        // Add allowed tools
        let tools_arg;
        if let Some(ref tools) = options.allowed_tools {
            tools_arg = format!("--allowedTools={}", tools);
            cmd.push(&tools_arg);
        }

        // Add YOLO mode
        if options.yolo {
            cmd.push("--dangerously-skip-permissions");
        }

        // Add session continue if provided
        let session_arg;
        if let Some(ref session_id) = options.session_id {
            session_arg = format!("--continue={}", session_id);
            cmd.push(&session_arg);
        }

        tracing::debug!("Executing in sandbox: {:?}", cmd);

        // Execute command
        let output = self
            .container_manager
            .exec_command(container_id, cmd, options.env.clone())
            .await?;

        let success = output.success();

        Ok(SandboxResult {
            container_id: container_id.clone(),
            output,
            success,
        })
    }

    /// Execute an arbitrary command in the sandbox
    pub async fn exec(&self, cmd: Vec<&str>) -> Result<ExecOutput> {
        let container_id = self
            .container_id
            .as_ref()
            .context("Sandbox not initialized. Call init() first.")?;

        self.container_manager
            .exec_command(container_id, cmd, None)
            .await
    }

    /// Get the current container state
    pub async fn state(&self) -> ContainerState {
        match &self.container_id {
            Some(id) => self.container_manager.get_container_state(id).await,
            None => ContainerState::NotExists,
        }
    }

    /// Stop the sandbox container
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(ref container_id) = self.container_id {
            self.container_manager
                .stop_container(container_id, Some(10))
                .await?;
        }
        Ok(())
    }

    /// Cleanup the sandbox (stop and remove container)
    pub async fn cleanup(&mut self) -> Result<()> {
        if let Some(ref container_id) = self.container_id {
            // Try to stop first
            let _ = self.container_manager.stop_container(container_id, Some(5)).await;

            // Force remove
            self.container_manager
                .remove_container(container_id, true)
                .await?;

            self.container_id = None;
        }
        Ok(())
    }

    /// Get container ID if initialized
    pub fn container_id(&self) -> Option<&str> {
        self.container_id.as_deref()
    }

    /// Get logs from the container
    pub async fn logs(&self, tail: Option<usize>) -> Result<String> {
        let container_id = self
            .container_id
            .as_ref()
            .context("Sandbox not initialized")?;

        self.container_manager.get_logs(container_id, tail).await
    }
}

impl Drop for SandboxRunner {
    fn drop(&mut self) {
        // Note: Async cleanup cannot be done in Drop
        // Container cleanup should be done explicitly via cleanup()
        if self.container_id.is_some() {
            tracing::warn!(
                "SandboxRunner dropped without cleanup. Container may still be running. \
                 Call cleanup() explicitly before dropping."
            );
        }
    }
}

/// Options for running Claude Code
#[derive(Debug, Clone, Default)]
pub struct ClaudeOptions {
    /// Model to use (haiku, sonnet, opus)
    pub model: Option<String>,
    /// Allowed tools
    pub allowed_tools: Option<String>,
    /// YOLO mode (skip permissions)
    pub yolo: bool,
    /// Session ID for continuation
    pub session_id: Option<String>,
    /// Additional environment variables
    pub env: Option<HashMap<String, String>>,
}

impl ClaudeOptions {
    /// Create new ClaudeOptions with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set allowed tools
    pub fn allowed_tools(mut self, tools: impl Into<String>) -> Self {
        self.allowed_tools = Some(tools.into());
        self
    }

    /// Enable YOLO mode
    pub fn yolo(mut self) -> Self {
        self.yolo = true;
        self
    }

    /// Set session ID for continuation
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Add environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }
}

/// Builder for SandboxRunner
#[derive(Debug, Default)]
pub struct SandboxRunnerBuilder {
    image: Option<String>,
    network: Option<NetworkMode>,
    workspace: Option<PathBuf>,
    mount_claude_config: Option<bool>,
    env_vars: HashMap<String, String>,
}

impl SandboxRunnerBuilder {
    /// Set the Docker image
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = Some(image.into());
        self
    }

    /// Set the network mode
    pub fn network(mut self, network: NetworkMode) -> Self {
        self.network = Some(network);
        self
    }

    /// Set the workspace directory
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace = Some(path.into());
        self
    }

    /// Set whether to mount Claude config
    pub fn mount_claude_config(mut self, mount: bool) -> Self {
        self.mount_claude_config = Some(mount);
        self
    }

    /// Add environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Build the SandboxRunner
    pub fn build(self) -> Result<SandboxRunner> {
        let mut config_builder = SandboxConfig::builder();

        if let Some(image) = self.image {
            config_builder = config_builder.image(image);
        }

        if let Some(network) = self.network {
            config_builder = config_builder.network(network);
        }

        if let Some(workspace) = self.workspace {
            config_builder = config_builder.workspace(workspace);
        }

        if let Some(mount) = self.mount_claude_config {
            config_builder = config_builder.mount_claude_config(mount);
        }

        for (k, v) in self.env_vars {
            config_builder = config_builder.env(k, v);
        }

        let config = config_builder.build();
        SandboxRunner::with_config(config)
    }
}
