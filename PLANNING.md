# Doodoori - Claude Code 자동화 CLI

> Claude Code를 내부적으로 사용하여 목표 완수까지 자율적으로 작업하는 CLI 앱

## 1. 프로젝트 개요

### 1.1 핵심 컨셉
- **자율적 목표 완수**: 사용자가 목표를 설정하면 완료될 때까지 사용자 개입 없이 자동 수행
- **마크다운 업무지시서**: 구조화된 업무 지시를 마크다운 문서로 정의
- **샌드박스 모드**: Docker 컨테이너 내에서 안전하게 실행
- **병렬 실행**: 여러 목표를 동시에 병렬 처리
- **자기 개선 루프**: Ralph Wiggum 메커니즘을 Rust로 직접 구현하여 완료까지 반복

### 1.2 이름의 의미
"Doodoori" (두두리, 豆豆里) - 신라 설화에 등장하는 대장장이 신격

- **단야신(鍛冶神)의 전형**: 방망이나 망치를 두드리는 소리에서 유래된 이름으로, 쇠를 두드려 물건을 만드는 대장장이를 신격화한 존재
- **기술자의 수호신**: 신라 경주 부근에서 숭배되었으며, 목재나 금속을 다루는 기술자들의 수호신 역할
- **프로젝트와의 연관**: 코드를 두드려(작성하여) 소프트웨어를 만들어내는 개발자/AI의 수호신이라는 의미

---

## 2. 기능 아이디에이션

### 2.1 핵심 기능

#### A. 프롬프트 실행 모드

```bash
# 1. 애드혹 프롬프트 (일회성)
doodoori run "REST API for todo app 구현"

# 2. 마크다운 업무지시서 실행
doodoori run --spec ./specs/todo-api.md

# 3. 업무지시서 생성
doodoori spec "REST API for todo app" -o ./specs/todo-api.md

# 4. 대화형 업무지시서 생성
doodoori spec --interactive -o ./specs/todo-api.md
```

#### B. 샌드박스 모드 (Docker)

```bash
# Docker 샌드박스에서 실행
doodoori run --sandbox "위험할 수 있는 작업"

# 커스텀 Docker 이미지 사용
doodoori run --sandbox --image my-dev-env:latest "작업"

# 네트워크 격리 (완전 오프라인)
doodoori run --sandbox --network none "작업"
```

**샌드박스 기능:**
- 호스트의 Claude Code 인증정보 자동 마운트 (`~/.claude/`)
- 환경변수 전달 (`ANTHROPIC_API_KEY` 등)
- 작업 디렉토리 볼륨 마운트
- 선택적 네트워크 격리

#### C. 병렬 실행 모드

```bash
# 1. 애드혹 병렬 실행 (각 task에 모델 지정 가능)
doodoori parallel \
  --task "API 구현" \
  --task "프론트엔드 구현:haiku" \
  --task "테스트 작성:opus" \
  --workers 3

# 2. 업무지시서 배치 실행 (각 스펙 파일의 모델 설정 사용)
doodoori parallel --specs ./specs/*.md --workers 5

# 3. 단일 스펙의 Tasks 섹션 자동 분리 실행
doodoori parallel --spec ./specs/fullstack.md
# → fullstack.md 내의 Tasks 섹션을 파싱하여 자동 병렬화

# 4. 전역 모델 오버라이드
doodoori parallel --spec ./specs/fullstack.md --model haiku
# → 모든 task를 haiku로 강제 (비용 절감 모드)

# 5. 병렬 + 샌드박스
doodoori parallel --sandbox --specs ./specs/*.md
```

**Task 분리 동작:**
```
┌─────────────────────────────────────────────────────────────────┐
│  doodoori parallel --spec ./specs/fullstack.md                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. Spec 파일 파싱                                              │
│          │                                                      │
│          ▼                                                      │
│  2. Tasks 섹션 존재?                                            │
│     ├─ Yes → 명시된 Task들로 분리                               │
│     │        (각 Task의 model, depends_on 등 사용)              │
│     │                                                           │
│     └─ No → 단일 작업으로 실행 (병렬화 없음)                    │
│                                                                 │
│  3. 의존성 그래프 구성 (DAG)                                    │
│          │                                                      │
│          ▼                                                      │
│  4. parallel_group 또는 depends_on 기반 스케줄링               │
│     - Group 0: 의존성 없는 작업들 (병렬)                        │
│     - Group 1: Group 0 완료 후 실행 가능한 작업들               │
│     - ...                                                       │
│                                                                 │
│  5. 각 Worker에 Task 할당                                       │
│     - Worker 1: backend-api (sonnet)                           │
│     - Worker 2: frontend-ui (sonnet)                           │
│     - Worker 3: 대기 → api-integration (haiku)                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**애드혹 task의 모델 지정 형식:**
```bash
# 형식: "task 설명:모델"
--task "API 구현:sonnet"
--task "문서화:haiku"
--task "아키텍처 설계:opus"
--task "간단한 수정"           # 모델 미지정 → 자동 선택
```

#### D. 권한 제어

```bash
# 1. 안전 모드 (기본값) - 제한된 도구만 허용
doodoori run "작업"

# 2. 확장 모드 - 추가 도구 허용
doodoori run --allow "Write,Edit,Bash(npm *)" "작업"

# 3. YOLO 모드 - 모든 권한 부여 (위험!)
doodoori run --yolo "작업"  # --dangerously-skip-permissions 전달

# 4. 읽기 전용 모드
doodoori run --readonly "코드 분석"
```

#### E. 모델 및 예산 관리

```bash
# 모델 지정 (alias 사용)
doodoori run --model haiku "간단한 작업"      # 저렴, 빠름
doodoori run --model sonnet "일반 작업"       # 기본값, 균형
doodoori run --model opus "복잡한 작업"       # 고성능, 고비용

# 예산 제한 설정
doodoori run --budget 5.00 "작업"             # 최대 $5까지만 사용
doodoori run --budget 10.00 --model opus "작업"

# 비용 추적 조회
doodoori cost                                  # 현재 세션 비용
doodoori cost --history                        # 전체 이력
doodoori cost --task-id <id>                   # 특정 작업 비용
```

**예산 초과 시 동작:**
- 예산의 80% 도달 시 경고 출력
- 예산 초과 시 즉시 작업 중단
- 중단 시점의 상태를 저장하여 재개 가능

#### F. Resume(재개) 기능

```bash
# 실패한 작업 재개
doodoori resume <task-id>

# 특정 단계부터 재개
doodoori resume <task-id> --from-step 3

# 워크플로우 재개 (실패 지점부터)
doodoori workflow resume <workflow-id>

# 상태 확인
doodoori status <task-id>
```

**상태 파일 구조 (`.doodoori/state.json`):**
```json
{
  "task_id": "uuid-1234",
  "workflow_id": "workflow-5678",
  "started_at": "2025-01-17T10:00:00Z",
  "updated_at": "2025-01-17T10:30:00Z",
  "status": "failed",
  "current_step": 2,
  "steps": [
    {
      "name": "Task-A",
      "status": "completed",
      "started_at": "2025-01-17T10:00:00Z",
      "completed_at": "2025-01-17T10:15:00Z",
      "cost_usd": 0.45,
      "tokens": { "input": 15000, "output": 3000 },
      "files_changed": ["src/api.ts", "src/types.ts"]
    },
    {
      "name": "Task-B",
      "status": "failed",
      "started_at": "2025-01-17T10:15:00Z",
      "error": "Budget exceeded",
      "cost_usd": 1.20,
      "tokens": { "input": 40000, "output": 8000 }
    },
    {
      "name": "Task-C",
      "status": "pending"
    }
  ],
  "total_cost_usd": 1.65,
  "budget_usd": 2.00
}
```

**재개 동작:**
- `completed` 단계는 건너뛰고 `failed` 또는 `pending`부터 시작
- 이전 단계의 파일 변경사항은 그대로 유지
- Claude 세션 ID를 저장하여 컨텍스트 유지 가능

#### G. Secret 관리

```bash
# .env 파일에서 시크릿 로드
doodoori run --env-file .env "작업"

