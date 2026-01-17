# Built-in Templates

## Objective
Create a set of useful built-in templates that are embedded in the binary.

## Dependencies
- `01-template-core.md` 완료 필요

## Requirements

### 1. Template Files Location
`src/templates/builtin/` 디렉토리에 YAML 파일로 저장

### 2. Scaffold Templates

#### `api-endpoint.yaml`
```yaml
name: api-endpoint
description: Create a REST API endpoint with CRUD operations
category: scaffold
tags: [rust, api, web, crud]
default_model: sonnet
variables:
  - name: resource
    description: Name of the resource (e.g., users, posts)
    required: true
  - name: path_prefix
    description: API path prefix
    default: /api/v1
prompt: |
  Create a REST API endpoint for the "{resource}" resource.

  Requirements:
  - Path prefix: {path_prefix}
  - Implement CRUD operations (Create, Read, Update, Delete)
  - Use appropriate HTTP methods (POST, GET, PUT/PATCH, DELETE)
  - Include proper error handling
  - Add input validation
  - Follow RESTful conventions

  Look at existing API endpoints in this codebase for patterns and consistency.
```

#### `react-component.yaml`
```yaml
name: react-component
description: Create a React component with TypeScript
category: scaffold
tags: [react, typescript, frontend]
default_model: sonnet
variables:
  - name: name
    description: Component name (PascalCase)
    required: true
  - name: type
    description: Component type
    default: functional
prompt: |
  Create a React {type} component named "{name}".

  Requirements:
  - Use TypeScript with proper type definitions
  - Include props interface
  - Add JSDoc comments
  - Follow existing component patterns in the codebase
  - Include basic styling setup
```

#### `cli-command.yaml`
```yaml
name: cli-command
description: Create a new CLI subcommand
category: scaffold
tags: [rust, cli, clap]
default_model: sonnet
variables:
  - name: name
    description: Command name
    required: true
  - name: description
    description: Command description
    required: true
prompt: |
  Create a new CLI subcommand "{name}" for this Rust CLI application.

  Description: {description}

  Requirements:
  - Use clap derive macros
  - Follow existing command patterns in src/cli/commands/
  - Add appropriate arguments and flags
  - Include help text for all options
  - Implement the execute method
  - Add unit tests
```

### 3. Refactor Templates

#### `extract-function.yaml`
```yaml
name: extract-function
description: Extract code into a separate function
category: refactor
tags: [refactor, clean-code]
variables:
  - name: file
    description: File path to refactor
    required: true
  - name: description
    description: Description of the code to extract
    required: true
prompt: |
  Refactor the code in "{file}".

  Task: {description}

  Requirements:
  - Extract the relevant code into a well-named function
  - Ensure proper parameter passing
  - Maintain existing functionality
  - Add documentation comments
  - Update any callers if necessary
```

#### `clean-imports.yaml`
```yaml
name: clean-imports
description: Clean up and organize imports
category: refactor
tags: [refactor, imports, cleanup]
variables:
  - name: path
    description: File or directory path
    required: true
prompt: |
  Clean up imports in "{path}".

  Requirements:
  - Remove unused imports
  - Sort imports alphabetically
  - Group imports by category (std, external, internal)
  - Follow the project's import style conventions
```

### 4. Test Templates

#### `add-tests.yaml`
```yaml
name: add-tests
description: Add unit tests for a file or module
category: test
tags: [test, unit-test, tdd]
variables:
  - name: file
    description: File to add tests for
    required: true
  - name: coverage
    description: Target coverage level
    default: "80%"
prompt: |
  Add unit tests for "{file}".

  Requirements:
  - Achieve at least {coverage} code coverage
  - Test all public functions and methods
  - Include edge cases and error conditions
  - Use descriptive test names
  - Follow existing test patterns in the codebase
  - Use appropriate mocking where necessary
```

#### `integration-test.yaml`
```yaml
name: integration-test
description: Add integration tests
category: test
tags: [test, integration, e2e]
variables:
  - name: feature
    description: Feature to test
    required: true
prompt: |
  Create integration tests for the "{feature}" feature.

  Requirements:
  - Test the feature end-to-end
  - Include setup and teardown
  - Test success and failure scenarios
  - Use realistic test data
  - Follow existing integration test patterns
```

### 5. Fix Templates

#### `fix-bug.yaml`
```yaml
name: fix-bug
description: Fix a bug in the codebase
category: fix
tags: [fix, bug, debug]
variables:
  - name: description
    description: Bug description
    required: true
  - name: file
    description: File where bug is located (optional)
    required: false
prompt: |
  Fix the following bug: {description}

  {file ? "Location hint: " + file : ""}

  Requirements:
  - Identify the root cause
  - Implement a fix that doesn't break existing functionality
  - Add or update tests to prevent regression
  - Document the fix in code comments if complex
```

### 6. Docs Templates

#### `add-docs.yaml`
```yaml
name: add-docs
description: Add documentation to code
category: docs
tags: [docs, documentation, comments]
variables:
  - name: path
    description: File or module path
    required: true
prompt: |
  Add documentation to "{path}".

  Requirements:
  - Add doc comments to all public items
  - Include examples in documentation where helpful
  - Document parameters, return values, and errors
  - Follow Rust doc conventions (/// and //!)
  - Keep documentation concise but informative
```

### 7. Loading Built-in Templates

```rust
// src/templates/builtin.rs
pub fn load_builtin_templates() -> Vec<Template> {
    vec![
        include_template!("builtin/api-endpoint.yaml"),
        include_template!("builtin/react-component.yaml"),
        include_template!("builtin/cli-command.yaml"),
        include_template!("builtin/extract-function.yaml"),
        include_template!("builtin/clean-imports.yaml"),
        include_template!("builtin/add-tests.yaml"),
        include_template!("builtin/integration-test.yaml"),
        include_template!("builtin/fix-bug.yaml"),
        include_template!("builtin/add-docs.yaml"),
    ]
}

macro_rules! include_template {
    ($path:expr) => {{
        let yaml = include_str!($path);
        serde_yaml::from_str(yaml).expect(concat!("Failed to parse ", $path))
    }};
}
```

## Files to Create
- `src/templates/builtin.rs`
- `src/templates/builtin/api-endpoint.yaml`
- `src/templates/builtin/react-component.yaml`
- `src/templates/builtin/cli-command.yaml`
- `src/templates/builtin/extract-function.yaml`
- `src/templates/builtin/clean-imports.yaml`
- `src/templates/builtin/add-tests.yaml`
- `src/templates/builtin/integration-test.yaml`
- `src/templates/builtin/fix-bug.yaml`
- `src/templates/builtin/add-docs.yaml`

## Tests
- 모든 내장 템플릿 로드 테스트
- YAML 파싱 유효성 테스트
- 필수 필드 존재 확인 테스트
