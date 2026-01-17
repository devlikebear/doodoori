//! Keychain integration for secure secret storage

use keyring::Entry;
use thiserror::Error;

/// Service name for keychain entries
const SERVICE_NAME: &str = "doodoori";

/// Errors that can occur during keychain operations
#[derive(Error, Debug)]
pub enum KeychainError {
    #[error("Failed to access keychain: {0}")]
    AccessError(String),
    #[error("Secret not found: {0}")]
    NotFound(String),
    #[error("Failed to store secret: {0}")]
    StoreError(String),
    #[error("Failed to delete secret: {0}")]
    DeleteError(String),
    #[error("Keychain not available on this platform")]
    NotAvailable,
}

/// Keychain manager for secure secret storage
pub struct KeychainManager {
    /// Whether keychain is available
    available: bool,
}

impl KeychainManager {
    /// Create a new keychain manager
    pub fn new() -> Result<Self, KeychainError> {
        // Test if keychain is available by trying to create an entry
        let test_entry = Entry::new(SERVICE_NAME, "test");
        let available = test_entry.is_ok();

        if !available {
            tracing::warn!("Keychain is not available on this system");
        }

        Ok(Self { available })
    }

    /// Check if keychain is available
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Get a secret from the keychain
    pub fn get(&self, key: &str) -> Result<String, KeychainError> {
        if !self.available {
            return Err(KeychainError::NotAvailable);
        }

        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| KeychainError::AccessError(e.to_string()))?;

        entry
            .get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeychainError::NotFound(key.to_string()),
                _ => KeychainError::AccessError(e.to_string()),
            })
    }

    /// Store a secret in the keychain
    pub fn set(&self, key: &str, value: &str) -> Result<(), KeychainError> {
        if !self.available {
            return Err(KeychainError::NotAvailable);
        }

        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| KeychainError::AccessError(e.to_string()))?;

        entry
            .set_password(value)
            .map_err(|e| KeychainError::StoreError(e.to_string()))
    }

    /// Delete a secret from the keychain
    pub fn delete(&self, key: &str) -> Result<(), KeychainError> {
        if !self.available {
            return Err(KeychainError::NotAvailable);
        }

        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| KeychainError::AccessError(e.to_string()))?;

        entry
            .delete_credential()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeychainError::NotFound(key.to_string()),
                _ => KeychainError::DeleteError(e.to_string()),
            })
    }

    /// Check if a secret exists in the keychain
    pub fn exists(&self, key: &str) -> bool {
        if !self.available {
            return false;
        }

        self.get(key).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keychain_manager_creation() {
        // This test just checks that creation doesn't panic
        let result = KeychainManager::new();
        // May or may not be available depending on the system
        assert!(result.is_ok());
    }

    #[test]
    #[ignore] // Ignore by default as it requires actual keychain access
    fn test_keychain_operations() {
        let manager = KeychainManager::new().unwrap();
        if !manager.is_available() {
            return;
        }

        let test_key = "doodoori_test_key";
        let test_value = "test_secret_value";

        // Set
        manager.set(test_key, test_value).unwrap();

        // Get
        let retrieved = manager.get(test_key).unwrap();
        assert_eq!(retrieved, test_value);

        // Exists
        assert!(manager.exists(test_key));

        // Delete
        manager.delete(test_key).unwrap();

        // Should not exist anymore
        assert!(!manager.exists(test_key));
    }
}