# 특정 환경변수만 전달
doodoori run --env API_KEY --env DB_URL "작업"

# 시스템 키체인에서 로드 (macOS Keychain, Linux Secret Service)
doodoori run --keychain "작업"

# 시크릿 저장 (키체인)
doodoori secret set ANTHROPIC_API_KEY
doodoori secret set --from-env ANTHROPIC_API_KEY

# 시크릿 목록 확인
doodoori secret list
```

**시크릿 우선순위:**
1. CLI 인자 (`--env`)
2. `.env` 파일 (`--env-file`)
3. 시스템 키체인 (`--keychain`)
4. 환경변수 (자동 탐지)

**보안 원칙:**
- 시크릿은 절대 로그에 기록하지 않음
- 샌드박스 모드에서 시크릿 마스킹 옵션

#### H. Dry Run 모드

```bash
# 실제 실행 없이 미리보기
doodoori run --dry-run "작업"

# 샌드박스 + dry run
doodoori run --sandbox --dry-run "작업"

# 워크플로우 dry run
doodoori workflow run --dry-run ./workflow.yaml

# 상세 출력
doodoori run --dry-run --verbose "작업"
```

**Dry Run 출력 정보:**
```
=== Dry Run Preview ===

[Prompt]
  "REST API for todo app 구현"

[Model]
  sonnet (claude-sonnet-4-20250514)

[Estimated Cost]
  Input: ~2,000 tokens × $3.00/MTok = ~$0.006
  Output: ~5,000 tokens × $15.00/MTok = ~$0.075
  Total: ~$0.08 (budget: $5.00)

[Permissions]
  Allowed: Read, Write, Edit, Grep, Glob
  Denied: Bash(rm *), Bash(curl *)

[Execution Mode]
  Direct (no sandbox)

[Mounts] (if sandbox)
  /workspace ← ./current-dir (rw)
  ~/.claude → /home/doodoori/.claude (ro)

[Environment Variables]
  ANTHROPIC_API_KEY: ***masked***
  NODE_ENV: development

[Ralph Wiggum]
  Enabled: true
  Max iterations: 50
  Completion promise: "COMPLETE"

=== End Preview ===
```

### 2.2 업무지시서 (Spec) 구조

#### 기본 구조 (단일 작업)

```markdown
# Task: REST API for Todo App

## Objective
CRUD 기능을 갖춘 Todo REST API 구현

## Model
sonnet  # 선택사항: haiku, sonnet, opus (미지정 시 자동 선택)

## Requirements
- [ ] GET /todos - 전체 목록 조회
- [ ] POST /todos - 새 항목 생성
- [ ] PUT /todos/:id - 항목 수정
- [ ] DELETE /todos/:id - 항목 삭제

## Constraints
- Node.js + Express 사용
- TypeScript 필수
- 테스트 커버리지 80% 이상

## Completion Criteria
모든 테스트 통과 시 완료

## Max Iterations
50

## Completion Promise
<promise>TASK_COMPLETE</promise>
```

#### 병렬 작업용 구조 (Tasks 섹션 포함)

```markdown
# Spec: Full Stack Todo Application

## Objective
프론트엔드와 백엔드를 갖춘 Todo 애플리케이션 구현

## Global Settings
default_model: sonnet           # 전역 기본 모델
max_parallel_workers: 3         # 최대 병렬 워커 수
completion_promise: "COMPLETE"

## Tasks
<!--
  각 Task는 독립적으로 병렬 실행 가능
  depends_on이 있으면 해당 작업 완료 후 실행
-->

### Task: backend-api
- **model**: sonnet              # 이 작업에 사용할 모델
- **priority**: 1                # 우선순위 (낮을수록 먼저)
- **depends_on**: []             # 의존성 없음 (즉시 시작 가능)
- **description**: REST API 백엔드 구현
- **requirements**:
  - Express + TypeScript 설정
  - CRUD 엔드포인트 구현
  - 입력 검증 미들웨어
  - 에러 핸들링
- **completion_criteria**: 모든 API 테스트 통과
- **max_iterations**: 30

### Task: frontend-ui
- **model**: sonnet
- **priority**: 1
- **depends_on**: []             # backend와 병렬 실행 가능
- **description**: React 프론트엔드 구현
- **requirements**:
  - Vite + React + TypeScript 설정
  - Todo 목록 컴포넌트
  - 추가/수정/삭제 기능
  - 반응형 디자인
- **completion_criteria**: UI 컴포넌트 렌더링 정상
- **max_iterations**: 30

### Task: api-integration
- **model**: haiku               # 간단한 통합 작업이므로 haiku
- **priority**: 2
- **depends_on**: [backend-api, frontend-ui]  # 둘 다 완료 후 실행
- **description**: 프론트엔드-백엔드 연동
- **requirements**:
  - API 클라이언트 설정
  - 에러 상태 처리
  - 로딩 상태 UI
- **completion_criteria**: E2E 흐름 정상 동작
- **max_iterations**: 20

### Task: testing
- **model**: opus                # 복잡한 테스트 설계는 opus
- **priority**: 3
- **depends_on**: [api-integration]
- **description**: 통합 테스트 작성
- **requirements**:
  - Jest + Testing Library 설정
  - 단위 테스트 (커버리지 80%)
  - E2E 테스트 (주요 흐름)
- **completion_criteria**: 모든 테스트 통과
- **max_iterations**: 40

## Constraints
- 모든 코드는 TypeScript strict 모드
- ESLint + Prettier 적용
- 민감 정보 하드코딩 금지

## Budget
max_total_usd: 15.00            # 전체 예산 상한
```

#### 모델 자동 선택 로직

Task에 모델이 지정되지 않으면 Doodoori가 작업 특성을 분석하여 자동 선택:

```rust
// claude/models.rs

pub fn auto_select_model(task: &TaskSpec) -> ModelAlias {
    let complexity_score = calculate_complexity(task);

    match complexity_score {
        0..=30 => ModelAlias::Haiku,    // 단순 작업
        31..=70 => ModelAlias::Sonnet,  // 일반 작업
        71..=100 => ModelAlias::Opus,   // 복잡한 작업
    }
}

