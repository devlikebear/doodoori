use anyhow::Result;
use clap::Args;

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
}

impl SpecArgs {
    pub async fn execute(&self) -> Result<()> {
        if let Some(ref validate_path) = self.validate {
            return self.validate_spec(validate_path).await;
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

        Ok(())
    }

    async fn generate_spec(&self, description: &str) -> Result<()> {
        let output = self.output.clone().unwrap_or_else(|| "spec.md".to_string());

        println!("Generating spec for: {}", description);
        println!("Output: {}", output);

        // TODO: Use Claude to generate spec
        let spec_content = format!(
            r#"# Task: {}

## Objective
{}

## Model
sonnet

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2
- [ ] Requirement 3

## Constraints
- Constraint 1
- Constraint 2

## Completion Criteria
All requirements implemented and tested

## Max Iterations
50

## Completion Promise
<promise>COMPLETE</promise>
"#,
            description, description
        );

        tokio::fs::write(&output, spec_content).await?;
        println!("✓ Spec file created: {}", output);

        Ok(())
    }

    async fn interactive_generate(&self) -> Result<()> {
        println!("Interactive spec generation not yet implemented");
        // TODO: Interactive prompts for spec creation
        Ok(())
    }

    async fn validate_spec(&self, path: &str) -> Result<()> {
        println!("Validating spec: {}", path);

        let content = tokio::fs::read_to_string(path).await?;

        // Basic validation
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if !content.contains("# Task:") && !content.contains("# Spec:") {
            errors.push("Missing title (# Task: or # Spec:)");
        }

        if !content.contains("## Objective") {
            errors.push("Missing ## Objective section");
        }

        if !content.contains("## Requirements") {
            warnings.push("Missing ## Requirements section");
        }

        if !content.contains("<promise>") {
            warnings.push("Missing completion promise (<promise>...</promise>)");
        }

        if errors.is_empty() {
            println!("✓ Spec is valid");
        } else {
            println!("✗ Validation errors:");
            for err in &errors {
                println!("  - {}", err);
            }
        }

        if !warnings.is_empty() {
            println!("⚠ Warnings:");
            for warn in &warnings {
                println!("  - {}", warn);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!("Spec validation failed")
        }
    }
}
