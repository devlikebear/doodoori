//! Sandbox management commands

use anyhow::Result;
use clap::{Args, Subcommand};

/// Manage Docker sandbox environment
#[derive(Args, Debug)]
pub struct SandboxArgs {
    #[command(subcommand)]
    pub command: SandboxCommand,
}

#[derive(Subcommand, Debug)]
pub enum SandboxCommand {
    /// Login to Claude in the sandbox environment
    ///
    /// This creates a Docker volume for storing Claude credentials
    /// and runs the interactive login process inside a container.
    Login(LoginArgs),

    /// Show sandbox status
    Status,

    /// Clean up sandbox resources (volumes, containers)
    Cleanup(CleanupArgs),
}

/// Arguments for the sandbox login command
#[derive(Args, Debug)]
pub struct LoginArgs {
    /// Docker image to use
    #[arg(long, default_value = "doodoori/sandbox:latest")]
    pub image: String,

    /// Docker volume name for Claude credentials
    #[arg(long, default_value = "doodoori-claude-credentials")]
    pub volume: String,
}

/// Arguments for the sandbox cleanup command
#[derive(Args, Debug)]
pub struct CleanupArgs {
    /// Remove the Claude credentials volume
    #[arg(long)]
    pub volumes: bool,

    /// Remove all doodoori containers
    #[arg(long)]
    pub containers: bool,

    /// Remove everything (volumes and containers)
    #[arg(long)]
    pub all: bool,
}

impl SandboxArgs {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            SandboxCommand::Login(args) => args.execute().await,
            SandboxCommand::Status => execute_status().await,
            SandboxCommand::Cleanup(args) => args.execute().await,
        }
    }
}