fn calculate_complexity(task: &TaskSpec) -> u8 {
    let mut score: u8 = 50;  // 기본값: sonnet

    // 요구사항 수에 따른 가중치
    score += (task.requirements.len() as u8).saturating_mul(5);

    // 키워드 기반 조정
    let desc = task.description.to_lowercase();

    // 단순 작업 키워드 → haiku
    if desc.contains("simple") || desc.contains("basic")
        || desc.contains("설정") || desc.contains("config") {
        score = score.saturating_sub(25);
    }

    // 복잡한 작업 키워드 → opus
    if desc.contains("architect") || desc.contains("design")
        || desc.contains("복잡") || desc.contains("최적화")
        || desc.contains("리팩토링") || desc.contains("테스트 설계") {
        score = score.saturating_add(25);
    }

    // 의존성 많으면 통합 작업 → sonnet 이상
    if task.depends_on.len() >= 2 {
        score = score.max(50);
    }

    score.min(100)
}
```

#### 모델 선택 가이드라인

| 작업 유형 | 권장 모델 | 이유 |
|----------|----------|------|
| 설정/보일러플레이트 | `haiku` | 단순 반복 작업, 비용 절감 |
| CRUD 구현 | `sonnet` | 균형 잡힌 성능 |
| API 통합 | `haiku`~`sonnet` | 난이도에 따라 |
| 테스트 작성 | `sonnet` | 적절한 품질 필요 |
| 아키텍처 설계 | `opus` | 복잡한 추론 필요 |
| 리팩토링 | `opus` | 전체 구조 이해 필요 |
| 버그 수정 (간단) | `haiku` | 빠른 피드백 |
| 버그 수정 (복잡) | `opus` | 깊은 분석 필요 |
| 문서화 | `haiku` | 비용 효율적 |
| 코드 리뷰 | `opus` | 품질 중요 |

### 2.3 설정 파일 구조

```toml
# doodoori.toml

[default]
model = "sonnet"           # haiku, sonnet, opus
max_iterations = 50
timeout = "30m"

[model]
# 모델 alias → 실제 모델 ID 매핑
# price.toml에서 자동으로 로드되며, 여기서 오버라이드 가능
default = "sonnet"
# 특정 작업 유형별 모델 지정
spec_generation = "haiku"  # 업무지시서 생성 시
code_review = "opus"       # 코드 리뷰 시

[budget]
# 전역 예산 설정
default_budget_usd = 10.00          # 기본 예산 (작업당)
warning_threshold_percent = 80      # 경고 임계치 (%)
hard_limit_usd = 100.00             # 절대 상한 (이 이상 불가)
track_history = true                # 비용 이력 저장
history_retention_days = 30         # 이력 보관 기간

[permissions]
# 기본 허용 도구
allowed_tools = ["Read", "Grep", "Glob", "Write", "Edit"]
# 명시적 거부
denied_tools = ["Bash(rm -rf *)", "Bash(curl *)"]
# YOLO 모드 허용 여부
allow_yolo = false

[sandbox]
enabled = false
image = "doodoori/sandbox:latest"
network = "bridge"  # bridge, none, host
mount_claude_config = true
extra_mounts = []

[secrets]
# 시크릿 소스 설정
use_keychain = true                 # 시스템 키체인 사용
env_file = ".env"                   # 기본 .env 파일 경로
auto_detect_env = true              # 환경변수 자동 탐지
mask_in_logs = true                 # 로그에서 시크릿 마스킹
# 샌드박스에 전달할 환경변수 화이트리스트
allowed_env_vars = [
    "ANTHROPIC_API_KEY",
    "NODE_ENV",
    "PATH"
]

[parallel]
max_workers = 4
isolate_workspaces = true

[loop]
# 자기 개선 루프 설정 (Ralph Wiggum 메커니즘 내부 구현)
enabled = true
completion_promise = "COMPLETE"        # 완료 신호 문자열
max_iterations = 50                    # 최대 반복 횟수
iteration_delay_ms = 1000              # 반복 간 딜레이 (ms)
context_carry_over = true              # 이전 실행 컨텍스트 유지 (--continue)

[state]
# 상태 관리 설정
state_dir = ".doodoori"             # 상태 파일 저장 위치
auto_save = true                    # 자동 저장
save_interval_secs = 30             # 저장 간격
keep_completed_tasks = 10           # 완료된 작업 보관 수

[logging]
level = "info"
output = "~/.doodoori/logs/"
```

### 2.4 개발 워크플로우 (Git 통합)

Doodoori는 모든 개발 작업에서 Git 워크플로우를 자동으로 관리합니다.

#### 기본 워크플로우

```
┌─────────────────────────────────────────────────────────────────┐
│                    Doodoori 개발 워크플로우                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. 프로젝트 인스트럭션 확인                                     │
│     └─ doodoori.md 존재? → 읽고 지시 따름                        │
│     └─ 없으면 → 기본 워크플로우 적용                             │
│                                                                 │
│  2. Git 초기화 확인                                              │
│     └─ .git 존재? → 확인 완료                                    │
│     └─ 없으면 → git init                                        │
│                                                                 │
│  3. Feature/Fix 브랜치 생성                                      │
│     └─ git checkout -b feature/<spec-name>                      │
│     └─ 또는 fix/<spec-name>                                     │
│                                                                 │
│  4. 개발 진행 (Loop Engine)                                      │
│     └─ 기능 단위로 커밋                                          │
│     └─ Conventional Commits 형식                                │
│                                                                 │
│  5. 스펙 완료 시 PR 생성                                         │
│     └─ gh pr create                                             │
│                                                                 │
│  6. 코드 리뷰 (자동)                                             │
│     └─ opus 모델로 PR diff 리뷰                                  │
│                                                                 │
│  7. 문제 없으면 main에 머지                                       │
│     └─ gh pr merge --squash                                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### doodoori.md (프로젝트 인스트럭션 파일)

`doodoori.md`는 CLAUDE.md와 동일한 역할을 하며, Claude Code에 전달되는 시스템 프롬프트입니다.

**파일 위치 우선순위:**
1. `./doodoori.md` (프로젝트 루트)
2. `./.doodoori/instructions.md`
3. 없으면 → Doodoori 기본 워크플로우 적용

**doodoori.md 예시:**

```markdown
# Project Instructions

## Overview
이 프로젝트는 Node.js + TypeScript 기반 REST API입니다.

## Tech Stack
- Runtime: Node.js 20+
- Language: TypeScript (strict mode)
- Framework: Express.js
- Database: PostgreSQL + Prisma
- Testing: Jest + Supertest

## Code Conventions

### Naming
- 파일명: kebab-case (e.g., `user-service.ts`)
- 클래스: PascalCase
- 함수/변수: camelCase
- 상수: SCREAMING_SNAKE_CASE

### Git Workflow
- Branch: `feature/<task-name>` 또는 `fix/<issue-number>`
- Commit: Conventional Commits
  - `feat:` 새 기능
  - `fix:` 버그 수정
  - `refactor:` 리팩토링
  - `docs:` 문서
  - `test:` 테스트
- PR: Squash merge to main

### Code Style
- ESLint + Prettier 적용 필수
- 함수는 단일 책임 원칙
- 에러는 try-catch로 명시적 처리
- 매직 넘버 금지, 상수로 추출

## Testing Requirements
- 단위 테스트 커버리지: 80% 이상
- 모든 API 엔드포인트 통합 테스트 필수
- 테스트 전에 린트 통과 필수

## Security Rules
- 환경변수로 시크릿 관리 (.env)
- SQL 인젝션 방지 (Prisma 사용)
- 입력 검증 필수 (zod 사용)

## PR Review Checklist
- [ ] 타입 에러 없음
- [ ] 린트 통과
- [ ] 테스트 통과
- [ ] 보안 이슈 없음
- [ ] 문서 업데이트 (필요시)
```

#### Claude Code 시스템 프롬프트 생성

Doodoori는 `doodoori.md` + 내부 워크플로우 규칙을 결합하여 시스템 프롬프트를 생성합니다.

