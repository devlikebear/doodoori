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
        use crate::templates::storage::TemplateStorage;
        use crate::templates::TemplateCategory;
        use console::{style, Emoji};
        use std::collections::HashMap;

        let storage = TemplateStorage::new()?;
        let mut templates = storage.list();

        // Apply filters
        if let Some(ref category_str) = self.category {
            let category: TemplateCategory = serde_yaml::from_str(&format!("\"{}\"", category_str.to_lowercase()))?;
            templates.retain(|t| t.category == category);
        }
        if let Some(ref tag) = self.tag {
            templates.retain(|t| t.tags.iter().any(|t| t == tag));
        }
        if self.builtin_only {
            // Keep only builtin templates (category != Custom)
            templates.retain(|t| t.category != TemplateCategory::Custom);
        }
        if self.user_only {
            // Keep only user templates (category == Custom)
            templates.retain(|t| t.category == TemplateCategory::Custom);
        }

        if templates.is_empty() {
            println!("{} No templates found", Emoji("📋", "[i]"));
            return Ok(());
        }

        // Group by category
        let mut by_category: HashMap<TemplateCategory, Vec<_>> = HashMap::new();
        for template in templates {
            by_category.entry(template.category).or_default().push(template);
        }

        println!("{} Templates", Emoji("📋", "==="));
        println!();

        // Display each category
        for category in [
            TemplateCategory::Scaffold,
            TemplateCategory::Refactor,
            TemplateCategory::Test,
            TemplateCategory::Fix,
            TemplateCategory::Docs,
            TemplateCategory::Custom,
        ] {
            if let Some(templates) = by_category.get(&category) {
                println!("[{}]", style(format!("{:?}", category)).cyan().bold());
                let mut sorted_templates: Vec<crate::templates::Template> = templates.clone();
                sorted_templates.sort_by(|a, b| a.name.cmp(&b.name));

                for template in sorted_templates {
                    println!("  {:<20} {}",
                        style(&template.name).green(),
                        template.description
                    );
                }
                println!();
            }
        }

        Ok(())
    }
}

impl TemplateShowArgs {
    pub async fn execute(&self) -> Result<()> {
        use crate::templates::storage::TemplateStorage;
        use console::{style, Emoji};

        let storage = TemplateStorage::new()?;
        let template = storage.get(&self.name)
            .ok_or_else(|| anyhow::anyhow!("Template not found: {}", self.name))?;

        println!("{} Template: {}", Emoji("📋", "==="), style(&template.name).cyan().bold());
        println!();
        println!("Description: {}", template.description);
        println!("Category:    {:?}", template.category);
        if !template.tags.is_empty() {
            println!("Tags:        {}", template.tags.join(", "));
        }
        if let Some(ref model) = template.default_model {
            println!("Model:       {:?} (default)", model);
        }
        if let Some(max_iter) = template.default_max_iterations {
            println!("Max Iter:    {} (default)", max_iter);
        }
        println!();

        if !template.variables.is_empty() {
            println!("Variables:");
            for var in &template.variables {
                let required = if var.required {
                    style("required").red().to_string()
                } else {
                    style("optional").dim().to_string()
                };

                if let Some(ref default) = var.default {
                    println!("  - {} ({}): {} [default: {}]",
                        style(&var.name).yellow(),
                        required,
                        var.description,
                        default
                    );
                } else {
                    println!("  - {} ({}): {}",
                        style(&var.name).yellow(),
                        required,
                        var.description
                    );
                }
            }
            println!();
        }

        println!("Prompt:");
        println!("{}", style("---").dim());
        println!("{}", template.prompt);
        println!("{}", style("---").dim());

        Ok(())
    }
}

