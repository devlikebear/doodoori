# Template CLI Implementation

## Objective
Implement the stub methods in `src/cli/commands/template.rs` to make all template CLI commands functional.

## Dependencies
- `src/templates/storage.rs` - TemplateStorage
- `src/templates/mod.rs` - Template, TemplateCategory, TemplateVariable

## Requirements

### 1. TemplateListArgs::execute()

Display all available templates grouped by category.

```rust
impl TemplateListArgs {
    pub async fn execute(&self) -> Result<()> {
        use crate::templates::storage::TemplateStorage;
        use console::{style, Emoji};

        let storage = TemplateStorage::new()?;
        let mut templates = storage.list();

        // Apply filters
        if let Some(ref category) = self.category {
            // Filter by category (case-insensitive)
        }
        if let Some(ref tag) = self.tag {
            // Filter by tag
        }

        // Group by category and display
        // Format:
        // === Templates ===
        //
        // [Scaffold]
        //   api-endpoint     Create a REST API endpoint
        //   cli-command      Create a new CLI subcommand
        //
        // [Test]
        //   add-tests        Add unit tests for a file
    }
}
```

### 2. TemplateShowArgs::execute()

Show detailed information about a specific template.

```rust
impl TemplateShowArgs {
    pub async fn execute(&self) -> Result<()> {
        // Load template by name
        // Display:
        // === Template: api-endpoint ===
        //
        // Description: Create a REST API endpoint with CRUD operations
        // Category:    Scaffold
        // Tags:        rust, api, web, crud
        // Model:       sonnet (default)
        //
        // Variables:
        //   - resource (required): Name of the resource
        //   - path_prefix (optional): API path prefix [default: /api/v1]
        //
        // Prompt:
        // ---
        // Create a REST API endpoint for the "{resource}" resource...
        // ---
    }
}
```

### 3. TemplateUseArgs::execute()

Render template and optionally execute it.

```rust
impl TemplateUseArgs {
    pub async fn execute(&self) -> Result<()> {
        // 1. Load template
        // 2. Parse variables from --var flags
        // 3. Validate required variables
        // 4. Render template with variables
        // 5. If --dry-run: print rendered prompt
        // 6. Otherwise: execute via RunArgs with rendered prompt
    }
}
```

### 4. TemplateCreateArgs::execute()

Create a new user template.

```rust
impl TemplateCreateArgs {
    pub async fn execute(&self) -> Result<()> {
        // If --from-file: load and save template from YAML file
        // Otherwise: create minimal template with provided name/category/description
        // Save to ~/.doodoori/templates/{name}.yaml
    }
}
```

### 5. TemplateDeleteArgs::execute()

Delete a user template.

```rust
impl TemplateDeleteArgs {
    pub async fn execute(&self) -> Result<()> {
        // Check if template exists
        // If not --force: ask for confirmation
        // Delete template file
    }
}
```

## Constraints

- Use `console` crate for styled output (style, Emoji)
- Use `TemplateStorage` for all template operations
- Follow existing CLI patterns in the codebase
- Add unit tests for each command

## Tests

Add tests to verify:
- List command displays templates correctly
- Show command displays template details
- Use command renders variables correctly
- Create command saves template file
- Delete command removes template file

## Files to Modify

- `src/cli/commands/template.rs` - Implement execute() methods