```rust
// claude/system_prompt.rs

pub fn build_system_prompt(
    project_instructions: Option<&str>,
    task: &TaskSpec,
    config: &DoodooriConfig,
) -> String {
    let mut prompt = String::new();

    // 1. 프로젝트 인스트럭션 (doodoori.md)
    if let Some(instructions) = project_instructions {
        prompt.push_str("# Project Instructions\n\n");
        prompt.push_str(instructions);
        prompt.push_str("\n\n---\n\n");
    }

    // 2. Doodoori 기본 워크플로우 (doodoori.md 없거나 보완)
    prompt.push_str(&DEFAULT_WORKFLOW_INSTRUCTIONS);

    // 3. 현재 작업 컨텍스트
    prompt.push_str(&format!("\n\n# Current Task\n\n{}\n", task.description));

    // 4. 완료 조건
    prompt.push_str(&format!(
        "\n완료되면 반드시 `<promise>{}</promise>`를 출력하세요.\n",
        config.loop_config.completion_promise
    ));

    prompt
}

const DEFAULT_WORKFLOW_INSTRUCTIONS: &str = r#"
# Development Workflow

## Git Workflow (필수 준수)

### 1. 시작 전 확인
- `.git` 디렉토리 존재 확인
- 없으면 `git init` 실행
- 현재 브랜치 확인 (`git branch --show-current`)

### 2. 브랜치 생성
- 새 기능: `git checkout -b feature/<task-name>`
- 버그 수정: `git checkout -b fix/<task-name>`
- main 브랜치에서 직접 작업 금지

### 3. 커밋 규칙 (Conventional Commits)
- `feat: <설명>` - 새 기능 추가
- `fix: <설명>` - 버그 수정
- `refactor: <설명>` - 코드 리팩토링
- `docs: <설명>` - 문서 수정
- `test: <설명>` - 테스트 추가/수정
- `chore: <설명>` - 빌드, 설정 등

### 4. 커밋 타이밍
- 하나의 논리적 변경 단위마다 커밋
- 큰 변경은 여러 커밋으로 분리
- 커밋 메시지는 명확하고 간결하게

### 5. 작업 완료 시
- 모든 테스트 통과 확인
- 린트 에러 없음 확인
- PR 생성: `gh pr create --title "<제목>" --body "<설명>"`

### 6. 코드 리뷰
- PR diff를 검토하고 문제점 확인
- 보안 이슈, 버그 가능성, 코드 품질 체크

### 7. 머지
- 리뷰 통과 후 `gh pr merge --squash`
- feature 브랜치 삭제

## 코드 품질 규칙

### 필수 체크
- 타입 안전성 확보
- 에러 핸들링 명시적 처리
- 하드코딩된 시크릿 금지
- 매직 넘버 상수화

### 테스트
- 새 기능에는 테스트 필수
- 기존 테스트 깨지지 않게 유지
"#;
```

#### CLI 옵션

```bash
# 커스텀 인스트럭션 파일 지정
doodoori run --instructions ./custom-instructions.md "작업"

# 인스트럭션 무시 (기본 워크플로우만)
doodoori run --no-instructions "작업"

# Git 워크플로우 비활성화 (커밋/PR 생성 안함)
doodoori run --no-git "작업"

# PR 자동 머지 비활성화 (리뷰만)
doodoori run --no-auto-merge "작업"
```

#### 워크플로우 설정 (doodoori.toml)

```toml
[git]
enabled = true                      # Git 워크플로우 활성화
auto_init = true                    # .git 없으면 자동 초기화
branch_prefix_feature = "feature/"
branch_prefix_fix = "fix/"
commit_style = "conventional"       # conventional, simple
auto_commit = true                  # 기능 단위 자동 커밋
auto_pr = true                      # 완료 시 자동 PR 생성
auto_merge = false                  # PR 자동 머지 (기본 비활성화)
pr_review_model = "opus"            # PR 리뷰에 사용할 모델

[instructions]
file = "doodoori.md"                # 인스트럭션 파일명
fallback_to_claude_md = true        # doodoori.md 없으면 CLAUDE.md 사용
```

### 2.5 고급 기능

#### 워크플로우 정의

```yaml
# workflow.yaml
name: "Full Stack Development"

# 전역 설정
global:
  default_model: sonnet           # 기본 모델
  max_parallel_workers: 4
  budget_usd: 20.00               # 전체 워크플로우 예산
  completion_promise: "COMPLETE"

# 스텝 정의
steps:
  - name: "Project Setup"
    prompt: "프로젝트 초기 설정 (패키지, 린터, 타입스크립트)"
    model: haiku                  # 단순 설정 작업 → haiku
    parallel_group: 0             # 먼저 실행
    max_iterations: 10
    budget_usd: 1.00              # 스텝별 예산 제한

  - name: "Backend API"
    spec: ./specs/backend.md      # 스펙 파일 참조
    model: sonnet                 # 명시적 모델 지정
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Frontend UI"
    spec: ./specs/frontend.md
    model: sonnet
    parallel_group: 1             # Backend와 병렬 실행
    depends_on: ["Project Setup"]
    budget_usd: 5.00

  - name: "Database Schema"
    prompt: "PostgreSQL 스키마 설계 및 마이그레이션"
    model: opus                   # 설계 작업 → opus
    parallel_group: 1
    depends_on: ["Project Setup"]
    budget_usd: 3.00

  - name: "API Integration"
    prompt: "프론트엔드와 백엔드 API 연동"
    # model 미지정 → 자동 선택 (sonnet 예상)
    parallel_group: 2
    depends_on: ["Backend API", "Frontend UI", "Database Schema"]
    budget_usd: 2.00

  - name: "Testing & QA"
    spec: ./specs/testing.md
    model: opus                   # 테스트 설계는 opus
    parallel_group: 3
    depends_on: ["API Integration"]
    budget_usd: 4.00

  - name: "Documentation"
    prompt: "README 및 API 문서 작성"
    model: haiku                  # 문서화 → haiku
    parallel_group: 3             # Testing과 병렬 가능
    depends_on: ["API Integration"]
    budget_usd: 0.50
```

```bash
# 워크플로우 실행
doodoori workflow run ./workflow.yaml

# Dry run으로 실행 계획 미리보기
doodoori workflow run --dry-run ./workflow.yaml

# 출력 예시:
# === Workflow: Full Stack Development ===
#
# [Group 0] Sequential
#   └─ Project Setup (haiku, budget: $1.00)
#
# [Group 1] Parallel (3 workers)
#   ├─ Backend API (sonnet, budget: $5.00)
#   ├─ Frontend UI (sonnet, budget: $5.00)
#   └─ Database Schema (opus, budget: $3.00)
#
# [Group 2] Sequential
#   └─ API Integration (auto→sonnet, budget: $2.00)
#
# [Group 3] Parallel (2 workers)
#   ├─ Testing & QA (opus, budget: $4.00)
#   └─ Documentation (haiku, budget: $0.50)
#
# Total estimated cost: $8.50 ~ $15.00
# Total budget: $20.00
```

#### 워크플로우 재개 (Resume)

```bash
# 실패 지점부터 재개
doodoori workflow resume <workflow-id>

# 특정 스텝부터 재개
doodoori workflow resume <workflow-id> --from-step "API Integration"
```

**워크플로우 상태 파일 (`.doodoori/workflow_state.json`):**
```json
{
  "workflow_id": "wf-uuid-1234",
  "name": "Full Stack Development",
  "status": "failed",
  "current_group": 2,
  "steps": {
    "Project Setup": { "status": "completed", "cost_usd": 0.35, "model": "haiku" },
    "Backend API": { "status": "completed", "cost_usd": 2.80, "model": "sonnet" },
    "Frontend UI": { "status": "completed", "cost_usd": 3.10, "model": "sonnet" },
    "Database Schema": { "status": "completed", "cost_usd": 2.50, "model": "opus" },
    "API Integration": { "status": "failed", "error": "Budget exceeded", "model": "sonnet" },
    "Testing & QA": { "status": "pending", "model": "opus" },
    "Documentation": { "status": "pending", "model": "haiku" }
  },
  "total_cost_usd": 8.75
}
```