impl LoginArgs {
    #[cfg(feature = "sandbox")]
    pub async fn execute(self) -> Result<()> {
        use crate::sandbox::ContainerManager;
        use std::process::Command;

        println!("🔐 Logging into Claude in sandbox environment...");
        println!("   Volume: {}", self.volume);
        println!("   Image: {}", self.image);

        // Initialize container manager
        let manager = ContainerManager::new()?;

        // Check Docker availability
        if !manager.is_docker_available().await {
            anyhow::bail!(
                "Docker is not available. Please ensure Docker is installed and running.\n\
                 On macOS: Start Docker Desktop\n\
                 On Linux: Run 'sudo systemctl start docker'"
            );
        }

        // Ensure image exists
        manager.ensure_image(&self.image).await?;

        // Ensure volume exists
        manager.ensure_volume(&self.volume).await?;

        println!("\n📦 Starting interactive login container...");
        println!("   Please complete the login process in the terminal.\n");

        // Run interactive docker command for login
        // This needs to be interactive so we can't use bollard - use Command instead
        let status = Command::new("docker")
            .args([
                "run",
                "-it",
                "--rm",
                "-v",
                &format!("{}:/home/doodoori/.claude:rw", self.volume),
                &self.image,
                "claude",
                "/login",
            ])
            .status()?;

        if status.success() {
            println!("\n✅ Login successful!");
            println!("   Claude credentials are stored in Docker volume: {}", self.volume);
            println!("   You can now use 'doodoori run --sandbox' to run tasks.");
        } else {
            println!("\n❌ Login failed or was cancelled.");
            println!("   You can try again with: doodoori sandbox login");
        }

        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn execute(self) -> Result<()> {
        anyhow::bail!(
            "Sandbox feature is not enabled. Rebuild with --features sandbox:\n\
             cargo build --features sandbox"
        )
    }
}

#[cfg(feature = "sandbox")]
async fn execute_status() -> Result<()> {
    use crate::sandbox::ContainerManager;

    let manager = ContainerManager::new()?;

    println!("🐳 Sandbox Status\n");

    // Check Docker
    let docker_available = manager.is_docker_available().await;
    println!("Docker: {}", if docker_available { "✅ Available" } else { "❌ Not available" });

    if !docker_available {
        println!("\n⚠️  Docker is required for sandbox mode.");
        println!("   On macOS: Start Docker Desktop");
        println!("   On Linux: Run 'sudo systemctl start docker'");
        return Ok(());
    }

    // Check for Claude credentials volume
    let volume_name = "doodoori-claude-credentials";
    let volume_check = std::process::Command::new("docker")
        .args(["volume", "inspect", volume_name])
        .output()?;

    if volume_check.status.success() {
        println!("Claude credentials volume: ✅ {} exists", volume_name);
    } else {
        println!("Claude credentials volume: ❌ Not found");
        println!("   Run 'doodoori sandbox login' to authenticate.");
    }

    // Check for running containers
    let containers = std::process::Command::new("docker")
        .args(["ps", "-a", "--filter", "name=doodoori-sandbox", "--format", "{{.Names}}: {{.Status}}"])
        .output()?;

    let container_output = String::from_utf8_lossy(&containers.stdout);
    if container_output.trim().is_empty() {
        println!("Running containers: None");
    } else {
        println!("Containers:");
        for line in container_output.lines() {
            println!("  - {}", line);
        }
    }

    Ok(())
}

#[cfg(not(feature = "sandbox"))]
async fn execute_status() -> Result<()> {
    anyhow::bail!(
        "Sandbox feature is not enabled. Rebuild with --features sandbox:\n\
         cargo build --features sandbox"
    )
}

impl CleanupArgs {
    #[cfg(feature = "sandbox")]
    pub async fn execute(self) -> Result<()> {
        use std::process::Command;

        let cleanup_volumes = self.volumes || self.all;
        let cleanup_containers = self.containers || self.all;

        if !cleanup_volumes && !cleanup_containers {
            println!("No cleanup action specified. Use --volumes, --containers, or --all");
            return Ok(());
        }

        if cleanup_containers {
            println!("🧹 Removing doodoori containers...");

            // Stop and remove all doodoori containers
            let containers = Command::new("docker")
                .args(["ps", "-aq", "--filter", "name=doodoori-sandbox"])
                .output()?;

            let container_ids = String::from_utf8_lossy(&containers.stdout);
            if !container_ids.trim().is_empty() {
                for id in container_ids.lines() {
                    let id = id.trim();
                    if !id.is_empty() {
                        let _ = Command::new("docker")
                            .args(["rm", "-f", id])
                            .output();
                        println!("   Removed container: {}", id);
                    }
                }
            } else {
                println!("   No containers to remove.");
            }
        }

        if cleanup_volumes {
            println!("🧹 Removing Claude credentials volume...");

            let volume_name = "doodoori-claude-credentials";
            let result = Command::new("docker")
                .args(["volume", "rm", volume_name])
                .output()?;

            if result.status.success() {
                println!("   Removed volume: {}", volume_name);
                println!("\n⚠️  You'll need to run 'doodoori sandbox login' again to use sandbox mode.");
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                if stderr.contains("No such volume") {
                    println!("   Volume {} does not exist.", volume_name);
                } else {
                    println!("   Failed to remove volume: {}", stderr.trim());
                }
            }
        }

        println!("\n✅ Cleanup complete.");
        Ok(())
    }

    #[cfg(not(feature = "sandbox"))]
    pub async fn execute(self) -> Result<()> {
        anyhow::bail!(
            "Sandbox feature is not enabled. Rebuild with --features sandbox:\n\
             cargo build --features sandbox"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_args_default() {
        let args = LoginArgs {
            image: "doodoori/sandbox:latest".to_string(),
            volume: "doodoori-claude-credentials".to_string(),
        };
        assert_eq!(args.image, "doodoori/sandbox:latest");
        assert_eq!(args.volume, "doodoori-claude-credentials");
    }

    #[test]
    fn test_cleanup_args() {
        let args = CleanupArgs {
            volumes: true,
            containers: false,
            all: false,
        };
        assert!(args.volumes);
        assert!(!args.containers);
        assert!(!args.all);
    }
}
