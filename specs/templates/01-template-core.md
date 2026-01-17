# Template Core Module

## Objective
Create the core template module with data structures and storage management.

## Requirements

### 1. Data Structures (`src/templates/mod.rs`)

```rust
pub struct Template {
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub prompt: String,
    pub variables: Vec<TemplateVariable>,
    pub default_model: Option<ModelAlias>,
    pub default_max_iterations: Option<u32>,
    pub tags: Vec<String>,
}

pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub default: Option<String>,
    pub required: bool,
}

pub enum TemplateCategory {
    Scaffold,    // 새 코드 생성
    Refactor,    // 리팩토링
    Test,        // 테스트 추가
    Fix,         // 버그 수정
    Docs,        // 문서화
    Custom,      // 사용자 정의
}
```

### 2. Template Storage (`src/templates/storage.rs`)

- 내장 템플릿: 바이너리에 포함 (include_str!)
- 사용자 템플릿: `~/.doodoori/templates/` 디렉토리
- 프로젝트 템플릿: `.doodoori/templates/` 디렉토리

```rust
pub struct TemplateStorage {
    builtin: Vec<Template>,
    user_dir: PathBuf,
    project_dir: Option<PathBuf>,
}

impl TemplateStorage {
    pub fn new() -> Result<Self>;
    pub fn list(&self) -> Vec<&Template>;
    pub fn get(&self, name: &str) -> Option<&Template>;
    pub fn save_user_template(&self, template: &Template) -> Result<()>;
    pub fn delete_user_template(&self, name: &str) -> Result<()>;
}
```

### 3. Template Rendering

```rust
impl Template {
    pub fn render(&self, variables: &HashMap<String, String>) -> Result<String>;
    pub fn validate_variables(&self, variables: &HashMap<String, String>) -> Result<()>;
}
```

## Constraints
- Rust 2024 edition
- 에러 처리: anyhow, thiserror
- 직렬화: serde (YAML 형식)
- 테스트 커버리지 필수

## Tests
- Template 구조체 생성 테스트
- 변수 렌더링 테스트
- 스토리지 저장/로드 테스트
- 카테고리별 필터링 테스트

## Files to Create
- `src/templates/mod.rs`
- `src/templates/storage.rs`