#### 실시간 모니터링

```bash
# 실행 상태 모니터링
doodoori status

# 특정 작업 로그 스트리밍
doodoori logs --follow <task-id>

# 대시보드 (TUI)
doodoori dashboard
```

#### 히스토리 및 재실행

```bash
# 실행 히스토리
doodoori history

# 실패한 작업 재실행
doodoori retry <task-id>

# 특정 시점부터 재개
doodoori resume <task-id> --from-iteration 25
```

### 2.5 비용 추적 및 price.toml

#### price.toml 구조

`price.toml`은 모델별 가격 정보를 저장하는 파일입니다.

**파일 위치 우선순위:**
1. `./price.toml` (프로젝트 로컬)
2. `~/.config/doodoori/price.toml` (사용자 전역)
3. 없으면 시작 시 웹에서 최신 가격을 조회하여 자동 생성

```toml
# price.toml
# Claude API 모델별 가격 정보
# 가격 단위: USD per Million Tokens (MTok)
# 출처: https://platform.claude.com/docs/en/about-claude/pricing

[meta]
version = "1.0.0"
updated_at = "2025-01-17"
source = "https://platform.claude.com/docs/en/about-claude/pricing"

# 모델 alias 정의
[aliases]
haiku = "claude-haiku-4-20250514"
sonnet = "claude-sonnet-4-20250514"
opus = "claude-opus-4-20250514"

# Claude Opus 4.5 (최신)
[models.claude-opus-4-5-20251101]
name = "Claude Opus 4.5"
input_per_mtok = 5.00
output_per_mtok = 25.00
cache_write_5m_per_mtok = 6.25
cache_write_1h_per_mtok = 10.00
cache_read_per_mtok = 0.50
max_context_tokens = 200000
deprecated = false

# Claude Opus 4
[models.claude-opus-4-20250514]
name = "Claude Opus 4"
input_per_mtok = 15.00
output_per_mtok = 75.00
cache_write_5m_per_mtok = 18.75
cache_write_1h_per_mtok = 30.00
cache_read_per_mtok = 1.50
max_context_tokens = 200000
deprecated = false

# Claude Sonnet 4.5 (최신)
[models.claude-sonnet-4-5-20251101]
name = "Claude Sonnet 4.5"
input_per_mtok = 3.00
output_per_mtok = 15.00
cache_write_5m_per_mtok = 3.75
cache_write_1h_per_mtok = 6.00
cache_read_per_mtok = 0.30
max_context_tokens = 1000000  # 1M context 지원
long_context_input_per_mtok = 6.00      # >200K input 시
long_context_output_per_mtok = 22.50    # >200K input 시
deprecated = false

# Claude Sonnet 4
[models.claude-sonnet-4-20250514]
name = "Claude Sonnet 4"
input_per_mtok = 3.00
output_per_mtok = 15.00
cache_write_5m_per_mtok = 3.75
cache_write_1h_per_mtok = 6.00
cache_read_per_mtok = 0.30
max_context_tokens = 1000000
long_context_input_per_mtok = 6.00
long_context_output_per_mtok = 22.50
deprecated = false

# Claude Haiku 4.5 (최신)
[models.claude-haiku-4-5-20251101]
name = "Claude Haiku 4.5"
input_per_mtok = 1.00
output_per_mtok = 5.00
cache_write_5m_per_mtok = 1.25
cache_write_1h_per_mtok = 2.00
cache_read_per_mtok = 0.10
max_context_tokens = 200000
deprecated = false

# Claude Haiku 4
[models.claude-haiku-4-20250514]
name = "Claude Haiku 4"
input_per_mtok = 1.00
output_per_mtok = 5.00
cache_write_5m_per_mtok = 1.25
cache_write_1h_per_mtok = 2.00
cache_read_per_mtok = 0.10
max_context_tokens = 200000
deprecated = false

# Claude Haiku 3.5 (Legacy)
[models.claude-3-5-haiku-20241022]
name = "Claude Haiku 3.5"
input_per_mtok = 0.80
output_per_mtok = 4.00
cache_write_5m_per_mtok = 1.00
cache_write_1h_per_mtok = 1.60
cache_read_per_mtok = 0.08
max_context_tokens = 200000
deprecated = false

# Batch API 가격 (50% 할인)
[batch]
discount_percent = 50

# 추가 비용
[extras]
web_search_per_1000 = 10.00  # 웹 검색 1000회당
```

#### 비용 계산 로직

```rust
// 비용 계산 예시
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
}

pub fn calculate_cost(model: &str, usage: &TokenUsage, prices: &PriceConfig) -> f64 {
    let model_price = prices.models.get(model).unwrap();

    let input_cost = (usage.input_tokens as f64 / 1_000_000.0)
        * model_price.input_per_mtok;
    let output_cost = (usage.output_tokens as f64 / 1_000_000.0)
        * model_price.output_per_mtok;
    let cache_write_cost = (usage.cache_write_tokens as f64 / 1_000_000.0)
        * model_price.cache_write_5m_per_mtok;
    let cache_read_cost = (usage.cache_read_tokens as f64 / 1_000_000.0)
        * model_price.cache_read_per_mtok;

    input_cost + output_cost + cache_write_cost + cache_read_cost
}
```

#### 비용 이력 저장 구조

```json
// ~/.doodoori/cost_history.json
{
  "total_spent_usd": 45.67,
  "period_start": "2025-01-01T00:00:00Z",
  "entries": [
    {
      "task_id": "uuid-1234",
      "timestamp": "2025-01-17T10:30:00Z",
      "model": "sonnet",
      "model_id": "claude-sonnet-4-20250514",
      "tokens": {
        "input": 15000,
        "output": 3500,
        "cache_write": 0,
        "cache_read": 2000
      },
      "cost_usd": 0.098,
      "task_name": "REST API 구현",
      "status": "completed"
    }
  ],
  "daily_summary": {
    "2025-01-17": { "cost_usd": 2.45, "tasks": 12 },
    "2025-01-16": { "cost_usd": 1.89, "tasks": 8 }
  }
}
```

#### 가격 자동 업데이트

```bash
# 가격 정보 수동 업데이트
doodoori price update

# 현재 가격 확인
doodoori price show

# 특정 모델 가격 확인
doodoori price show --model opus
```

**자동 업데이트 동작:**
1. `price.toml`이 없으면 시작 시 웹에서 조회
2. 파일이 30일 이상 오래되면 업데이트 제안
3. `--offline` 모드에서는 기존 파일 사용

---

## 3. 기술 스택 및 아키텍처

### 3.1 핵심 의존성

