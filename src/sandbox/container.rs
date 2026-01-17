//! Docker container management

#![allow(unused_imports)]

use anyhow::{Context, Result};
use std::collections::HashMap;

#[cfg(feature = "sandbox")]
use bollard::{
    container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, StopContainerOptions, WaitContainerOptions,
    },
    exec::{CreateExecOptions, StartExecResults},
    image::CreateImageOptions,
    Docker,
};

#[cfg(feature = "sandbox")]
use futures_util::StreamExt;

use super::config::{MountConfig, SandboxConfig};

/// Container state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerState {
    /// Container does not exist
    NotExists,
    /// Container is created but not started
    Created,
    /// Container is running
    Running,
    /// Container has exited
    Exited(i64),
    /// Unknown state
    Unknown,
}

/// Container manager for Docker operations
pub struct ContainerManager {
    #[cfg(feature = "sandbox")]
    docker: Docker,
    #[cfg(not(feature = "sandbox"))]
    _phantom: std::marker::PhantomData<()>,
}

impl ContainerManager {
    /// Create a new container manager
    #[cfg(feature = "sandbox")]
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon. Is Docker running?")?;
        Ok(Self { docker })
    }

    /// Create a new container manager (stub for non-sandbox builds)
    #[cfg(not(feature = "sandbox"))]
    pub fn new() -> Result<Self> {
        Ok(Self {
            _phantom: std::marker::PhantomData,
        })
    }

    /// Check if Docker is available
    #[cfg(feature = "sandbox")]
    pub async fn is_docker_available(&self) -> bool {
        self.docker.ping().await.is_ok()
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn is_docker_available(&self) -> bool {
        false
    }

    /// Pull an image if it doesn't exist locally
    #[cfg(feature = "sandbox")]
    pub async fn ensure_image(&self, image: &str) -> Result<()> {
        // Check if image exists locally
        if self.docker.inspect_image(image).await.is_ok() {
            tracing::debug!("Image {} already exists locally", image);
            return Ok(());
        }

        tracing::info!("Pulling image: {}", image);

        let options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        let mut stream = self.docker.create_image(Some(options), None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        tracing::debug!("Pull: {}", status);
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to pull image: {}", e));
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn ensure_image(&self, _image: &str) -> Result<()> {
        anyhow::bail!("Sandbox feature not enabled. Rebuild with --features sandbox")
    }

    /// Create a container from config
    #[cfg(feature = "sandbox")]
    pub async fn create_container(&self, name: &str, config: &SandboxConfig) -> Result<String> {
        // Ensure image exists
        self.ensure_image(&config.image).await?;

        // Build mounts
        let mounts = config.all_mounts();
        let binds: Vec<String> = mounts
            .iter()
            .map(|m| {
                let mode = if m.read_only { "ro" } else { "rw" };
                format!(
                    "{}:{}:{}",
                    m.host_path.display(),
                    m.container_path.display(),
                    mode
                )
            })
            .collect();

        // Build environment variables
        let env: Vec<String> = config
            .all_env_vars()
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // Create container config
        let container_config = Config {
            image: Some(config.image.clone()),
            env: Some(env),
            working_dir: Some(config.working_dir.to_string_lossy().to_string()),
            user: config.user.clone(),
            host_config: Some(bollard::models::HostConfig {
                binds: Some(binds),
                network_mode: Some(config.network.as_str().to_string()),
                ..Default::default()
            }),
            // Keep container running with a shell
            cmd: Some(vec!["tail".to_string(), "-f".to_string(), "/dev/null".to_string()]),
            tty: Some(true),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name,
            platform: None,
        };

        let response = self
            .docker
            .create_container(Some(options), container_config)
            .await
            .context("Failed to create container")?;

        tracing::info!("Created container: {} ({})", name, response.id);

        Ok(response.id)
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn create_container(&self, _name: &str, _config: &SandboxConfig) -> Result<String> {
        anyhow::bail!("Sandbox feature not enabled. Rebuild with --features sandbox")
    }

    /// Start a container
    #[cfg(feature = "sandbox")]
    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;

        tracing::info!("Started container: {}", container_id);
        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn start_container(&self, _container_id: &str) -> Result<()> {
        anyhow::bail!("Sandbox feature not enabled")
    }

    /// Stop a container
    #[cfg(feature = "sandbox")]
    pub async fn stop_container(&self, container_id: &str, timeout_secs: Option<i64>) -> Result<()> {
        let options = StopContainerOptions {
            t: timeout_secs.unwrap_or(10),
        };

        self.docker
            .stop_container(container_id, Some(options))
            .await
            .context("Failed to stop container")?;

        tracing::info!("Stopped container: {}", container_id);
        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn stop_container(&self, _container_id: &str, _timeout_secs: Option<i64>) -> Result<()> {
        anyhow::bail!("Sandbox feature not enabled")
    }

    /// Remove a container
    #[cfg(feature = "sandbox")]
    pub async fn remove_container(&self, container_id: &str, force: bool) -> Result<()> {
        let options = RemoveContainerOptions {
            force,
            v: true, // Remove volumes
            ..Default::default()
        };

        self.docker
            .remove_container(container_id, Some(options))
            .await
            .context("Failed to remove container")?;

        tracing::info!("Removed container: {}", container_id);
        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn remove_container(&self, _container_id: &str, _force: bool) -> Result<()> {
        anyhow::bail!("Sandbox feature not enabled")
    }

    /// Execute a command in a running container
    #[cfg(feature = "sandbox")]
    pub async fn exec_command(
        &self,
        container_id: &str,
        cmd: Vec<&str>,
        env: Option<HashMap<String, String>>,
    ) -> Result<ExecOutput> {
        let env_vec: Option<Vec<String>> = env.map(|e| {
            e.iter().map(|(k, v)| format!("{}={}", k, v)).collect()
        });

        let exec_config = CreateExecOptions {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
            env: env_vec,
            working_dir: Some("/workspace".to_string()),
            ..Default::default()
        };

        let exec = self
            .docker
            .create_exec(container_id, exec_config)
            .await
            .context("Failed to create exec")?;

        let output = self
            .docker
            .start_exec(&exec.id, None)
            .await
            .context("Failed to start exec")?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        if let StartExecResults::Attached { mut output, .. } = output {
            while let Some(msg) = output.next().await {
                match msg {
                    Ok(LogOutput::StdOut { message }) => {
                        stdout.push_str(&String::from_utf8_lossy(&message));
                    }
                    Ok(LogOutput::StdErr { message }) => {
                        stderr.push_str(&String::from_utf8_lossy(&message));
                    }
                    _ => {}
                }
            }
        }

        // Get exit code
        let inspect = self.docker.inspect_exec(&exec.id).await?;
        let exit_code = inspect.exit_code.unwrap_or(-1);

        Ok(ExecOutput {
            stdout,
            stderr,
            exit_code,
        })
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn exec_command(
        &self,
        _container_id: &str,
        _cmd: Vec<&str>,
        _env: Option<HashMap<String, String>>,
    ) -> Result<ExecOutput> {
        anyhow::bail!("Sandbox feature not enabled")
    }

    /// Get container state
    #[cfg(feature = "sandbox")]
    pub async fn get_container_state(&self, container_id: &str) -> ContainerState {
        match self.docker.inspect_container(container_id, None).await {
            Ok(info) => {
                if let Some(state) = info.state {
                    if state.running.unwrap_or(false) {
                        ContainerState::Running
                    } else if let Some(code) = state.exit_code {
                        ContainerState::Exited(code)
                    } else {
                        ContainerState::Created
                    }
                } else {
                    ContainerState::Unknown
                }
            }
            Err(_) => ContainerState::NotExists,
        }
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn get_container_state(&self, _container_id: &str) -> ContainerState {
        ContainerState::NotExists
    }

    /// Wait for container to exit
    #[cfg(feature = "sandbox")]
    pub async fn wait_container(&self, container_id: &str) -> Result<i64> {
        let options = WaitContainerOptions {
            condition: "not-running",
        };

        let mut stream = self.docker.wait_container(container_id, Some(options));

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    return Ok(response.status_code);
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Wait failed: {}", e));
                }
            }
        }

        Ok(0)
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn wait_container(&self, _container_id: &str) -> Result<i64> {
        anyhow::bail!("Sandbox feature not enabled")
    }

    /// Get container logs
    #[cfg(feature = "sandbox")]
    pub async fn get_logs(&self, container_id: &str, tail: Option<usize>) -> Result<String> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail.map(|n| n.to_string()).unwrap_or_else(|| "all".to_string()),
            ..Default::default()
        };

        let mut logs = String::new();
        let mut stream = self.docker.logs(container_id, Some(options));

        while let Some(result) = stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    logs.push_str(&String::from_utf8_lossy(&message));
                }
                Ok(LogOutput::StdErr { message }) => {
                    logs.push_str(&String::from_utf8_lossy(&message));
                }
                _ => {}
            }
        }

        Ok(logs)
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn get_logs(&self, _container_id: &str, _tail: Option<usize>) -> Result<String> {
        anyhow::bail!("Sandbox feature not enabled")
    }
}

impl Default for ContainerManager {
    fn default() -> Self {
        Self::new().expect("Failed to create ContainerManager")
    }
}

/// Output from executing a command in a container
#[derive(Debug, Clone)]
pub struct ExecOutput {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code
    pub exit_code: i64,
}

impl ExecOutput {
    /// Check if the command succeeded (exit code 0)
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get combined output (stdout + stderr)
    pub fn combined_output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}
