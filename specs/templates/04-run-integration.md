# Template Integration with Run Command

## Objective
Integrate template system with the `doodoori run` command via `--template` flag.

## Dependencies
- `01-template-core.md` 완료 필요
- `03-builtin-templates.md` 완료 필요

## Requirements

### 1. Run Command Extension

`src/cli/commands/run.rs` 수정:

```rust
#[derive(Args, Debug)]
pub struct RunArgs {
    // 기존 필드들...

    /// Use a template instead of direct prompt
    #[arg(long, short = 't')]
    pub template: Option<String>,

    /// Template variables (key=value format)
    #[arg(long = "var", short = 'V')]
    pub template_vars: Vec<String>,
}
```

### 2. Usage Examples

```bash
# 템플릿으로 실행
doodoori run --template api-endpoint --var resource=users

# 간단한 변수 형식
doodoori run -t add-tests -V file=src/utils.rs

# 템플릿 + 추가 옵션
doodoori run -t fix-bug -V description="Login fails" --model opus --budget 5.0

# 템플릿과 함께 추가 지시사항
doodoori run -t refactor -V file=src/main.rs "Also improve error handling"
```

### 3. Implementation Logic

```rust
impl RunArgs {
    pub async fn execute(self) -> Result<()> {
        let prompt = if let Some(template_name) = &self.template {
            // 템플릿 로드
            let storage = TemplateStorage::new()?;
            let template = storage.get(template_name)
                .ok_or_else(|| anyhow!("Template not found: {}", template_name))?;

            // 변수 파싱
            let vars = self.parse_template_vars()?;

            // 변수 검증
            template.validate_variables(&vars)?;

            // 프롬프트 렌더링
            let mut rendered = template.render(&vars)?;

            // 추가 프롬프트가 있으면 합치기
            if let Some(ref additional) = self.prompt {
                rendered = format!("{}\n\nAdditional instructions:\n{}", rendered, additional);
            }

            rendered
        } else if let Some(ref prompt) = self.prompt {
            prompt.clone()
        } else if let Some(ref spec) = self.spec {
            // 기존 spec 로직
            self.load_spec_prompt(spec)?
        } else {
            anyhow::bail!("Either --prompt, --spec, or --template is required");
        };

        // 나머지 실행 로직...
    }

    fn parse_template_vars(&self) -> Result<HashMap<String, String>> {
        let mut vars = HashMap::new();
        for var_str in &self.template_vars {
            let parts: Vec<&str> = var_str.splitn(2, '=').collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid variable format: {}. Expected key=value", var_str);
            }
            vars.insert(parts[0].to_string(), parts[1].to_string());
        }
        Ok(vars)
    }
}
```

### 4. Template Default Overrides

템플릿의 기본값을 CLI 옵션으로 오버라이드:

```rust
// 템플릿에 default_model이 있어도 --model 옵션이 우선
let model = self.model
    .or(template.default_model)
    .unwrap_or(ModelAlias::Sonnet);

// 템플릿에 default_max_iterations가 있어도 --max-iterations가 우선
let max_iterations = self.max_iterations
    .or(template.default_max_iterations)
    .unwrap_or(50);
```

### 5. Error Messages

친절한 에러 메시지:

```
Error: Template 'api-endpoint' requires variable 'resource'

Usage: doodoori run -t api-endpoint --var resource=<value>

Available variables:
  - resource (required): Name of the resource (e.g., users, posts)
  - path_prefix (optional): API path prefix [default: /api/v1]
```

### 6. Dry Run Support

```bash
doodoori run --template api-endpoint --var resource=users --dry-run
```

출력:
```
=== Template: api-endpoint ===
Category: scaffold
Model: sonnet

=== Rendered Prompt ===
Create a REST API endpoint for the "users" resource.

Requirements:
- Path prefix: /api/v1
- Implement CRUD operations...
[truncated]

=== Would execute with ===
Model: sonnet
Max iterations: 50
Budget: unlimited
```

## Tests
- 템플릿 변수 파싱 테스트
- 템플릿 렌더링 통합 테스트
- 필수 변수 누락 에러 테스트
- 기본값 오버라이드 테스트
- 드라이런 출력 테스트

## Files to Modify
- `src/cli/commands/run.rs`