impl TemplateUseArgs {
    pub async fn execute(&self) -> Result<()> {
        use crate::templates::storage::TemplateStorage;
        use console::{style, Emoji};

        // 1. Load template
        let storage = TemplateStorage::new()?;
        let template = storage.get(&self.name)
            .ok_or_else(|| anyhow::anyhow!("Template not found: {}", self.name))?;

        // 2. Parse variables from --var flags
        let variables = self.parse_variables()?;

        // 3. Validate required variables
        template.validate_variables(&variables)?;

        // 4. Render template with variables
        let rendered_prompt = template.render(&variables)?;

        // 5. If --dry-run: print rendered prompt
        if self.dry_run {
            println!("{} Template: {}", Emoji("📋", "==="), style(&template.name).cyan().bold());
            println!();
            println!("Rendered Prompt:");
            println!("{}", style("---").dim());
            println!("{}", rendered_prompt);
            println!("{}", style("---").dim());
            println!();
            println!("Would execute with:");
            println!("  Model:  {:?}", self.model.as_ref().or(template.default_model.as_ref()).unwrap_or(&crate::claude::ModelAlias::Sonnet));
            if let Some(budget) = self.budget {
                println!("  Budget: ${:.2}", budget);
            }
            if self.yolo {
                println!("  Mode:   YOLO");
            }
            return Ok(());
        }

        // 6. Otherwise: execute via RunArgs with rendered prompt
        println!("{} Executing template: {}", Emoji("🔨", ">>>"), style(&template.name).cyan());
        println!();

        // Build RunArgs from template
        let run_args = super::run::RunArgs {
            prompt: Some(rendered_prompt),
            spec: None,
            model: self.model.clone().unwrap_or_else(|| {
                template.default_model.clone().unwrap_or(crate::claude::ModelAlias::Sonnet)
            }),
            budget: self.budget,
            max_iterations: template.default_max_iterations.unwrap_or(50),
            sandbox: false,
            image: "doodoori/sandbox:latest".to_string(),
            network: "bridge".to_string(),
            dry_run: false,
            yolo: self.yolo,
            readonly: false,
            allow: None,
            instructions: None,
            no_instructions: false,
            no_git: false,
            no_auto_merge: false,
            verbose: false,
            no_hooks: false,
            notify: None,
            no_notify: false,
            format: "text".to_string(),
            output: None,
            template: None,
            template_vars: vec![],
            detach: false,
            internal_detached: false,
            #[cfg(feature = "dashboard")]
            dashboard: false,
        };

        run_args.execute().await
    }
}

impl TemplateCreateArgs {
    pub async fn execute(&self) -> Result<()> {
        use crate::templates::storage::TemplateStorage;
        use crate::templates::{Template, TemplateCategory};
        use console::{style, Emoji};
        use std::fs;

        let storage = TemplateStorage::new()?;

        // If --from-file: load and save template from YAML file
        if let Some(ref file_path) = self.from_file {
            let contents = fs::read_to_string(file_path)?;
            let mut template: Template = serde_yaml::from_str(&contents)?;

            // Override name if provided
            template.name = self.name.clone();

            // Save to user templates
            storage.save_user_template(&template)?;

            println!("{} Created template: {}", Emoji("✅", "[OK]"), style(&template.name).green());
            println!("  Source:      {}", file_path);
            println!("  Category:    {:?}", template.category);
            println!("  Description: {}", template.description);

            return Ok(());
        }

        // Otherwise: create minimal template with provided name/category/description
        let category = if let Some(ref cat_str) = self.category {
            serde_yaml::from_str(&format!("\"{}\"", cat_str.to_lowercase()))?
        } else {
            TemplateCategory::Custom
        };

        let description = self.description.clone()
            .unwrap_or_else(|| format!("Custom template: {}", self.name));

        let template = Template {
            name: self.name.clone(),
            description,
            category,
            prompt: "TODO: Add your prompt here with {variables}".to_string(),
            variables: vec![],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };

        storage.save_user_template(&template)?;

        println!("{} Created template: {}", Emoji("✅", "[OK]"), style(&template.name).green());
        println!("  Category:    {:?}", template.category);
        println!("  Description: {}", template.description);
        println!();
        println!("Edit the template file to customize:");
        println!("  ~/.doodoori/templates/{}.yaml", self.name);

        Ok(())
    }
}

