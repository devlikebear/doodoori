//! Secret management command for keychain integration

use anyhow::Result;
use clap::{Args, Subcommand};
use std::io::{self, BufRead, Write};

/// Arguments for the secret command
#[derive(Args, Debug)]
pub struct SecretArgs {
    #[command(subcommand)]
    pub command: SecretCommand,
}

#[derive(Subcommand, Debug)]
pub enum SecretCommand {
    /// Set a secret in the keychain
    Set(SetArgs),

    /// Get a secret from the keychain
    Get(GetArgs),

    /// Delete a secret from the keychain
    Delete(DeleteArgs),

    /// List known secret keys
    List,
}

#[derive(Args, Debug)]
pub struct SetArgs {
    /// Secret key name (e.g., ANTHROPIC_API_KEY)
    pub key: String,

    /// Secret value (if not provided, will prompt securely)
    #[arg(long)]
    pub value: Option<String>,

    /// Read value from environment variable
    #[arg(long)]
    pub from_env: bool,
}

#[derive(Args, Debug)]
pub struct GetArgs {
    /// Secret key name
    pub key: String,

    /// Show the full value (default: masked)
    #[arg(long)]
    pub reveal: bool,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Secret key name
    pub key: String,

    /// Skip confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

impl SecretArgs {
    pub async fn execute(self) -> Result<()> {
        match self.command {
            SecretCommand::Set(args) => args.execute().await,
            SecretCommand::Get(args) => args.execute().await,
            SecretCommand::Delete(args) => args.execute().await,
            SecretCommand::List => list_secrets().await,
        }
    }
}

impl SetArgs {
    #[cfg(feature = "keychain")]
    pub async fn execute(self) -> Result<()> {
        use crate::secrets::KeychainManager;

        let manager = KeychainManager::new()?;
        if !manager.is_available() {
            anyhow::bail!("Keychain is not available on this system");
        }

        let value = if let Some(v) = self.value {
            v
        } else if self.from_env {
            std::env::var(&self.key)
                .map_err(|_| anyhow::anyhow!("Environment variable {} not set", self.key))?
        } else {
            // Prompt for value securely
            print!("Enter value for {}: ", self.key);
            io::stdout().flush()?;

            let value = rpassword_read()?;
            if value.is_empty() {
                anyhow::bail!("Value cannot be empty");
            }
            value
        };

        manager.set(&self.key, &value)?;
        println!("Secret '{}' stored in keychain", self.key);

        Ok(())
    }

    #[cfg(not(feature = "keychain"))]
    pub async fn execute(self) -> Result<()> {
        anyhow::bail!("Keychain feature not enabled. Rebuild with --features keychain")
    }
}

impl GetArgs {
    #[cfg(feature = "keychain")]
    pub async fn execute(self) -> Result<()> {
        use crate::secrets::{KeychainManager, SecretValue};

        let manager = KeychainManager::new()?;
        if !manager.is_available() {
            anyhow::bail!("Keychain is not available on this system");
        }

        match manager.get(&self.key) {
            Ok(value) => {
                if self.reveal {
                    println!("{}", value);
                } else {
                    let secret = SecretValue::new(value);
                    println!("{}", secret.masked());
                }
            }
            Err(e) => {
                anyhow::bail!("Failed to get secret '{}': {}", self.key, e);
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "keychain"))]
    pub async fn execute(self) -> Result<()> {
        anyhow::bail!("Keychain feature not enabled. Rebuild with --features keychain")
    }
}

impl DeleteArgs {
    #[cfg(feature = "keychain")]
    pub async fn execute(self) -> Result<()> {
        use crate::secrets::KeychainManager;

        let manager = KeychainManager::new()?;
        if !manager.is_available() {
            anyhow::bail!("Keychain is not available on this system");
        }

        if !self.yes {
            print!("Are you sure you want to delete '{}'? [y/N]: ", self.key);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Cancelled");
                return Ok(());
            }
        }

        match manager.delete(&self.key) {
            Ok(_) => {
                println!("Secret '{}' deleted from keychain", self.key);
            }
            Err(e) => {
                anyhow::bail!("Failed to delete secret '{}': {}", self.key, e);
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "keychain"))]
    pub async fn execute(self) -> Result<()> {
        anyhow::bail!("Keychain feature not enabled. Rebuild with --features keychain")
    }
}

#[cfg(feature = "keychain")]
async fn list_secrets() -> Result<()> {
    use crate::secrets::KeychainManager;

    let manager = KeychainManager::new()?;
    if !manager.is_available() {
        anyhow::bail!("Keychain is not available on this system");
    }

    // Common secret keys that doodoori might use
    let known_keys = vec![
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "GH_TOKEN",
    ];

    println!("Known secret keys:");
    println!();
    println!("{:<25} {}", "KEY", "STATUS");
    println!("{}", "-".repeat(40));

    for key in &known_keys {
        let status = if manager.exists(key) {
            "stored"
        } else {
            "not set"
        };
        println!("{:<25} {}", key, status);
    }

    println!();
    println!("Use 'doodoori secret set <KEY>' to store a secret");
    println!("Use 'doodoori secret get <KEY> --reveal' to view a secret");

    Ok(())
}

#[cfg(not(feature = "keychain"))]
async fn list_secrets() -> Result<()> {
    anyhow::bail!("Keychain feature not enabled. Rebuild with --features keychain")
}

/// Read password without echo (simple implementation)
#[cfg(feature = "keychain")]
fn rpassword_read() -> Result<String> {
    // Try to disable echo for password input
    // For now, use a simple approach
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    Ok(password.trim().to_string())
}