| 크레이트 | 버전 | 용도 | 문서 |
|---------|------|------|------|
| `clap` | 4.x | CLI 인자 파싱 | [docs.rs/clap](https://docs.rs/clap/latest/clap/) |
| `tokio` | 1.48+ | 비동기 런타임 | [tokio.rs](https://tokio.rs/) |
| `bollard` | 0.19+ | Docker API | [docs.rs/bollard](https://docs.rs/bollard/latest/bollard/) |
| `serde` | 1.x | 직렬화/역직렬화 | [serde.rs](https://serde.rs/) |
| `serde_json` | 1.x | JSON 처리 | [docs.rs/serde_json](https://docs.rs/serde_json/latest/serde_json/) |
| `toml` | 0.8+ | 설정 파일 파싱 | [docs.rs/toml](https://docs.rs/toml/latest/toml/) |
| `tracing` | 0.1+ | 로깅/추적 | [docs.rs/tracing](https://docs.rs/tracing/latest/tracing/) |
| `anyhow` | 1.x | 에러 처리 | [docs.rs/anyhow](https://docs.rs/anyhow/latest/anyhow/) |
| `thiserror` | 1.x | 커스텀 에러 타입 | [docs.rs/thiserror](https://docs.rs/thiserror/latest/thiserror/) |
| `indicatif` | 0.17+ | 프로그레스 바 | [docs.rs/indicatif](https://docs.rs/indicatif/latest/indicatif/) |
| `ratatui` | 0.28+ | TUI 대시보드 | [ratatui.rs](https://ratatui.rs/) |
| `pulldown-cmark` | 0.12+ | 마크다운 파싱 | [docs.rs/pulldown-cmark](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/) |
| `handlebars` | 6.x | 템플릿 엔진 | [docs.rs/handlebars](https://docs.rs/handlebars/latest/handlebars/) |
| `reqwest` | 0.12+ | HTTP 클라이언트 (가격 업데이트) | [docs.rs/reqwest](https://docs.rs/reqwest/latest/reqwest/) |
| `chrono` | 0.4+ | 날짜/시간 처리 | [docs.rs/chrono](https://docs.rs/chrono/latest/chrono/) |
| `uuid` | 1.x | 작업 ID 생성 | [docs.rs/uuid](https://docs.rs/uuid/latest/uuid/) |
| `dotenvy` | 0.15+ | .env 파일 로드 | [docs.rs/dotenvy](https://docs.rs/dotenvy/latest/dotenvy/) |
| `keyring` | 3.x | 시스템 키체인 연동 | [docs.rs/keyring](https://docs.rs/keyring/latest/keyring/) |
| `directories` | 5.x | 플랫폼별 디렉토리 경로 | [docs.rs/directories](https://docs.rs/directories/latest/directories/) |

### 3.2 아키텍처 다이어그램

```
┌─────────────────────────────────────────────────────────────────┐
│                        Doodoori CLI                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │   Parser    │  │   Config    │  │  Spec Mgr   │             │
│  │   (clap)    │  │   (toml)    │  │ (markdown)  │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         └────────────────┼────────────────┘                     │
│                          ▼                                      │
│  ┌─────────────────────────────────────────────────────────────┤
│  │                    Task Orchestrator                        │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  │ Single Task  │  │  Parallel    │  │  Workflow    │      │
│  │  │   Runner     │  │   Executor   │  │   Engine     │      │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│  └─────────┼─────────────────┼─────────────────┼───────────────┤
│            └─────────────────┼─────────────────┘               │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────────┤
│  │                  Execution Layer                            │
│  │  ┌──────────────────────┐  ┌──────────────────────┐        │
│  │  │    Direct Runner     │  │   Sandbox Runner     │        │
│  │  │  (local claude CLI)  │  │     (Docker)         │        │
│  │  └──────────┬───────────┘  └──────────┬───────────┘        │
│  └─────────────┼─────────────────────────┼─────────────────────┤
│                └─────────────┬───────────┘                     │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────────┤
│  │              Claude Code CLI Wrapper                        │
│  │  - Permission management (--allowedTools, --yolo)           │
│  │  - Output parsing (stream-json)                             │
│  │  - Session management                                        │
│  └─────────────────────────────────────────────────────────────┤
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────────┤
│  │                    Loop Engine                              │
│  │  - 자기 개선 루프 (Ralph Wiggum 메커니즘 내부 구현)           │
│  │  - Completion promise 검사                                   │
│  │  - 반복 제어 및 예산 체크                                    │
│  │  - Session context 유지 (--continue)                        │
│  └─────────────────────────────────────────────────────────────┤
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Claude Code CLI                             │
│  claude -p "prompt" --output-format stream-json                │
│         --allowedTools "..." [--dangerously-skip-permissions]  │
│         [--continue <session-id>]                               │
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 디렉토리 구조

```
doodoori/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── PLANNING.md
├── doodoori.toml.example
├── price.toml                  # 모델별 가격 정보
│
├── src/
│   ├── main.rs                 # 진입점
│   ├── lib.rs                  # 라이브러리 루트
│   │
│   ├── cli/                    # CLI 인터페이스
│   │   ├── mod.rs
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── run.rs          # doodoori run
│   │   │   ├── spec.rs         # doodoori spec
│   │   │   ├── parallel.rs     # doodoori parallel
│   │   │   ├── workflow.rs     # doodoori workflow
│   │   │   ├── status.rs       # doodoori status
│   │   │   ├── resume.rs       # doodoori resume
│   │   │   ├── cost.rs         # doodoori cost
│   │   │   ├── price.rs        # doodoori price
│   │   │   ├── secret.rs       # doodoori secret
│   │   │   └── config.rs       # doodoori config
│   │   └── args.rs             # CLI 인자 정의
│   │
│   ├── config/                 # 설정 관리
│   │   ├── mod.rs
│   │   ├── settings.rs         # doodoori.toml 파싱
│   │   └── permissions.rs      # 권한 설정
│   │
│   ├── pricing/                # 가격 및 예산 관리
│   │   ├── mod.rs
│   │   ├── price_config.rs     # price.toml 파싱
│   │   ├── calculator.rs       # 비용 계산
│   │   ├── budget.rs           # 예산 관리
│   │   ├── history.rs          # 비용 이력
│   │   └── updater.rs          # 가격 자동 업데이트
│   │
│   ├── secrets/                # 시크릿 관리
│   │   ├── mod.rs
│   │   ├── env_file.rs         # .env 파일 로드
│   │   ├── keychain.rs         # 시스템 키체인 연동
│   │   └── masking.rs          # 로그 마스킹
│   │
│   ├── state/                  # 상태 관리 (Resume 기능)
│   │   ├── mod.rs
│   │   ├── task_state.rs       # 작업 상태
│   │   ├── workflow_state.rs   # 워크플로우 상태
│   │   ├── persistence.rs      # 상태 저장/로드
│   │   └── recovery.rs         # 재개 로직
│   │
│   ├── spec/                   # 업무지시서 처리
│   │   ├── mod.rs
│   │   ├── parser.rs           # 마크다운 파싱
│   │   ├── generator.rs        # 업무지시서 생성
│   │   └── templates.rs        # 템플릿
│   │
│   ├── executor/               # 실행 엔진
│   │   ├── mod.rs
│   │   ├── direct.rs           # 직접 실행
│   │   ├── sandbox.rs          # Docker 샌드박스
│   │   ├── parallel.rs         # 병렬 실행
│   │   └── dry_run.rs          # Dry Run 모드
│   │
│   ├── claude/                 # Claude Code 통합
│   │   ├── mod.rs
│   │   ├── cli.rs              # CLI 래퍼
│   │   ├── output.rs           # 출력 파싱
│   │   ├── session.rs          # 세션 관리
│   │   ├── models.rs           # 모델 alias 관리
│   │   └── system_prompt.rs    # 시스템 프롬프트 생성
│   │
│   ├── instructions/           # 프로젝트 인스트럭션 (doodoori.md)
│   │   ├── mod.rs
│   │   ├── loader.rs           # doodoori.md 로드
│   │   ├── parser.rs           # 인스트럭션 파싱
│   │   └── defaults.rs         # 기본 워크플로우 인스트럭션
│   │
│   ├── git/                    # Git 워크플로우
│   │   ├── mod.rs
│   │   ├── repo.rs             # 저장소 초기화/확인
│   │   ├── branch.rs           # 브랜치 관리
│   │   ├── commit.rs           # 커밋 (Conventional Commits)
│   │   ├── pr.rs               # PR 생성 (gh CLI)
│   │   ├── review.rs           # 코드 리뷰 (Claude)
│   │   └── merge.rs            # 머지 처리
│   │
│   ├── loop_engine/            # 자기 개선 루프 (Ralph Wiggum 메커니즘)
│   │   ├── mod.rs
│   │   ├── controller.rs       # 루프 제어 로직
│   │   ├── completion.rs       # 완료 조건 검사
│   │   └── context.rs          # 컨텍스트 관리
│   │
│   ├── docker/                 # Docker 통합
│   │   ├── mod.rs
│   │   ├── container.rs        # 컨테이너 관리
│   │   ├── image.rs            # 이미지 관리
│   │   └── volume.rs           # 볼륨 마운트
│   │
│   ├── workflow/               # 워크플로우 엔진
│   │   ├── mod.rs
│   │   ├── parser.rs           # YAML 파싱
│   │   ├── scheduler.rs        # 작업 스케줄링
│   │   └── dag.rs              # 의존성 그래프
│   │
│   ├── monitoring/             # 모니터링
│   │   ├── mod.rs
│   │   ├── logger.rs           # 로깅
│   │   ├── progress.rs         # 진행 상태
│   │   └── dashboard.rs        # TUI 대시보드
│   │
│   └── utils/                  # 유틸리티
│       ├── mod.rs
│       └── error.rs            # 에러 타입
│
├── tests/                      # 통합 테스트
│   ├── integration/
│   └── fixtures/
│
└── docker/                     # Docker 관련
    ├── Dockerfile.sandbox      # 샌드박스 이미지
    └── docker-compose.yml
```

---

## 4. 개발 계획

### Phase 1: 기반 구축 (MVP)

**목표**: 기본적인 Claude Code 래퍼 + 단일 작업 실행

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| 프로젝트 초기화 | Cargo.toml, 기본 구조 설정 | P0 |
| CLI 스켈레톤 | clap 기반 명령어 구조 | P0 |
| Claude CLI 래퍼 | 기본 실행 및 출력 캡처 | P0 |
| 권한 관리 | allowedTools, YOLO 모드 | P0 |
| **Loop Engine** | 자기 개선 루프 Rust 구현 | P0 |
| 설정 파일 | doodoori.toml 파싱 | P1 |
| **모델 관리** | alias(haiku/sonnet/opus) 지원 | P0 |
| **price.toml 로드** | 가격 정보 파싱 및 자동 생성 | P1 |
| **예산 관리 기초** | --budget 옵션, 비용 계산 | P1 |
| **Dry Run 모드** | --dry-run 미리보기 | P1 |

**산출물**: `doodoori run "prompt"`, `doodoori run --dry-run`, `doodoori run --model haiku`

### Phase 2: 업무지시서 시스템

**목표**: 마크다운 기반 업무지시서 지원

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| 마크다운 파서 | 업무지시서 구조 파싱 | P0 |
| 업무지시서 생성기 | 프롬프트 → 마크다운 변환 | P1 |
| 템플릿 시스템 | 재사용 가능한 템플릿 | P1 |
| 업무지시서 검증 | 스키마 검증 | P2 |

**산출물**: `doodoori run --spec ./spec.md`, `doodoori spec "prompt"`

### Phase 3: 샌드박스 모드

**목표**: Docker 기반 안전 실행 환경

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| Bollard 통합 | Docker API 연결 | P0 |
| 컨테이너 생성 | 샌드박스 컨테이너 관리 | P0 |
| 볼륨 마운트 | 작업 디렉토리, 설정 마운트 | P0 |
| 인증정보 전달 | ~/.claude, 환경변수 | P0 |
| 네트워크 격리 | 선택적 네트워크 모드 | P1 |
| 샌드박스 이미지 | Dockerfile 작성 | P1 |

**산출물**: `doodoori run --sandbox "prompt"`

### Phase 4: 상태 관리 및 시크릿

**목표**: Resume 기능 + Secret 관리

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| 상태 파일 구조 | `.doodoori/state.json` 설계 | P0 |
| 작업 상태 저장 | 단계별 진행 상태 기록 | P0 |
| Resume 명령 | `doodoori resume <task-id>` | P0 |
| 비용 이력 저장 | `cost_history.json` 관리 | P1 |
| .env 파일 로드 | `dotenvy` 통합 | P0 |
| 키체인 연동 | `keyring` 크레이트 통합 | P1 |
| 시크릿 마스킹 | 로그에서 시크릿 필터링 | P1 |
| 가격 자동 업데이트 | `doodoori price update` | P2 |

**산출물**: `doodoori resume`, `doodoori secret`, `doodoori cost`

### Phase 5: 병렬 실행

**목표**: 다중 작업 동시 실행

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| Task Pool | Tokio 기반 작업 풀 | P0 |
| 워커 관리 | 동시 실행 수 제어 | P0 |
| 작업 격리 | 작업별 독립 워크스페이스 | P1 |
| 진행 상태 추적 | 개별 작업 상태 관리 | P1 |
| 결과 집계 | 전체 결과 보고 | P1 |

**산출물**: `doodoori parallel --task "A" --task "B"`

### Phase 6: 워크플로우 및 모니터링

**목표**: 복잡한 워크플로우 지원 + 실시간 모니터링

| 작업 | 설명 | 우선순위 |
|------|------|---------|
| 워크플로우 파서 | YAML 워크플로우 정의 | P1 |
| DAG 스케줄러 | 의존성 기반 실행 순서 | P1 |
| TUI 대시보드 | ratatui 기반 모니터링 | P2 |
| 로그 스트리밍 | 실시간 로그 출력 | P1 |
| 히스토리 관리 | 실행 이력 저장/조회 | P2 |

**산출물**: `doodoori workflow run`, `doodoori dashboard`

---

## 5. Claude Code CLI 통합 상세

### 5.1 기본 실행 명령

```bash
# Doodoori가 내부적으로 실행하는 Claude Code 명령
claude -p "<prompt>" \
  --output-format stream-json \
  --plugin-dir /path/to/ralph-wiggum \
  --allowedTools "Read,Write,Edit,Bash(npm *),Bash(git *)" \
  --max-turns 100
```

### 5.2 자기 개선 루프 (Loop Engine)

Ralph Wiggum 플러그인의 핵심 메커니즘을 Rust로 직접 구현합니다.

#### 핵심 메커니즘 분석

```
┌─────────────────────────────────────────────────────────────┐
│                    Loop Engine 동작 흐름                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 초기 프롬프트로 Claude 실행                              │
│          │                                                  │
│          ▼                                                  │
│  2. stream-json 출력 파싱                                   │
│          │                                                  │
│          ▼                                                  │
│  3. completion_promise 검사 ──────── 발견 ──→ 완료 (종료)    │
│          │                                                  │
│          │ 미발견                                           │
│          ▼                                                  │
│  4. max_iterations 검사 ──────── 초과 ──→ 타임아웃 (종료)    │
│          │                                                  │
│          │ 미초과                                           │
│          ▼                                                  │
│  5. 컨텍스트 유지하여 재실행 (--continue)                    │
│          │                                                  │
│          └──────────────→ 2번으로 돌아감                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### Rust 구현 설계

```rust
// loop_engine/controller.rs

pub struct LoopConfig {
    pub completion_promise: String,
    pub max_iterations: u32,
    pub iteration_delay: Duration,
    pub context_carry_over: bool,
}

pub struct LoopEngine {
    config: LoopConfig,
    claude_cli: ClaudeCli,
    current_iteration: u32,
    session_id: Option<String>,
}

impl LoopEngine {
    pub async fn run(&mut self, prompt: &str) -> Result<LoopResult> {
        let mut last_output = String::new();

        loop {
            self.current_iteration += 1;

            // 예산 체크
            if self.budget_exceeded() {
                return Ok(LoopResult::BudgetExceeded {
                    iteration: self.current_iteration,
                    cost: self.total_cost,
                });
            }

            // 최대 반복 체크
            if self.current_iteration > self.config.max_iterations {
                return Ok(LoopResult::MaxIterationsReached {
                    iteration: self.current_iteration,
                });
            }

            // Claude 실행
            let output = self.execute_claude(prompt).await?;
            last_output = output.clone();

            // 완료 조건 검사
            if self.check_completion(&output) {
                return Ok(LoopResult::Completed {
                    iteration: self.current_iteration,
                    output: last_output,
                });
            }

            // 딜레이 후 다음 반복
            tokio::time::sleep(self.config.iteration_delay).await;
        }
    }

    async fn execute_claude(&mut self, prompt: &str) -> Result<String> {
        let mut cmd = self.claude_cli.build_command();

        // 첫 실행이 아니면 --continue로 세션 유지
        if let Some(ref session_id) = self.session_id {
            cmd.arg("--continue").arg(session_id);
        }

        let output = cmd
            .arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("stream-json")
            .output()
            .await?;

        // 세션 ID 저장 (첫 실행 시)
        if self.session_id.is_none() {
            self.session_id = self.extract_session_id(&output);
        }

        // 토큰 사용량 추적
        self.track_usage(&output)?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn check_completion(&self, output: &str) -> bool {
        output.contains(&self.config.completion_promise)
    }
}

pub enum LoopResult {
    Completed { iteration: u32, output: String },
    MaxIterationsReached { iteration: u32 },
    BudgetExceeded { iteration: u32, cost: f64 },
    Error { iteration: u32, error: String },
}
```

#### 실행 예시

```bash
# 내부적으로 루프 엔진이 동작
doodoori run "REST API 구현. 완료되면 <promise>COMPLETE</promise> 출력"

# 실행 로그 예시:
# [Loop 1/50] Claude 실행 중...
# [Loop 1/50] 완료 신호 미발견, 계속 진행
# [Loop 2/50] Claude 실행 중... (세션 유지)
# [Loop 2/50] 완료 신호 미발견, 계속 진행
# ...
# [Loop 7/50] 완료 신호 발견: COMPLETE
# ✓ 작업 완료 (7회 반복, 비용: $0.45)
```

### 5.3 출력 파싱 (stream-json)

```rust
// Claude Code stream-json 출력 구조
#[derive(Deserialize)]
#[serde(tag = "type")]
enum ClaudeEvent {
    #[serde(rename = "assistant")]
    Assistant { message: AssistantMessage },
    #[serde(rename = "result")]
    Result { result: String, session_id: String },
    #[serde(rename = "error")]
    Error { error: ErrorInfo },
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { name: String, input: Value },
}
```

### 5.4 권한 모드 매핑

| Doodoori 모드 | Claude Code 플래그 |
|--------------|-------------------|
| `--readonly` | `--allowedTools "Read,Grep,Glob"` |
| (기본값) | `--allowedTools "Read,Write,Edit,Grep,Glob"` |
| `--allow "..."` | `--allowedTools "Read,Write,Edit,...추가도구..."` |
| `--yolo` | `--dangerously-skip-permissions` |

---

## 6. Docker 샌드박스 상세

### 6.1 Dockerfile

```dockerfile
# docker/Dockerfile.sandbox
FROM node:20-slim

# Claude Code CLI 설치
RUN npm install -g @anthropic-ai/claude-code

# 작업 디렉토리
WORKDIR /workspace

# 비루트 사용자
RUN useradd -m -s /bin/bash doodoori
USER doodoori

# 기본 명령
CMD ["claude"]
```

### 6.2 볼륨 마운트 전략

```rust
// Docker 컨테이너 생성 시 마운트 설정
let mounts = vec![
    // 작업 디렉토리
    Mount {
        source: work_dir.to_string(),
        target: "/workspace".to_string(),
        read_only: false,
    },
    // Claude 설정 (읽기 전용)
    Mount {
        source: home.join(".claude").to_string(),
        target: "/home/doodoori/.claude".to_string(),
        read_only: true,
    },
    // Ralph Wiggum 플러그인
    Mount {
        source: ralph_plugin_path.to_string(),
        target: "/plugins/ralph-wiggum".to_string(),
        read_only: true,
    },
];

// 환경변수 전달
let env = vec![
    format!("ANTHROPIC_API_KEY={}", api_key),
    "HOME=/home/doodoori".to_string(),
];
```

---

## 7. 참고 문서 및 리소스

### 공식 문서
- [Claude Code CLI Reference](https://code.claude.com/docs/en/cli-reference)
- [Claude Code Settings](https://code.claude.com/docs/en/settings)
- [Claude Code Headless Mode](https://code.claude.com/docs/en/headless)
- [Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)

### 자기 개선 루프 참고 자료
- [Ralph Wiggum 플러그인 (참고용)](https://github.com/anthropics/claude-code/tree/main/plugins/ralph-wiggum)
- [원본 기법 (ghuntley)](https://ghuntley.com/ralph/)
- 주의: 외부 플러그인 사용 대신 Doodoori에서 직접 Rust로 구현

### Rust 생태계
- [Clap (CLI)](https://docs.rs/clap/latest/clap/)
- [Tokio (Async)](https://tokio.rs/)
- [Bollard (Docker)](https://docs.rs/bollard/latest/bollard/)
- [Ratatui (TUI)](https://ratatui.rs/)

### 관련 프로젝트
- [Claude Flow](https://github.com/ruvnet/claude-flow) - 비대화형 자동화
- [Ralph Orchestrator](https://github.com/mikeyobrien/ralph-orchestrator)

---

## 8. 보안 고려사항

### 8.1 위험 시나리오 방어

| 위험 | 방어책 |
|------|--------|
| 호스트 파일 삭제 | 샌드박스 모드 + 제한된 마운트 |
| 인터넷 데이터 유출 | `--network none` 옵션 |
| 악성 스크립트 다운로드 | 허용 명령어 화이트리스트 |
| API 키 노출 | 읽기 전용 설정 마운트 + 로그 마스킹 |
| 무한 루프/과다 비용 | `--max-turns`, `--max-iterations` |
| 예산 초과 | `--budget` 제한 + 80% 경고 + 하드 리밋 |
| 시크릿 유출 | 시크릿 마스킹 + 화이트리스트 환경변수 |

### 8.2 권한 레벨

```
Level 0 (Readonly):    Read, Grep, Glob
Level 1 (Safe):        + Write, Edit
Level 2 (Extended):    + Bash(제한된 명령)
Level 3 (Full):        + Bash(모든 명령)
Level 4 (YOLO):        --dangerously-skip-permissions
```

### 8.3 감사 로깅

모든 실행은 상세 로그 기록:
- 실행된 명령
- 수정된 파일
- 네트워크 요청 (가능한 경우)
- 비용 추적

---

## 9. 향후 확장 계획

### v2.0 로드맵
- [ ] 웹 UI 대시보드
- [ ] 팀 협업 기능
- [ ] 클라우드 실행 지원 (AWS ECS, GCP Cloud Run)
- [ ] 커스텀 플러그인 시스템
- [ ] MCP 서버 통합

### 커뮤니티 피드백 대응
- GitHub Issues 기반 기능 요청 수집
- 사용 패턴 분석을 통한 UX 개선
