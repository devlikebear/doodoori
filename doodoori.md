# Doodoori Development Instructions

이 문서는 doodoori가 Claude Code를 실행할 때 따라야 할 개발 규칙입니다.

## Core Principles

### 1. TDD (Test-Driven Development)
- **테스트를 먼저 작성**한 후 구현
- Red → Green → Refactor 사이클
- 테스트가 통과해야만 기능 완성

```rust
// 1. 먼저 테스트 작성 (Red)
#[test]
fn test_new_feature() {
    let result = new_feature();
    assert!(result.is_ok());
}

// 2. 구현 (Green)
pub fn new_feature() -> Result<()> {
    // implementation
}

// 3. 리팩토링 (Refactor)
```

### 2. Code Quality
- **cargo build** 성공 필수
- **cargo test** 모든 테스트 통과 필수
- **cargo clippy** 경고 없음
- **cargo fmt** 적용

### 3. Rust Style
- Rust 2024 edition
- 모든 public 함수에 문서 주석 (`///`)
- 에러 처리: `anyhow::Result`, `thiserror`
- 비동기: `tokio`, `async-trait`

## Project Structure

```
src/
├── cli/           # CLI 인터페이스 (clap)
├── claude/        # Claude Code CLI 래퍼
├── config/        # 설정 관리
├── executor/      # 실행 엔진
├── git/           # Git 워크플로우
├── hooks/         # Hook 시스템
├── instructions/  # Spec 파일 처리
├── loop_engine/   # 자기 개선 루프
├── notifications/ # 알림 시스템
├── output/        # 출력 포매터
├── pricing/       # 가격/예산 관리
├── sandbox/       # Docker 샌드박스
├── secrets/       # 비밀 관리
├── state/         # 상태 관리
├── templates/     # 템플릿 시스템
├── utils/         # 유틸리티
├── watch/         # 파일 감시
└── workflow/      # 워크플로우 엔진
```

## Development Workflow

### 새 모듈 추가 시
1. `src/{module}/mod.rs` 생성
2. `src/main.rs`에 모듈 선언 추가
3. 필요시 `Cargo.toml`에 의존성 추가

### 새 CLI 명령 추가 시
1. `src/cli/commands/{command}.rs` 생성
2. `src/cli/commands/mod.rs`에 모듈 추가
3. `src/cli/mod.rs`에 명령 추가

### 테스트 작성
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // Arrange
        let input = "test";

        // Act
        let result = function_under_test(input);

        // Assert
        assert!(result.is_ok());
    }
}
```

## Commit Convention

하나의 기능 = 하나의 커밋
Conventional Commits 형식:
- `feat:` 새 기능
- `fix:` 버그 수정
- `test:` 테스트 추가/수정
- `refactor:` 리팩토링
- `docs:` 문서
- `chore:` 빌드, 설정

## Verification Checklist

작업 완료 전 반드시 확인:

```bash
# 1. 빌드 확인
cargo build

# 2. 테스트 확인
cargo test

# 3. (선택) 클리피 확인
cargo clippy
```

## Error Handling Pattern

```rust
use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub fn my_function() -> Result<()> {
    some_operation()
        .context("Failed to perform operation")?;
    Ok(())
}
```

## Builder Pattern

```rust
pub struct Config {
    pub name: String,
    pub value: i32,
}

impl Config {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: 0,
        }
    }

    pub fn with_value(mut self, value: i32) -> Self {
        self.value = value;
        self
    }
}
```

## 금지 사항

- ❌ `unwrap()` 사용 (테스트 제외)
- ❌ `panic!()` 사용 (명백한 프로그래밍 오류 제외)
- ❌ 하드코딩된 경로
- ❌ 테스트 없는 기능 추가
- ❌ cargo build/test 실패 상태로 종료
