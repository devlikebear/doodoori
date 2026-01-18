use anyhow::Result;
use clap::{Args, Subcommand};

use crate::claude::ModelAlias;

/// Template management commands
#[derive(Subcommand, Debug)]
pub enum TemplateCommand {
    /// List available templates
    List(TemplateListArgs),
    /// Show template details
    Show(TemplateShowArgs),
    /// Use a template to generate a prompt
    Use(TemplateUseArgs),
    /// Create a new user template
    Create(TemplateCreateArgs),
    /// Delete a user template
    Delete(TemplateDeleteArgs),
}

/// Arguments for listing templates
#[derive(Args, Debug)]
pub struct TemplateListArgs {
    /// Filter by category
    #[arg(long)]
    pub category: Option<String>,

    /// Filter by tag
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Show only built-in templates
    #[arg(long)]
    pub builtin_only: bool,

    /// Show only user templates
    #[arg(long)]
    pub user_only: bool,
}

/// Arguments for showing template details
#[derive(Args, Debug)]
pub struct TemplateShowArgs {
    /// Template name to show
    pub name: String,
}

/// Arguments for using a template
#[derive(Args, Debug)]
pub struct TemplateUseArgs {
    /// Template name to use
    pub name: String,

    /// Template variables in key=value format
    #[arg(long)]
    pub var: Vec<String>,

    /// Show rendered prompt without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Model to use for execution
    #[arg(short, long)]
    pub model: Option<ModelAlias>,

    /// Maximum budget in USD
    #[arg(long)]
    pub budget: Option<f64>,

    /// Skip all permission prompts (DANGEROUS)
    #[arg(long)]
    pub yolo: bool,
}

impl TemplateUseArgs {
    /// Parse variables from key=value format
    pub fn parse_variables(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut vars = std::collections::HashMap::new();

        for var_str in &self.var {
            if let Some((key, value)) = var_str.split_once('=') {
                vars.insert(key.to_string(), value.to_string());
            } else {
                anyhow::bail!("Invalid variable format: '{}'. Use key=value", var_str);
            }
        }

        Ok(vars)
    }
}

/// Arguments for creating a template
#[derive(Args, Debug)]
pub struct TemplateCreateArgs {
    /// Name for the new template
    pub name: String,

    /// Create from file (YAML)
    #[arg(short, long)]
    pub from_file: Option<String>,

    /// Template category
    #[arg(long)]
    pub category: Option<String>,

    /// Template description
    #[arg(short, long)]
    pub description: Option<String>,
}

/// Arguments for deleting a template
#[derive(Args, Debug)]
pub struct TemplateDeleteArgs {
    /// Template name to delete
    pub name: String,

    /// Force deletion without confirmation
    #[arg(short, long)]
    pub force: bool,
}

impl TemplateListArgs {
    pub async fn execute(&self) -> Result<()> {
        println!("Template list command - not yet implemented");
        println!("  category: {:?}", self.category);
        println!("  tag: {:?}", self.tag);
        println!("  builtin_only: {}", self.builtin_only);
        println!("  user_only: {}", self.user_only);
        Ok(())
    }
}

impl TemplateShowArgs {
    pub async fn execute(&self) -> Result<()> {
        println!("Template show command - not yet implemented");
        println!("  name: {}", self.name);
        Ok(())
    }
}

impl TemplateUseArgs {
    pub async fn execute(&self) -> Result<()> {
        println!("Template use command - not yet implemented");
        println!("  name: {}", self.name);
        println!("  variables: {:?}", self.parse_variables()?);
        println!("  dry_run: {}", self.dry_run);
        println!("  model: {:?}", self.model);
        println!("  budget: {:?}", self.budget);
        println!("  yolo: {}", self.yolo);
        Ok(())
    }
}

impl TemplateCreateArgs {
    pub async fn execute(&self) -> Result<()> {
        println!("Template create command - not yet implemented");
        println!("  name: {}", self.name);
        println!("  from_file: {:?}", self.from_file);
        println!("  category: {:?}", self.category);
        println!("  description: {:?}", self.description);
        Ok(())
    }
}

