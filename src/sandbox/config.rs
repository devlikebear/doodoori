//! Sandbox configuration

use std::collections::HashMap;
use std::path::PathBuf;

/// Network mode for sandbox container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkMode {
    /// Bridge network - container can access internet
    #[default]
    Bridge,
    /// No network - completely isolated
    None,
    /// Host network - share host's network (less isolated)
    Host,
}

impl NetworkMode {
    /// Convert to Docker network mode string
    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkMode::Bridge => "bridge",
            NetworkMode::None => "none",
            NetworkMode::Host => "host",
        }
    }
}

/// Mount configuration for sandbox
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// Host path to mount
    pub host_path: PathBuf,
    /// Container path to mount to
    pub container_path: PathBuf,
    /// Whether the mount is read-only
    pub read_only: bool,
}

impl MountConfig {
    /// Create a new mount configuration
    pub fn new(host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>, read_only: bool) -> Self {
        Self {
            host_path: host_path.into(),
            container_path: container_path.into(),
            read_only,
        }
    }

    /// Create a read-write mount
    pub fn rw(host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>) -> Self {
        Self::new(host_path, container_path, false)
    }

    /// Create a read-only mount
    pub fn ro(host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>) -> Self {
        Self::new(host_path, container_path, true)
    }
}

/// Sandbox configuration
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Docker image to use
    pub image: String,
    /// Network mode
    pub network: NetworkMode,
    /// Whether to mount ~/.claude for authentication
    pub mount_claude_config: bool,
    /// Working directory to mount
    pub workspace_dir: Option<PathBuf>,
    /// Container path for workspace
    pub workspace_container_path: PathBuf,
    /// Additional mounts
    pub extra_mounts: Vec<MountConfig>,
    /// Environment variables to pass
    pub env_vars: HashMap<String, String>,
    /// Container name prefix
    pub container_name_prefix: String,
    /// Timeout in seconds (0 = no timeout)
    pub timeout_secs: u64,
    /// User to run as inside container
    pub user: Option<String>,
    /// Working directory inside container
    pub working_dir: PathBuf,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            image: "doodoori/sandbox:latest".to_string(),
            network: NetworkMode::Bridge,
            mount_claude_config: true,
            workspace_dir: None,
            workspace_container_path: PathBuf::from("/workspace"),
            extra_mounts: Vec::new(),
            env_vars: HashMap::new(),
            container_name_prefix: "doodoori-sandbox".to_string(),
            timeout_secs: 0,
            user: Some("doodoori".to_string()),
            working_dir: PathBuf::from("/workspace"),
        }
    }
}

impl SandboxConfig {
    /// Create a new builder for SandboxConfig
    pub fn builder() -> SandboxConfigBuilder {
        SandboxConfigBuilder::default()
    }

    /// Get all mounts including automatic ones
    pub fn all_mounts(&self) -> Vec<MountConfig> {
        let mut mounts = Vec::new();

        // Workspace mount (read-write)
        if let Some(ref workspace) = self.workspace_dir {
            mounts.push(MountConfig::rw(workspace.clone(), &self.workspace_container_path));
        }

        // Claude config mount (read-only)
        if self.mount_claude_config {
            if let Some(home) = dirs::home_dir() {
                let claude_path = home.join(".claude");
                if claude_path.exists() {
                    mounts.push(MountConfig::ro(claude_path, "/home/doodoori/.claude"));
                }
            }
        }

        // Extra mounts
        mounts.extend(self.extra_mounts.clone());

        mounts
    }

    /// Get environment variables including automatic ones
    pub fn all_env_vars(&self) -> HashMap<String, String> {
        let mut env = self.env_vars.clone();

        // Add ANTHROPIC_API_KEY if set in environment
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            env.entry("ANTHROPIC_API_KEY".to_string())
                .or_insert(api_key);
        }

        // Set HOME for the container user
        env.entry("HOME".to_string())
            .or_insert("/home/doodoori".to_string());

        env
    }
}

/// Builder for SandboxConfig
#[derive(Debug, Default)]
pub struct SandboxConfigBuilder {
    image: Option<String>,
    network: Option<NetworkMode>,
    mount_claude_config: Option<bool>,
    workspace_dir: Option<PathBuf>,
    workspace_container_path: Option<PathBuf>,
    extra_mounts: Vec<MountConfig>,
    env_vars: HashMap<String, String>,
    container_name_prefix: Option<String>,
    timeout_secs: Option<u64>,
    user: Option<String>,
    working_dir: Option<PathBuf>,
}

impl SandboxConfigBuilder {
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

    /// Set whether to mount Claude config
    pub fn mount_claude_config(mut self, mount: bool) -> Self {
        self.mount_claude_config = Some(mount);
        self
    }

    /// Set the workspace directory
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace_dir = Some(path.into());
        self
    }

    /// Set the container workspace path
    pub fn workspace_container_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.workspace_container_path = Some(path.into());
        self
    }

    /// Add an extra mount
    pub fn mount(mut self, host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>, read_only: bool) -> Self {
        self.extra_mounts.push(MountConfig::new(host_path, container_path, read_only));
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Set the container name prefix
    pub fn container_name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.container_name_prefix = Some(prefix.into());
        self
    }

    /// Set the timeout in seconds
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set the user to run as
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the working directory inside container
    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Build the SandboxConfig
    pub fn build(self) -> SandboxConfig {
        let default = SandboxConfig::default();
        SandboxConfig {
            image: self.image.unwrap_or(default.image),
            network: self.network.unwrap_or(default.network),
            mount_claude_config: self.mount_claude_config.unwrap_or(default.mount_claude_config),
            workspace_dir: self.workspace_dir,
            workspace_container_path: self.workspace_container_path.unwrap_or(default.workspace_container_path),
            extra_mounts: self.extra_mounts,
            env_vars: self.env_vars,
            container_name_prefix: self.container_name_prefix.unwrap_or(default.container_name_prefix),
            timeout_secs: self.timeout_secs.unwrap_or(default.timeout_secs),
            user: self.user.or(default.user),
            working_dir: self.working_dir.unwrap_or(default.working_dir),
        }
    }
}

/// Helper module for home directory detection
mod dirs {
    use std::path::PathBuf;

    /// Get the user's home directory
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
    }
}
