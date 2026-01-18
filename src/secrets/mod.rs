//! Secrets management module for environment variables and keychain integration

mod env_loader;

#[cfg(feature = "keychain")]
mod keychain;

pub use env_loader::EnvLoader;

#[cfg(feature = "keychain")]
pub use keychain::{KeychainManager, KeychainError};

use anyhow::Result;
use std::collections::HashMap;

/// Secret value wrapper that masks itself in debug output
#[derive(Clone)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn masked(&self) -> String {
        if self.0.len() <= 8 {
            "***".to_string()
        } else {
            format!("{}***{}", &self.0[..4], &self.0[self.0.len() - 4..])
        }
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretValue({})", self.masked())
    }
}

impl std::fmt::Display for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.masked())
    }
}

/// Unified secrets manager
pub struct SecretsManager {
    env_loader: EnvLoader,
    #[cfg(feature = "keychain")]
    keychain: Option<KeychainManager>,
    /// Cache of loaded secrets
    cache: HashMap<String, SecretValue>,
}

impl SecretsManager {
    /// Create a new secrets manager
    pub fn new() -> Self {
        Self {
            env_loader: EnvLoader::new(),
            #[cfg(feature = "keychain")]
            keychain: KeychainManager::new().ok(),
            cache: HashMap::new(),
        }
    }

    /// Load secrets from .env file
    pub fn load_env_file(&mut self, path: Option<&str>) -> Result<()> {
        self.env_loader.load_file(path)?;
        Ok(())
    }

    /// Get a secret value with priority: cache -> env -> keychain
    pub fn get(&mut self, key: &str) -> Option<SecretValue> {
        // Check cache first
        if let Some(value) = self.cache.get(key) {
            return Some(value.clone());
        }

        // Try environment variable
        if let Some(value) = self.env_loader.get(key) {
            let secret = SecretValue::new(value);
            self.cache.insert(key.to_string(), secret.clone());
            return Some(secret);
        }

        // Try keychain
        #[cfg(feature = "keychain")]
        if let Some(ref keychain) = self.keychain {
            if let Ok(value) = keychain.get(key) {
                let secret = SecretValue::new(value);
                self.cache.insert(key.to_string(), secret.clone());
                return Some(secret);
            }
        }

        None
    }

    /// Set a secret in the keychain (requires keychain feature)
    #[cfg(feature = "keychain")]
    pub fn set_in_keychain(&self, key: &str, value: &str) -> Result<()> {
        if let Some(ref keychain) = self.keychain {
            keychain.set(key, value)?;
        } else {
            anyhow::bail!("Keychain not available");
        }
        Ok(())
    }

    /// Delete a secret from the keychain (requires keychain feature)
    #[cfg(feature = "keychain")]
    pub fn delete_from_keychain(&self, key: &str) -> Result<()> {
        if let Some(ref keychain) = self.keychain {
            keychain.delete(key)?;
        } else {
            anyhow::bail!("Keychain not available");
        }
        Ok(())
    }

    /// List all secrets in the keychain (requires keychain feature)
    #[cfg(feature = "keychain")]
    pub fn list_keychain_keys(&self) -> Vec<String> {
        // Note: keyring crate doesn't support listing, so we maintain a known list
        vec![
            "ANTHROPIC_API_KEY".to_string(),
            "OPENAI_API_KEY".to_string(),
            "GITHUB_TOKEN".to_string(),
        ]
    }

    /// Get all loaded environment variables as a HashMap
    pub fn get_env_vars(&self) -> HashMap<String, String> {
        self.env_loader.all()
    }

    /// Clear the cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for SecretsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_value_masking() {
        let secret = SecretValue::new("sk-ant-api-key-12345678901234567890".to_string());
        assert!(secret.masked().contains("***"));
        assert!(!secret.masked().contains("12345678901234567890"));
        assert_eq!(secret.expose(), "sk-ant-api-key-12345678901234567890");
    }

    #[test]
    fn test_secret_value_short() {
        let secret = SecretValue::new("short".to_string());
        assert_eq!(secret.masked(), "***");
    }

    #[test]
    fn test_secrets_manager_env() {
        // SAFETY: Test environment, no other threads accessing this variable
        unsafe { std::env::set_var("TEST_SECRET_KEY", "test_value") };
        let mut manager = SecretsManager::new();
        let value = manager.get("TEST_SECRET_KEY");
        assert!(value.is_some());
        assert_eq!(value.unwrap().expose(), "test_value");
        // SAFETY: Test environment cleanup
        unsafe { std::env::remove_var("TEST_SECRET_KEY") };
    }
}