impl TemplateDeleteArgs {
    pub async fn execute(&self) -> Result<()> {
        println!("Template delete command - not yet implemented");
        println!("  name: {}", self.name);
        println!("  force: {}", self.force);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Wrapper for testing subcommands
    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(subcommand)]
        command: TemplateCommand,
    }

    #[test]
    fn test_template_list_basic() {
        let cli = TestCli::try_parse_from(["test", "list"]).unwrap();

        match cli.command {
            TemplateCommand::List(args) => {
                assert!(args.category.is_none());
                assert!(args.tag.is_none());
                assert!(!args.builtin_only);
                assert!(!args.user_only);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_template_list_with_category() {
        let cli = TestCli::try_parse_from(["test", "list", "--category", "scaffold"]).unwrap();

        match cli.command {
            TemplateCommand::List(args) => {
                assert_eq!(args.category, Some("scaffold".to_string()));
                assert!(args.tag.is_none());
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_template_list_with_tag() {
        let cli = TestCli::try_parse_from(["test", "list", "--tag", "rust"]).unwrap();

        match cli.command {
            TemplateCommand::List(args) => {
                assert!(args.category.is_none());
                assert_eq!(args.tag, Some("rust".to_string()));
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_template_list_builtin_only() {
        let cli = TestCli::try_parse_from(["test", "list", "--builtin-only"]).unwrap();

        match cli.command {
            TemplateCommand::List(args) => {
                assert!(args.builtin_only);
                assert!(!args.user_only);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_template_list_user_only() {
        let cli = TestCli::try_parse_from(["test", "list", "--user-only"]).unwrap();

        match cli.command {
            TemplateCommand::List(args) => {
                assert!(!args.builtin_only);
                assert!(args.user_only);
            }
            _ => panic!("Expected List command"),
        }
    }

    #[test]
    fn test_template_show() {
        let cli = TestCli::try_parse_from(["test", "show", "api-endpoint"]).unwrap();

        match cli.command {
            TemplateCommand::Show(args) => {
                assert_eq!(args.name, "api-endpoint");
            }
            _ => panic!("Expected Show command"),
        }
    }

    #[test]
    fn test_template_use_basic() {
        let cli = TestCli::try_parse_from(["test", "use", "api-endpoint"]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert_eq!(args.name, "api-endpoint");
                assert!(args.var.is_empty());
                assert!(!args.dry_run);
                assert!(args.model.is_none());
                assert!(args.budget.is_none());
                assert!(!args.yolo);
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_use_with_variables() {
        let cli = TestCli::try_parse_from([
            "test", "use", "api-endpoint",
            "--var", "name=users",
            "--var", "path=/v1"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert_eq!(args.name, "api-endpoint");
                assert_eq!(args.var, vec!["name=users", "path=/v1"]);

                let vars = args.parse_variables().unwrap();
                assert_eq!(vars.get("name"), Some(&"users".to_string()));
                assert_eq!(vars.get("path"), Some(&"/v1".to_string()));
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_use_variable_parsing() {
        let args = TemplateUseArgs {
            name: "test".to_string(),
            var: vec!["key1=value1".to_string(), "key2=value2".to_string()],
            dry_run: false,
            model: None,
            budget: None,
            yolo: false,
        };

        let vars = args.parse_variables().unwrap();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars.get("key1"), Some(&"value1".to_string()));
        assert_eq!(vars.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_template_use_variable_parsing_invalid() {
        let args = TemplateUseArgs {
            name: "test".to_string(),
            var: vec!["invalid_format".to_string()],
            dry_run: false,
            model: None,
            budget: None,
            yolo: false,
        };

        let result = args.parse_variables();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid variable format"));
    }

    #[test]
    fn test_template_use_with_dry_run() {
        let cli = TestCli::try_parse_from([
            "test", "use", "api-endpoint",
            "--dry-run"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert!(args.dry_run);
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_use_with_model() {
        let cli = TestCli::try_parse_from([
            "test", "use", "api-endpoint",
            "--model", "opus"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert_eq!(args.model, Some(ModelAlias::Opus));
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_use_with_budget() {
        let cli = TestCli::try_parse_from([
            "test", "use", "api-endpoint",
            "--budget", "5.0"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert_eq!(args.budget, Some(5.0));
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_use_with_yolo() {
        let cli = TestCli::try_parse_from([
            "test", "use", "api-endpoint",
            "--yolo"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Use(args) => {
                assert!(args.yolo);
            }
            _ => panic!("Expected Use command"),
        }
    }

    #[test]
    fn test_template_create_basic() {
        let cli = TestCli::try_parse_from(["test", "create", "my-template"]).unwrap();

        match cli.command {
            TemplateCommand::Create(args) => {
                assert_eq!(args.name, "my-template");
                assert!(args.from_file.is_none());
                assert!(args.category.is_none());
                assert!(args.description.is_none());
            }
            _ => panic!("Expected Create command"),
        }
    }

    #[test]
    fn test_template_create_from_file() {
        let cli = TestCli::try_parse_from([
            "test", "create", "my-template",
            "--from-file", "template.yaml"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Create(args) => {
                assert_eq!(args.name, "my-template");
                assert_eq!(args.from_file, Some("template.yaml".to_string()));
            }
            _ => panic!("Expected Create command"),
        }
    }

    #[test]
    fn test_template_create_with_options() {
        let cli = TestCli::try_parse_from([
            "test", "create", "my-template",
            "--category", "scaffold",
            "--description", "My custom template"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Create(args) => {
                assert_eq!(args.name, "my-template");
                assert_eq!(args.category, Some("scaffold".to_string()));
                assert_eq!(args.description, Some("My custom template".to_string()));
            }
            _ => panic!("Expected Create command"),
        }
    }

    #[test]
    fn test_template_delete_basic() {
        let cli = TestCli::try_parse_from(["test", "delete", "my-template"]).unwrap();

        match cli.command {
            TemplateCommand::Delete(args) => {
                assert_eq!(args.name, "my-template");
                assert!(!args.force);
            }
            _ => panic!("Expected Delete command"),
        }
    }

    #[test]
    fn test_template_delete_force() {
        let cli = TestCli::try_parse_from([
            "test", "delete", "my-template",
            "--force"
        ]).unwrap();

        match cli.command {
            TemplateCommand::Delete(args) => {
                assert_eq!(args.name, "my-template");
                assert!(args.force);
            }
            _ => panic!("Expected Delete command"),
        }
    }
}
