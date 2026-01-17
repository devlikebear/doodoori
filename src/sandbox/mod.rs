//! Sandbox module for Docker-based isolated execution
//!
//! Provides safe execution of Claude Code in Docker containers with:
//! - Isolated filesystem (only mounted workspace)
//! - Optional network isolation
//! - Automatic Claude credentials mounting
//! - Environment variable passing

#![allow(dead_code)]
#![allow(unused_imports)]

mod config;
mod container;
mod runner;

pub use config::{NetworkMode, SandboxConfig};
pub use container::ContainerManager;
pub use runner::{ClaudeOptions, SandboxRunner};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.image, "doodoori/sandbox:latest");
        assert_eq!(config.network, NetworkMode::Bridge);
        // By default, use Docker volume (not host mount)
        assert!(config.use_claude_volume);
        assert!(!config.mount_host_claude_config);
    }

    #[test]
    fn test_sandbox_config_builder() {
        let config = SandboxConfig::builder()
            .image("custom-image:v1")
            .network(NetworkMode::None)
            .use_claude_volume(false)
            .mount_host_claude_config(true)
            .build();

        assert_eq!(config.image, "custom-image:v1");
        assert_eq!(config.network, NetworkMode::None);
        assert!(!config.use_claude_volume);
        assert!(config.mount_host_claude_config);
    }

    #[test]
    fn test_network_mode_to_string() {
        assert_eq!(NetworkMode::Bridge.as_str(), "bridge");
        assert_eq!(NetworkMode::None.as_str(), "none");
        assert_eq!(NetworkMode::Host.as_str(), "host");
    }

    #[test]
    fn test_sandbox_config_with_env_vars() {
        let config = SandboxConfig::builder()
            .env("API_KEY", "secret123")
            .env("NODE_ENV", "production")
            .build();

        assert_eq!(config.env_vars.len(), 2);
        assert_eq!(config.env_vars.get("API_KEY"), Some(&"secret123".to_string()));
    }

    #[test]
    fn test_sandbox_config_with_mounts() {
        let config = SandboxConfig::builder()
            .mount("/host/path", "/container/path", false)
            .mount("/host/readonly", "/container/readonly", true)
            .build();

        assert_eq!(config.extra_mounts.len(), 2);
        assert!(!config.extra_mounts[0].read_only);
        assert!(config.extra_mounts[1].read_only);
    }
}
