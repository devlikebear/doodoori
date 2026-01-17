# Development Instructions for Claude

이 프로젝트에서 Claude가 따라야 할 개발 규칙입니다.

## Core Principles

### 1. TDD (Test-Driven Development)
- **테스트를 먼저 작성**한 후 구현합니다.
- Red → Green → Refactor 사이클을 따릅니다.
- 테스트가 통과해야만 기능이 완성된 것으로 간주합니다.

### 2. Commit Convention
- **하나의 기능 = 하나의 커밋**
- Conventional Commits 형식 사용:
  - `feat:` 새 기능
  - `fix:` 버그 수정
  - `test:` 테스트 추가/수정
  - `refactor:` 리팩토링
  - `docs:` 문서
  - `chore:` 빌드, 설정 등
- 커밋 메시지에 Claude Code 생성 메시지 포함 금지

### 3. Epic Completion
큰 단위의 에픽(Phase) 개발이 완료되면:
1. **Minor 버전 올리기** (Cargo.toml의 version)
2. **README.md 업데이트** - 새로운 기능 문서화
3. **CHANGELOG.md 업데이트** - 변경 이력 기록

## Git Workflow

1. Feature 브랜치에서 작업 (main 직접 작업 금지)
2. 기능별 커밋
3. PR 생성 및 리뷰
4. Squash merge to main

## Code Style

- Rust 2024 edition
- `cargo fmt` 적용
- `cargo clippy` 경고 없음
- 모든 public 함수에 문서 주석

## Testing

- 단위 테스트: `#[cfg(test)]` 모듈
- 통합 테스트: `tests/` 디렉토리
- 테스트 커버리지 목표: 80%+

## Project Structure

```
src/
├── cli/           # CLI 인터페이스 (clap)
├── claude/        # Claude Code CLI 래퍼
├── config/        # 설정 관리
├── pricing/       # 가격/예산 관리
├── loop_engine/   # 자기 개선 루프
├── state/         # 상태 관리 (Resume)
├── instructions/  # doodoori.md 처리
├── git/           # Git 워크플로우
├── executor/      # 실행 엔진
└── utils/         # 유틸리티
```