impl TemplateDeleteArgs {
    pub async fn execute(&self) -> Result<()> {
        use crate::templates::storage::TemplateStorage;
        use crate::templates::TemplateCategory;
        use console::{style, Emoji};
        use std::io::{self, Write};

        let storage = TemplateStorage::new()?;

        // Check if template exists
        let template = storage.get(&self.name)
            .ok_or_else(|| anyhow::anyhow!("Template not found: {}", self.name))?;

        // Cannot delete built-in templates
        if template.category != TemplateCategory::Custom {
            anyhow::bail!("Cannot delete built-in template: {}\nOnly user templates (category: Custom) can be deleted.", self.name);
        }

        // If not --force: ask for confirmation
        if !self.force {
            print!("{} Delete template '{}'? [y/N] ",
                Emoji("⚠️", "[!]"),
                style(&self.name).yellow()
            );
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let confirmed = matches!(input.trim().to_lowercase().as_str(), "y" | "yes");
            if !confirmed {
                println!("Cancelled.");
                return Ok(());
            }
        }

        // Delete template file
        storage.delete_user_template(&self.name)?;

        println!("{} Deleted template: {}", Emoji("✅", "[OK]"), style(&self.name).green());

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

    // Integration tests for execute methods
    #[tokio::test]
    async fn test_list_execute() {
        let args = TemplateListArgs {
            category: None,
            tag: None,
            builtin_only: false,
            user_only: false,
        };

        // Should not error
        let result = args.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_execute_with_category() {
        let args = TemplateListArgs {
            category: Some("test".to_string()),
            tag: None,
            builtin_only: false,
            user_only: false,
        };

        let result = args.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_execute() {
        let args = TemplateShowArgs {
            name: "add-tests".to_string(),
        };

        let result = args.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_execute_not_found() {
        let args = TemplateShowArgs {
            name: "nonexistent-template-xyz".to_string(),
        };

        let result = args.execute().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Template not found"));
    }

    #[tokio::test]
    async fn test_use_dry_run() {
        let args = TemplateUseArgs {
            name: "add-tests".to_string(),
            var: vec!["file=test.rs".to_string()],
            dry_run: true,
            model: None,
            budget: None,
            yolo: false,
        };

        let result = args.execute().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_use_missing_required_variable() {
        let args = TemplateUseArgs {
            name: "add-tests".to_string(),
            var: vec![], // Missing required 'file' variable
            dry_run: true,
            model: None,
            budget: None,
            yolo: false,
        };

        let result = args.execute().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing required variable"));
    }

    #[tokio::test]
    async fn test_create_execute() {
        use crate::templates::storage::TemplateStorage;

        let args = TemplateCreateArgs {
            name: "test-template-integration".to_string(),
            from_file: None,
            category: Some("custom".to_string()),
            description: Some("Test template".to_string()),
        };

        let result = args.execute().await;
        assert!(result.is_ok());

        // Cleanup
        let storage = TemplateStorage::new().unwrap();
        let _ = storage.delete_user_template("test-template-integration");
    }

    #[tokio::test]
    async fn test_delete_execute() {
        use crate::templates::storage::TemplateStorage;
        use crate::templates::{Template, TemplateCategory};

        // Create a test template
        let storage = TemplateStorage::new().unwrap();
        let template = Template {
            name: "test-delete-template".to_string(),
            description: "Template to delete".to_string(),
            category: TemplateCategory::Custom,
            prompt: "Test".to_string(),
            variables: vec![],
            default_model: None,
            default_max_iterations: None,
            tags: vec![],
        };
        storage.save_user_template(&template).unwrap();

        // Delete it
        let args = TemplateDeleteArgs {
            name: "test-delete-template".to_string(),
            force: true,
        };

        let result = args.execute().await;
        assert!(result.is_ok());

        // Verify it's gone
        assert!(storage.get("test-delete-template").is_none());
    }

    #[tokio::test]
    async fn test_delete_builtin_fails() {
        let args = TemplateDeleteArgs {
            name: "add-tests".to_string(), // Built-in template
            force: true,
        };

        let result = args.execute().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot delete built-in template"));
    }
}
