use anyhow::Result;
use clap::Args;
use std::path::Path;

use crate::instructions::{validate, SpecParser};

/// Generate or manage spec files
#[derive(Args, Debug)]
pub struct SpecArgs {
    /// Description for spec generation
    pub description: Option<String>,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<String>,

    /// Interactive mode for spec generation
    #[arg(short, long)]
    pub interactive: bool,

    /// Validate an existing spec file
    #[arg(long)]
    pub validate: Option<String>,

    /// Show parsed spec info
    #[arg(long)]
    pub info: Option<String>,
}

impl SpecArgs {
    pub async fn execute(&self) -> Result<()> {
        if let Some(ref validate_path) = self.validate {
            return self.validate_spec(validate_path).await;
        }

        if let Some(ref info_path) = self.info {
            return self.show_spec_info(info_path).await;
        }

        if self.interactive {
            return self.interactive_generate().await;
        }

        if let Some(ref desc) = self.description {
            return self.generate_spec(desc).await;
        }

        println!("Usage: doodoori spec <description> [-o output.md]");
        println!("       doodoori spec --interactive [-o output.md]");
        println!("       doodoori spec --validate <spec.md>");
        println!("       doodoori spec --info <spec.md>");

        Ok(())
    }

    async fn generate_spec(&self, description: &str) -> Result<()> {
        let output = self.output.clone().unwrap_or_else(|| "spec.md".to_string());

        println!("Generating spec for: {}", description);

        // Generate spec using the parser
        let spec = SpecParser::generate_spec(description, None);
        let content = SpecParser::to_markdown(&spec);

        tokio::fs::write(&output, content).await?;
        println!("Spec file created: {}", output);

        Ok(())
    }

    async fn interactive_generate(&self) -> Result<()> {
        println!("Interactive spec generation not yet implemented");
        println!("Use: doodoori spec \"<description>\" instead");
        Ok(())
    }

    async fn validate_spec(&self, path: &str) -> Result<()> {
        println!("Validating spec: {}\n", path);

        // Parse the spec file
        let spec = SpecParser::parse_file(Path::new(path))?;

        // Run validation
        let result = validate(&spec);

        // Show parsed info
        println!("Title: {}", spec.title);
        println!("Model: {:?}", spec.effective_model());
        println!("Max iterations: {}", spec.effective_max_iterations());
        if !spec.requirements.is_empty() {
            println!("Requirements: {}", spec.requirements.len());
        }
        if !spec.tasks.is_empty() {
            println!("Tasks: {}", spec.tasks.len());
        }
        println!();

        // Show validation results
        if result.is_valid() {
            println!("Validation: PASSED");
        } else {
            println!("Validation: FAILED");
            println!("\nErrors:");
            for err in &result.errors {
                println!("  - [{}] {}", err.field, err.message);
            }
        }

        if result.has_warnings() {
            println!("\nWarnings:");
            for warn in &result.warnings {
                println!("  - [{}] {}", warn.field, warn.message);
            }
        }

        if result.is_valid() {
            Ok(())
        } else {
            anyhow::bail!("Spec validation failed")
        }
    }

    async fn show_spec_info(&self, path: &str) -> Result<()> {
        println!("Spec Info: {}\n", path);

        let spec = SpecParser::parse_file(Path::new(path))?;

        println!("Title: {}", spec.title);
        println!("Objective: {}", spec.objective);
        println!("Model: {:?}", spec.effective_model());
        println!("Max Iterations: {}", spec.effective_max_iterations());
        println!("Completion Promise: {}", spec.effective_completion_promise());

        if let Some(budget) = spec.budget {
            println!("Budget: ${:.2}", budget);
        }

        if !spec.requirements.is_empty() {
            println!("\nRequirements ({}):", spec.requirements.len());
            for (i, req) in spec.requirements.iter().enumerate() {
                let status = if req.completed { "[x]" } else { "[ ]" };
                println!("  {}. {} {}", i + 1, status, req.description);
            }
        }

        if !spec.constraints.is_empty() {
            println!("\nConstraints ({}):", spec.constraints.len());
            for constraint in &spec.constraints {
                println!("  - {}", constraint);
            }
        }

        if !spec.tasks.is_empty() {
            println!("\nTasks ({}):", spec.tasks.len());
            for task in &spec.tasks {
                let deps = if task.depends_on.is_empty() {
                    "none".to_string()
                } else {
                    task.depends_on.join(", ")
                };
                println!(
                    "  - {} (model: {:?}, priority: {}, deps: {})",
                    task.id,
                    task.model.as_ref().unwrap_or(&spec.effective_model()),
                    task.priority,
                    deps
                );
            }
        }

        Ok(())
    }
}
