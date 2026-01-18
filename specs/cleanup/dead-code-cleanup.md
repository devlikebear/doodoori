# Dead Code Cleanup

## Objective
Remove `#![allow(dead_code)]` module-level directives and clean up truly unused code.

## Strategy

### 1. Remove Unused Helper Code
These are internal helpers that will never be used:

**src/secrets/masking.rs**
- Remove entire file (SECRET_PATTERNS, SecretMasker, MaskingLayer not used)
- Update src/secrets/mod.rs to remove masking module

### 2. Add Item-Level Allows for Public API
For code that's part of the public library API but not yet used internally, add `#[allow(dead_code)]` at the item level instead of module level.

**src/git/**: Keep all, add item-level allows
- CommitManager, PRManager, WorktreeManager are public API

**src/sandbox/**: Keep structure, add item-level allows
- SandboxRunner, ContainerManager are public API

**src/pricing/**: Keep all fields, add item-level allows
- Pricing config is public API

### 3. Files to Process

For each file, remove `#![allow(dead_code)]` and add `#[allow(dead_code)]` to specific unused items:

1. src/config/mod.rs
2. src/secrets/mod.rs (after removing masking)
3. src/secrets/env_loader.rs
4. src/output/mod.rs
5. src/state/mod.rs
6. src/hooks/mod.rs
7. src/workflow/mod.rs
8. src/watch/mod.rs
9. src/executor/mod.rs
10. src/loop_engine/mod.rs
11. src/pricing/mod.rs
12. src/notifications/mod.rs
13. src/git/*.rs (all 6 files)
14. src/sandbox/mod.rs
15. src/instructions/mod.rs

## Constraints
- Do NOT remove public API code
- Add `#[allow(dead_code)]` only to specific items, not modules
- Keep all tests passing
- Keep all public struct fields even if unused

## Verification
```bash
cargo build  # No warnings
cargo test   # All tests pass
```
