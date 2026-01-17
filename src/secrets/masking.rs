//! Secret masking for logs and output

use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

/// Patterns for common secrets
static SECRET_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // Anthropic API keys
        (
            Regex::new(r"sk-ant-[a-zA-Z0-9_-]{20,}").unwrap(),
            "sk-ant-***"
        ),
        // OpenAI API keys (includes sk-proj-*, sk-org-*, etc.)
        (
            Regex::new(r"sk-[a-zA-Z0-9_-]{20,}").unwrap(),
            "sk-***"
        ),
        // GitHub tokens
        (
            Regex::new(r"ghp_[a-zA-Z0-9]{36,}").unwrap(),
            "ghp_***"
        ),
        (
            Regex::new(r"gho_[a-zA-Z0-9]{36,}").unwrap(),
            "gho_***"
        ),
        (
            Regex::new(r"ghu_[a-zA-Z0-9]{36,}").unwrap(),
            "ghu_***"
        ),
        // AWS keys
        (
            Regex::new(r"AKIA[A-Z0-9]{16}").unwrap(),
            "AKIA***"
        ),
        // Generic API key patterns in key=value format
        (
            Regex::new(r#"(?i)(api[_-]?key|secret[_-]?key|access[_-]?token|auth[_-]?token)\s*[=:]\s*['"]?([a-zA-Z0-9_-]{20,})['"]?"#).unwrap(),
            "$1=***"
        ),
        // Bearer tokens
        (
            Regex::new(r"(?i)bearer\s+[a-zA-Z0-9_.=-]{20,}").unwrap(),
            "Bearer ***"
        ),
    ]
});

/// Environment variable names that should always be masked
static SENSITIVE_ENV_VARS: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "DOCKER_PASSWORD",
    "NPM_TOKEN",
    "PYPI_TOKEN",
];

/// Secret masker for filtering sensitive data from output
pub struct SecretMasker {
    /// Additional custom patterns to mask
    custom_patterns: Vec<(Regex, String)>,
    /// Additional environment variable names to mask
    custom_env_vars: Vec<String>,
}

impl SecretMasker {
    /// Create a new secret masker with default patterns
    pub fn new() -> Self {
        Self {
            custom_patterns: Vec::new(),
            custom_env_vars: Vec::new(),
        }
    }

    /// Add a custom pattern to mask
    pub fn add_pattern(&mut self, pattern: &str, replacement: &str) -> Result<(), regex::Error> {
        let regex = Regex::new(pattern)?;
        self.custom_patterns.push((regex, replacement.to_string()));
        Ok(())
    }

    /// Add a custom environment variable name to mask
    pub fn add_env_var(&mut self, name: &str) {
        self.custom_env_vars.push(name.to_string());
    }

    /// Mask all sensitive data in a string
    pub fn mask<'a>(&self, text: &'a str) -> Cow<'a, str> {
        let mut result = Cow::Borrowed(text);

        // Apply default patterns
        for (pattern, replacement) in SECRET_PATTERNS.iter() {
            if pattern.is_match(&result) {
                result = Cow::Owned(pattern.replace_all(&result, *replacement).to_string());
            }
        }

        // Apply custom patterns
        for (pattern, replacement) in &self.custom_patterns {
            if pattern.is_match(&result) {
                result = Cow::Owned(pattern.replace_all(&result, replacement.as_str()).to_string());
            }
        }

        // Mask environment variable values
        for env_var in SENSITIVE_ENV_VARS.iter().chain(self.custom_env_vars.iter().map(|s| s.as_str()).collect::<Vec<_>>().iter()) {
            if let Ok(value) = std::env::var(env_var) {
                if !value.is_empty() && result.contains(&value) {
                    result = Cow::Owned(result.replace(&value, &format!("${{{}}}", env_var)));
                }
            }
        }

        result
    }

    /// Check if a string contains any sensitive data
    pub fn contains_secrets(&self, text: &str) -> bool {
        // Check default patterns
        for (pattern, _) in SECRET_PATTERNS.iter() {
            if pattern.is_match(text) {
                return true;
            }
        }

        // Check custom patterns
        for (pattern, _) in &self.custom_patterns {
            if pattern.is_match(text) {
                return true;
            }
        }

        // Check for env var values
        for env_var in SENSITIVE_ENV_VARS.iter().chain(self.custom_env_vars.iter().map(|s| s.as_str()).collect::<Vec<_>>().iter()) {
            if let Ok(value) = std::env::var(env_var) {
                if !value.is_empty() && text.contains(&value) {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for SecretMasker {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracing layer for masking secrets in log output
pub struct MaskingLayer {
    masker: SecretMasker,
}

impl MaskingLayer {
    /// Create a new masking layer
    pub fn new() -> Self {
        Self {
            masker: SecretMasker::new(),
        }
    }

    /// Mask a log message
    pub fn mask_message(&self, message: &str) -> String {
        self.masker.mask(message).into_owned()
    }
}

impl Default for MaskingLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_anthropic_key() {
        let masker = SecretMasker::new();
        let text = "Using key: sk-ant-api03-abc123def456ghi789jkl012mno345pqr678stu901vwx234yz";
        let masked = masker.mask(text);
        assert!(masked.contains("sk-ant-***"));
        assert!(!masked.contains("abc123"));
    }

    #[test]
    fn test_mask_openai_key() {
        let masker = SecretMasker::new();
        let text = "OPENAI_API_KEY=sk-proj-abc123def456ghi789jkl012mno345pqr";
        let masked = masker.mask(text);
        assert!(masked.contains("sk-***"));
    }

    #[test]
    fn test_mask_github_token() {
        let masker = SecretMasker::new();
        let text = "token: ghp_abc123def456ghi789jkl012mno345pqrstu678";
        let masked = masker.mask(text);
        assert!(masked.contains("ghp_***"));
    }

    #[test]
    fn test_mask_bearer_token() {
        let masker = SecretMasker::new();
        let text = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0";
        let masked = masker.mask(text);
        assert!(masked.contains("Bearer ***"));
    }

    #[test]
    fn test_mask_generic_api_key() {
        let masker = SecretMasker::new();
        let text = "api_key=abcdef123456789012345678901234567890";
        let masked = masker.mask(text);
        assert!(masked.contains("api_key=***"));
    }

    #[test]
    fn test_mask_env_var_value() {
        // SAFETY: Test environment, no other threads accessing this variable
        unsafe { std::env::set_var("TEST_MASK_SECRET", "supersecretvalue12345") };
        let mut masker = SecretMasker::new();
        masker.add_env_var("TEST_MASK_SECRET");
        let text = "The secret is supersecretvalue12345 embedded here";
        let masked = masker.mask(text);
        assert!(!masked.contains("supersecretvalue12345"));
        assert!(masked.contains("${TEST_MASK_SECRET}"));
        // SAFETY: Test environment cleanup
        unsafe { std::env::remove_var("TEST_MASK_SECRET") };
    }

    #[test]
    fn test_contains_secrets() {
        let masker = SecretMasker::new();
        assert!(masker.contains_secrets("sk-ant-api-key-123456789012345678901234567890"));
        assert!(!masker.contains_secrets("This is normal text"));
    }

    #[test]
    fn test_custom_pattern() {
        let mut masker = SecretMasker::new();
        masker.add_pattern(r"custom-secret-[a-z0-9]+", "custom-***").unwrap();
        let text = "Using custom-secret-abc123 in config";
        let masked = masker.mask(text);
        assert!(masked.contains("custom-***"));
    }
}
