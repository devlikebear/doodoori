use std::fmt;
use std::str::FromStr;

/// Model alias for easy selection
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModelAlias {
    Haiku,
    #[default]
    Sonnet,
    Opus,
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
}
