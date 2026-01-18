# Template CLI Commands

## Objective
Implement CLI commands for template management: list, show, use, create, delete.

## Dependencies
- `01-template-core.md` 완료 필요

## Requirements

### 1. CLI Command Structure (`src/cli/commands/template.rs`)

```rust
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
```

### 2. List Command

```bash
doodoori template list
doodoori template list --category scaffold
doodoori template list --tag rust
```

```rust
#[derive(Args, Debug)]
pub struct TemplateListArgs {
    #[arg(short, long)]
    pub category: Option<String>,

    #[arg(short, long)]
    pub tag: Option<String>,

    #[arg(long)]
    pub builtin_only: bool,

    #[arg(long)]
    pub user_only: bool,
}
```

**출력 형식:**
```
=== Templates ===

[Scaffold]
  api-endpoint     Create a REST API endpoint
  react-component  Create a React component

[Test]
  add-tests        Add unit tests for a file
  integration      Add integration tests

[Refactor]
  extract-func     Extract function from code
  clean-imports    Clean up imports
```

### 3. Show Command

```bash
doodoori template show api-endpoint
```

**출력:**
```
=== Template: api-endpoint ===

Description: Create a REST API endpoint with CRUD operations
Category:    Scaffold
Model:       sonnet
Tags:        rust, api, web

Variables:
  - name (required): Name of the resource
  - path (optional): API path prefix [default: /api]

Prompt:
---
Create a REST API endpoint for {name} resource...
---
```

### 4. Use Command

```bash
doodoori template use api-endpoint --var name=users --var path=/v1
doodoori template use api-endpoint name=users  # 간단한 형식
```

```rust
#[derive(Args, Debug)]
pub struct TemplateUseArgs {
    pub name: String,

    #[arg(short, long)]
    pub var: Vec<String>,  // key=value 형식

    /// 바로 실행하지 않고 프롬프트만 출력
    #[arg(long)]
    pub dry_run: bool,

    /// 실행 옵션
    #[arg(short, long)]
    pub model: Option<ModelAlias>,

    #[arg(long)]
    pub budget: Option<f64>,

    #[arg(long)]
    pub yolo: bool,
}
```

### 5. Create Command

```bash
doodoori template create my-template
doodoori template create my-template --from-file template.yaml
```

대화형 또는 파일에서 템플릿 생성.

### 6. Delete Command

```bash
doodoori template delete my-template
doodoori template delete my-template --force
```

## Constraints
- clap derive 매크로 사용
- 기존 CLI 패턴과 일관성 유지
- 에러 메시지 명확하게

## Tests
- 각 명령어 파싱 테스트
- 변수 파싱 테스트 (key=value)
- 드라이런 출력 테스트

## Files to Create/Modify
- `src/cli/commands/template.rs` (신규)
- `src/cli/commands/mod.rs` (수정 - template 모듈 추가)
- `src/cli/mod.rs` (수정 - Template 명령 추가)
