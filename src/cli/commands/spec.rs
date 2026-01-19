use anyhow::Result;
use clap::Args;
use console::{style, Emoji};
use std::io::{self, Write};
use std::path::Path;

use crate::claude::ModelAlias;
use crate::instructions::{validate, Requirement, SpecFile, SpecParser};

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
        println!(
            "\n{} {}",
            Emoji("📝", ""),
            style("Interactive Spec Generator").bold().cyan()
        );
        println!("{}\n", style("─".repeat(40)).dim());

        let mut spec = SpecFile::default();

        // 1. Title
        spec.title = self.prompt_required("Task title")?;

        // 2. Objective
        println!(
            "\n{} (press Enter twice to finish):",
            style("Objective").bold()
        );
        spec.objective = self.prompt_multiline()?;

        // 3. Model selection
        spec.model = Some(self.prompt_model()?);

        // 4. Requirements
        println!(
            "\n{} (empty line to finish):",
            style("Requirements").bold()
        );
        spec.requirements = self.prompt_requirements()?;

        // 5. Constraints (optional)
        println!(
            "\n{} (empty line to skip/finish):",
            style("Constraints (optional)").bold()
        );
        spec.constraints = self.prompt_list()?;

        // 6. Budget (optional)
        if let Some(budget) = self.prompt_optional_number("Budget in USD (optional)")? {
            spec.budget = Some(budget);
        }

        // 7. Max iterations
        spec.max_iterations = Some(
            self.prompt_optional_number("Max iterations")?
                .map(|n| n as u32)
                .unwrap_or(50),
        );

        // 8. Completion promise
        spec.completion_promise = Some("<promise>COMPLETE</promise>".to_string());

        // Preview
        println!("\n{}", style("─".repeat(40)).dim());
        println!(
            "{} {}",
            Emoji("👁️", ""),
            style("Preview").bold().green()
        );
        println!("{}\n", style("─".repeat(40)).dim());

        let content = SpecParser::to_markdown(&spec);
        println!("{}", content);

        // Confirm
        println!("{}", style("─".repeat(40)).dim());
        if !self.prompt_confirm("Save this spec?")? {
            println!("{} Cancelled", Emoji("❌", ""));
            return Ok(());
        }

        // Save
        let output = self.output.clone().unwrap_or_else(|| "spec.md".to_string());
        tokio::fs::write(&output, &content).await?;

        println!(
            "\n{} Spec file created: {}",
            Emoji("✅", ""),
            style(&output).green()
        );

        Ok(())
    }

    /// Prompt for a required string input
    fn prompt_required(&self, label: &str) -> Result<String> {
        loop {
            print!("{}: ", style(label).bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if !input.is_empty() {
                return Ok(input);
            }

            println!("{} This field is required", style("!").red());
        }
    }

    /// Prompt for multiline input (ends with empty line)
    fn prompt_multiline(&self) -> Result<String> {
        let mut lines = Vec::new();
        let mut empty_count = 0;

        loop {
            print!("  ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let line = input.trim_end_matches('\n').trim_end_matches('\r');

            if line.is_empty() {
                empty_count += 1;
                if empty_count >= 1 && !lines.is_empty() {
                    break;
                }
            } else {
                empty_count = 0;
                lines.push(line.to_string());
            }
        }

        Ok(lines.join("\n"))
    }

    /// Prompt for model selection
    fn prompt_model(&self) -> Result<ModelAlias> {
        println!("\n{}", style("Select model").bold());
        println!("  1. {} (fastest, cheapest)", style("haiku").cyan());
        println!("  2. {} (balanced, recommended)", style("sonnet").green());
        println!("  3. {} (most capable)", style("opus").magenta());

        loop {
            print!("Choice [1-3, default=2]: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            match input {
                "" | "2" => return Ok(ModelAlias::Sonnet),
                "1" => return Ok(ModelAlias::Haiku),
                "3" => return Ok(ModelAlias::Opus),
                _ => println!("{} Please enter 1, 2, or 3", style("!").red()),
            }
        }
    }

    /// Prompt for a list of requirements
    fn prompt_requirements(&self) -> Result<Vec<Requirement>> {
        let mut requirements = Vec::new();
        let mut index = 1;

        loop {
            print!("  {}. ", index);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                break;
            }

            requirements.push(Requirement::new(input));
            index += 1;
        }

        Ok(requirements)
    }

    /// Prompt for a list of strings
    fn prompt_list(&self) -> Result<Vec<String>> {
        let mut items = Vec::new();

        loop {
            print!("  - ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();

            if input.is_empty() {
                break;
            }

            items.push(input);
        }

        Ok(items)
    }

    /// Prompt for an optional number
    fn prompt_optional_number(&self, label: &str) -> Result<Option<f64>> {
        print!("{}: ", style(label).bold());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(None);
        }

        match input.parse::<f64>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => {
                println!("{} Invalid number, skipping", style("!").yellow());
                Ok(None)
            }
        }
    }

    /// Prompt for yes/no confirmation
    fn prompt_confirm(&self, label: &str) -> Result<bool> {
        loop {
            print!("{} [Y/n]: ", style(label).bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();

            match input.as_str() {
                "" | "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => println!("{} Please enter y or n", style("!").red()),
            }
        }
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
