# Doodoori Makefile
# Autonomous CLI tool powered by Claude Code

.PHONY: all build build-sandbox build-full release release-sandbox release-full \
        test test-all clean install uninstall \
        docker-build docker-up docker-down docker-shell docker-logs docker-start \
        sandbox-login sandbox-status sandbox-cleanup sandbox-cleanup-containers sandbox-cleanup-volumes \
        run run-sandbox run-dry fmt clippy check lint watch watch-test stats doc doc-all help

# Default target
all: build

# =============================================================================
# Build Targets
# =============================================================================

## Build debug version (default, no optional features)
build:
	cargo build

## Build with sandbox feature
build-sandbox:
	cargo build --features sandbox

## Build with all features
build-full:
	cargo build --features full

## Build release version
release:
	cargo build --release

## Build release with sandbox feature
release-sandbox:
	cargo build --release --features sandbox

## Build release with all features
release-full:
	cargo build --release --features full

# =============================================================================
# Test Targets
# =============================================================================

## Run tests
test:
	cargo test

## Run tests with all features
test-all:
	cargo test --all-features

## Run tests with output
test-verbose:
	cargo test -- --nocapture

# =============================================================================
# Code Quality
# =============================================================================

## Format code
fmt:
	cargo fmt

## Run clippy linter
clippy:
	cargo clippy --all-features -- -D warnings

## Check compilation without building
check:
	cargo check --all-features

## Run all quality checks
lint: fmt clippy

# =============================================================================
# Docker Targets
# =============================================================================

## Build Docker sandbox image
docker-build:
	docker build -t doodoori/sandbox:latest -f docker/Dockerfile.sandbox .

## Start sandbox container (detached)
docker-up:
	docker-compose -f docker/docker-compose.yml up -d sandbox

## Stop sandbox container
docker-down:
	docker-compose -f docker/docker-compose.yml down

## Open shell in sandbox container
docker-shell:
	docker-compose -f docker/docker-compose.yml exec sandbox bash

## Build and start sandbox
docker-start: docker-build docker-up

## View sandbox logs
docker-logs:
	docker-compose -f docker/docker-compose.yml logs -f sandbox

## Login to Claude in sandbox (required for subscription auth)
sandbox-login:
	cargo run --features sandbox -- sandbox login

## Check sandbox status
sandbox-status:
	cargo run --features sandbox -- sandbox status

## Cleanup sandbox resources (volumes and containers)
sandbox-cleanup:
	cargo run --features sandbox -- sandbox cleanup --all

## Cleanup only sandbox containers
sandbox-cleanup-containers:
	cargo run --features sandbox -- sandbox cleanup --containers

## Cleanup only credentials volume
sandbox-cleanup-volumes:
	cargo run --features sandbox -- sandbox cleanup --volumes

# =============================================================================
# Run Targets
# =============================================================================

## Run with prompt (usage: make run PROMPT="your task")
run:
	cargo run -- run "$(PROMPT)"

## Run in sandbox mode
run-sandbox:
	cargo run --features sandbox -- run --sandbox "$(PROMPT)"

## Dry run (preview)
run-dry:
	cargo run -- run --dry-run "$(PROMPT)"

## Run spec command
spec:
	cargo run -- spec "$(PROMPT)"

# =============================================================================
# Installation
# =============================================================================

## Install to ~/.cargo/bin
install:
	cargo install --path . --features full

## Install without optional features
install-minimal:
	cargo install --path .

## Uninstall
uninstall:
	cargo uninstall doodoori

# =============================================================================
# Clean
# =============================================================================

## Clean build artifacts
clean:
	cargo clean

## Clean everything including Docker images
clean-all: clean
	-docker rmi doodoori/sandbox:latest 2>/dev/null || true

# =============================================================================
# Documentation
# =============================================================================

## Generate documentation
doc:
	cargo doc --no-deps --open

## Generate documentation for all features
doc-all:
	cargo doc --all-features --no-deps --open

# =============================================================================
# Development Helpers
# =============================================================================

## Watch and rebuild on changes (requires cargo-watch)
watch:
	cargo watch -x build

## Watch and test on changes
watch-test:
	cargo watch -x test

## Show project stats
stats:
	@echo "=== Lines of Code ==="
	@find src -name "*.rs" | xargs wc -l | tail -1
	@echo ""
	@echo "=== Test Count ==="
	@cargo test 2>&1 | grep -E "^test result:" || echo "Run 'make test' first"
	@echo ""
	@echo "=== Dependencies ==="
	@cargo tree --depth 1 | wc -l | xargs echo "Direct dependencies:"

# =============================================================================
# Help
# =============================================================================

## Show this help
help:
	@echo "Doodoori - Autonomous CLI tool powered by Claude Code"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Build Targets:"
	@echo "  build            Build debug version (default)"
	@echo "  build-sandbox    Build with sandbox feature"
	@echo "  build-full       Build with all features"
	@echo "  release          Build release version"
	@echo "  release-sandbox  Build release with sandbox"
	@echo "  release-full     Build release with all features"
	@echo ""
	@echo "Test & Quality:"
	@echo "  test             Run tests"
	@echo "  test-all         Run tests with all features"
	@echo "  fmt              Format code"
	@echo "  clippy           Run linter"
	@echo "  lint             Run fmt + clippy"
	@echo ""
	@echo "Docker & Sandbox:"
	@echo "  docker-build     Build sandbox Docker image"
	@echo "  docker-up        Start sandbox container"
	@echo "  docker-down      Stop sandbox container"
	@echo "  docker-shell     Open shell in sandbox"
	@echo "  docker-start     Build and start sandbox"
	@echo "  sandbox-login    Login to Claude in sandbox (required first)"
	@echo "  sandbox-status   Check sandbox status"
	@echo "  sandbox-cleanup  Cleanup all sandbox resources"
	@echo ""
	@echo "Run:"
	@echo "  run PROMPT=\"...\"         Run with prompt"
	@echo "  run-sandbox PROMPT=\"...\" Run in sandbox mode"
	@echo "  run-dry PROMPT=\"...\"     Dry run (preview)"
	@echo ""
	@echo "Installation:"
	@echo "  install          Install to ~/.cargo/bin (full features)"
	@echo "  install-minimal  Install without optional features"
	@echo "  uninstall        Uninstall"
	@echo ""
	@echo "Other:"
	@echo "  clean            Clean build artifacts"
	@echo "  clean-all        Clean everything including Docker"
	@echo "  doc              Generate documentation"
	@echo "  help             Show this help"
