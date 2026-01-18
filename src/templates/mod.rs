use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod builtin;
pub mod storage;

/// Template for code generation and automation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Template {
    /// Unique template name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Template category
    pub category: TemplateCategory,
    /// Prompt template with variable placeholders
    pub prompt: String,
    /// Variables used in the template
    #[serde(default)]
    pub variables: Vec<TemplateVariable>,
    /// Default model to use for this template
    #[serde(default)]
    pub default_model: Option<crate::claude::ModelAlias>,
    /// Default max iterations for this template
    #[serde(default)]
    pub default_max_iterations: Option<u32>,
    /// Tags for filtering and categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Template variable definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateVariable {
    /// Variable name (used in prompt as {name})
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Default value if not provided
    #[serde(default)]
    pub default: Option<String>,
    /// Whether this variable is required
    #[serde(default)]
    pub required: bool,
}

/// Template categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TemplateCategory {
    /// Scaffold new code
    Scaffold,
    /// Refactor existing code
    Refactor,
    /// Add tests
    Test,
    /// Fix bugs
    Fix,
    /// Add documentation
    Docs,
    /// User-defined custom templates
    Custom,
}

impl Template {
    /// Render the template with provided variables
    pub fn render(&self, variables: &HashMap<String, String>) -> Result<String> {
        let mut result = self.prompt.clone();

        // Apply all variables
        for var in &self.variables {
            let value = if let Some(provided) = variables.get(&var.name) {
                provided.clone()
            } else if let Some(default) = &var.default {
                default.clone()
            } else if var.required {
                anyhow::bail!("Missing required variable: {}", var.name);
            } else {
                // Optional variable with no default, skip
                continue;
            };

            // Replace {variable_name} with the value
            let placeholder = format!("{{{}}}", var.name);
            result = result.replace(&placeholder, &value);
        }

        Ok(result)
    }

    /// Validate that all required variables are provided
    pub fn validate_variables(&self, variables: &HashMap<String, String>) -> Result<()> {
        for var in &self.variables {
            if var.required && !variables.contains_key(&var.name) && var.default.is_none() {
                anyhow::bail!(
                    "Missing required variable: {}\n\n\
                    Usage: --var {}=<value>\n\n\
                    Description: {}",
                    var.name,
                    var.name,
                    var.description
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_render_with_all_variables() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Create a {type} for {name}".to_string(),
            variables: vec![
                TemplateVariable {
                    name: "type".to_string(),
                    description: "Type of thing".to_string(),
                    default: None,
                    required: true,
                },
                TemplateVariable {
                    name: "name".to_string(),
                    description: "Name of thing".to_string(),
                    default: None,
                    required: true,
                },
            ],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let mut vars = HashMap::new();
        vars.insert("type".to_string(), "function".to_string());
        vars.insert("name".to_string(), "hello".to_string());

        let result = template.render(&vars).unwrap();
        assert_eq!(result, "Create a function for hello");
    }

    #[test]
    fn test_template_render_with_default_value() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Path: {path}".to_string(),
            variables: vec![TemplateVariable {
                name: "path".to_string(),
                description: "Path".to_string(),
                default: Some("/default".to_string()),
                required: false,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let vars = HashMap::new();
        let result = template.render(&vars).unwrap();
        assert_eq!(result, "Path: /default");
    }

    #[test]
    fn test_template_render_missing_required_variable() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Name: {name}".to_string(),
            variables: vec![TemplateVariable {
                name: "name".to_string(),
                description: "Name".to_string(),
                default: None,
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let vars = HashMap::new();
        let result = template.render(&vars);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required variable: name"));
    }

    #[test]
    fn test_validate_variables_success() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Name: {name}".to_string(),
            variables: vec![TemplateVariable {
                name: "name".to_string(),
                description: "Name".to_string(),
                default: None,
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "test".to_string());

        assert!(template.validate_variables(&vars).is_ok());
    }

    #[test]
    fn test_validate_variables_failure() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Name: {name}".to_string(),
            variables: vec![TemplateVariable {
                name: "name".to_string(),
                description: "Name".to_string(),
                default: None,
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let vars = HashMap::new();
        let result = template.validate_variables(&vars);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required variable: name"));
    }

    #[test]
    fn test_validate_variables_with_default() {
        let template = Template {
            name: "test".to_string(),
            description: "Test template".to_string(),
            category: TemplateCategory::Test,
            prompt: "Path: {path}".to_string(),
            variables: vec![TemplateVariable {
                name: "path".to_string(),
                description: "Path".to_string(),
                default: Some("/default".to_string()),
                required: true,
            }],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        let vars = HashMap::new();
        // Should pass because required variable has a default
        assert!(template.validate_variables(&vars).is_ok());
    }

    #[test]
    fn test_template_category_serialization() {
        assert_eq!(
            serde_yaml::to_string(&TemplateCategory::Scaffold).unwrap().trim(),
            "scaffold"
        );
        assert_eq!(
            serde_yaml::to_string(&TemplateCategory::Refactor).unwrap().trim(),
            "refactor"
        );
        assert_eq!(
            serde_yaml::to_string(&TemplateCategory::Test).unwrap().trim(),
            "test"
        );
    }

    #[test]
    fn test_template_deserialization_from_yaml() {
        let yaml = r#"
name: test-template
description: A test template
category: scaffold
prompt: "Create a {type} named {name}"
variables:
  - name: type
    description: Type of thing
    required: true
  - name: name
    description: Name of thing
    required: true
tags:
  - rust
  - test
"#;

        let template: Template = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.name, "test-template");
        assert_eq!(template.category, TemplateCategory::Scaffold);
        assert_eq!(template.variables.len(), 2);
        assert_eq!(template.tags, vec!["rust", "test"]);
    }
}
