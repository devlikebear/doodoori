use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Model alias for easy selection
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelAlias {
    Haiku,
    #[default]
    Sonnet,
    Opus,
}

impl ModelAlias {
    /// Convert alias to full model ID (for pricing calculation)
    /// Reference: https://platform.claude.com/docs/ko/about-claude/models/overview
    #[allow(dead_code)]
    pub fn to_model_id(&self) -> &'static str {
        match self {
            ModelAlias::Haiku => "claude-haiku-4-5-20251001",
            ModelAlias::Sonnet => "claude-sonnet-4-5-20250929",
            ModelAlias::Opus => "claude-opus-4-5-20251101",
        }
    }
}

impl fmt::Display for ModelAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelAlias::Haiku => write!(f, "haiku"),
            ModelAlias::Sonnet => write!(f, "sonnet"),
            ModelAlias::Opus => write!(f, "opus"),
        }
    }
}

impl FromStr for ModelAlias {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "haiku" => Ok(ModelAlias::Haiku),
            "sonnet" => Ok(ModelAlias::Sonnet),
            "opus" => Ok(ModelAlias::Opus),
            _ => Err(format!("Unknown model: {}. Use haiku, sonnet, or opus.", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_from_str() {
        assert_eq!(ModelAlias::from_str("haiku").unwrap(), ModelAlias::Haiku);
        assert_eq!(ModelAlias::from_str("SONNET").unwrap(), ModelAlias::Sonnet);
        assert_eq!(ModelAlias::from_str("Opus").unwrap(), ModelAlias::Opus);
    }

    #[test]
    fn test_model_from_str_error() {
        assert!(ModelAlias::from_str("unknown").is_err());
    }

    #[test]
    fn test_model_display() {
        assert_eq!(ModelAlias::Haiku.to_string(), "haiku");
        assert_eq!(ModelAlias::Sonnet.to_string(), "sonnet");
        assert_eq!(ModelAlias::Opus.to_string(), "opus");
    }

    #[test]
    fn test_default_model() {
        assert_eq!(ModelAlias::default(), ModelAlias::Sonnet);
    }

    #[test]
    fn test_model_to_id() {
        // Reference: https://platform.claude.com/docs/ko/about-claude/models/overview
        assert_eq!(ModelAlias::Haiku.to_model_id(), "claude-haiku-4-5-20251001");
        assert_eq!(ModelAlias::Sonnet.to_model_id(), "claude-sonnet-4-5-20250929");
        assert_eq!(ModelAlias::Opus.to_model_id(), "claude-opus-4-5-20251101");
    }
}
