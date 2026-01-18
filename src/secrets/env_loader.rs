//! Environment variable loader with dotenvy integration
#![allow(dead_code)]

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Source of environment variable
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSource {
    /// From system environment
    System,
    /// From .env file
    DotEnv,
    /// From CLI argument
    Cli,
}

/// Environment variable loader
pub struct EnvLoader {
    /// Cached environment variables from .env files
    dotenv_vars: HashMap<String, String>,
    /// CLI-provided environment variables
    cli_vars: HashMap<String, String>,
}

impl EnvLoader {
    /// Create a new environment loader
    pub fn new() -> Self {
        Self {
            dotenv_vars: HashMap::new(),
            cli_vars: HashMap::new(),
        }
    }

    /// Load environment variables from a file (defaults to .env)
    pub fn load_file(&mut self, path: Option<&str>) -> Result<()> {
        let path = path.unwrap_or(".env");

        if !Path::new(path).exists() {
            tracing::debug!("Env file {} does not exist, skipping", path);
            return Ok(());
        }

        match dotenvy::from_filename(path) {
            Ok(_) => {
                tracing::info!("Loaded environment from {}", path);
                // Also cache the variables for our use
                self.cache_from_file(path)?;
            }
            Err(e) => {
                tracing::warn!("Failed to load {}: {}", path, e);
            }
        }
        Ok(())
    }

    /// Load and apply environment variables from .env in current directory
    pub fn load_dotenv(&mut self) -> Result<()> {
        self.load_file(None)
    }

    /// Cache variables from a .env file without setting them
    fn cache_from_file(&mut self, path: &str) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path))?;

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse KEY=VALUE
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim();

                // Remove surrounding quotes if present
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value[1..value.len() - 1].to_string()
                } else {
                    value.to_string()
                };

                self.dotenv_vars.insert(key, value);
            }
        }
        Ok(())
    }

    /// Add a CLI-provided environment variable
    pub fn add_cli_var(&mut self, key: String, value: String) {
        self.cli_vars.insert(key, value);
    }

    /// Get an environment variable with priority: CLI > .env > System
    pub fn get(&self, key: &str) -> Option<String> {
        // CLI has highest priority
        if let Some(value) = self.cli_vars.get(key) {
            return Some(value.clone());
        }

        // Then .env file
        if let Some(value) = self.dotenv_vars.get(key) {
            return Some(value.clone());
        }

        // Finally system environment
        std::env::var(key).ok()
    }

    /// Get the source of an environment variable
    pub fn get_source(&self, key: &str) -> Option<EnvSource> {
        if self.cli_vars.contains_key(key) {
            return Some(EnvSource::Cli);
        }
        if self.dotenv_vars.contains_key(key) {
            return Some(EnvSource::DotEnv);
        }
        if std::env::var(key).is_ok() {
            return Some(EnvSource::System);
        }
        None
    }

    /// Get all loaded environment variables (merged)
    pub fn all(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        // Add system vars that we care about
        for key in &["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "PATH", "HOME", "USER"] {
            if let Ok(value) = std::env::var(key) {
                result.insert(key.to_string(), value);
            }
        }

        // Overlay .env vars
        for (key, value) in &self.dotenv_vars {
            result.insert(key.clone(), value.clone());
        }

        // Overlay CLI vars (highest priority)
        for (key, value) in &self.cli_vars {
            result.insert(key.clone(), value.clone());
        }

        result
    }

    /// Check if a specific key is set anywhere
    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }
}

impl Default for EnvLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_env_loader_system() {
        // SAFETY: Test environment, no other threads accessing this variable
        unsafe { std::env::set_var("TEST_ENV_LOADER_KEY", "system_value") };
        let loader = EnvLoader::new();
        assert_eq!(loader.get("TEST_ENV_LOADER_KEY"), Some("system_value".to_string()));
        assert_eq!(loader.get_source("TEST_ENV_LOADER_KEY"), Some(EnvSource::System));
        // SAFETY: Test environment cleanup
        unsafe { std::env::remove_var("TEST_ENV_LOADER_KEY") };
    }

    #[test]
    fn test_env_loader_cli_priority() {
        // SAFETY: Test environment, no other threads accessing this variable
        unsafe { std::env::set_var("TEST_PRIORITY_KEY", "system_value") };
        let mut loader = EnvLoader::new();
        loader.add_cli_var("TEST_PRIORITY_KEY".to_string(), "cli_value".to_string());
        assert_eq!(loader.get("TEST_PRIORITY_KEY"), Some("cli_value".to_string()));
        assert_eq!(loader.get_source("TEST_PRIORITY_KEY"), Some(EnvSource::Cli));
        // SAFETY: Test environment cleanup
        unsafe { std::env::remove_var("TEST_PRIORITY_KEY") };
    }

    #[test]
    fn test_env_loader_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "TEST_FILE_KEY=file_value").unwrap();
        writeln!(file, "QUOTED_KEY=\"quoted value\"").unwrap();
        writeln!(file, "# This is a comment").unwrap();
        writeln!(file, "").unwrap();

        let mut loader = EnvLoader::new();
        loader.cache_from_file(file.path().to_str().unwrap()).unwrap();

        assert_eq!(loader.get("TEST_FILE_KEY"), Some("file_value".to_string()));
        assert_eq!(loader.get("QUOTED_KEY"), Some("quoted value".to_string()));
        assert_eq!(loader.get_source("TEST_FILE_KEY"), Some(EnvSource::DotEnv));
    }

    #[test]
    fn test_env_loader_missing_file() {
        let mut loader = EnvLoader::new();
        // Should not error on missing file
        assert!(loader.load_file(Some("/nonexistent/.env")).is_ok());
    }
}
