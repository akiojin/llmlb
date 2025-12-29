SHELL := /bin/sh

.PHONY: quality-checks fmt clippy test markdownlint specify-checks specify-tasks specify-tests specify-compile specify-commits
.PHONY: openai-tests test-hooks e2e-tests
.PHONY: bench-local bench-openai bench-google bench-anthropic
.PHONY: build-macos-x86_64 build-macos-aarch64 build-macos-all
.PHONY: poc-gptoss poc-gptoss-metal poc-gptoss-cuda

TASKS ?= $(shell find specs -name tasks.md)

fmt:
	cargo fmt --check

clippy:
	cargo clippy -- -D warnings

test:
	cargo test -- --test-threads=1

markdownlint:
	pnpm dlx markdownlint-cli2 "**/*.md" "!**/node_modules" "!.git" "!.github" "!.worktrees" "!CHANGELOG.md"

specify-tasks:
	@for file in $(TASKS); do \
		echo "🔍 Checking tasks in $$file"; \
		bash .specify/scripts/checks/check-tasks.sh $$file; \
	done

specify-tests:
	bash .specify/scripts/checks/check-tests.sh

specify-compile:
	bash .specify/scripts/checks/check-compile.sh

specify-commits:
	bash .specify/scripts/checks/check-commits.sh --from origin/main --to HEAD

specify-checks: specify-tasks specify-tests specify-compile specify-commits

quality-checks: fmt clippy test specify-checks markdownlint openai-tests test-hooks

openai-tests:
	cargo test -p llm-router --test openai_proxy

test-hooks:
	npx bats tests/hooks/test-block-git-branch-ops.bats tests/hooks/test-block-cd-command.bats

# E2E tests for OpenAI-compatible API (requires running router/node)
# Usage: LLM_ROUTER_URL=http://localhost:8081 LLM_ROUTER_API_KEY=sk_xxx make e2e-tests
e2e-tests:
	npx bats tests/e2e/test-openai-api.bats

# Benchmarks (wrk required)
bench-local:
	WRK_TARGET=http://localhost:8080 \
	WRK_ENDPOINT=/v1/chat/completions \
	WRK_MODEL=gpt-oss:20b \
	scripts/benchmarks/run_wrk.sh -t10 -c50 -d30s --latency | \
	scripts/benchmarks/wrk_parse.py --label local > benchmarks/results/$$(date +%Y%m%d)-local.csv

bench-openai:
	WRK_TARGET=http://localhost:8080 \
	WRK_ENDPOINT=/v1/chat/completions \
	WRK_MODEL=openai:gpt-4o \
	scripts/benchmarks/run_wrk.sh -t10 -c50 -d30s --latency | \
	scripts/benchmarks/wrk_parse.py --label openai > benchmarks/results/$$(date +%Y%m%d)-openai.csv

bench-google:
	WRK_TARGET=http://localhost:8080 \
	WRK_ENDPOINT=/v1/chat/completions \
	WRK_MODEL=google:gemini-1.5-pro \
	scripts/benchmarks/run_wrk.sh -t10 -c50 -d30s --latency | \
	scripts/benchmarks/wrk_parse.py --label google > benchmarks/results/$$(date +%Y%m%d)-google.csv

bench-anthropic:
	WRK_TARGET=http://localhost:8080 \
	WRK_ENDPOINT=/v1/chat/completions \
	WRK_MODEL=anthropic:claude-3-opus \
	scripts/benchmarks/run_wrk.sh -t10 -c50 -d30s --latency | \
	scripts/benchmarks/wrk_parse.py --label anthropic > benchmarks/results/$$(date +%Y%m%d)-anthropic.csv

# macOS cross-compilation targets
build-macos-x86_64:
	@echo "Building for macOS x86_64 (Intel)..."
	cargo build --release --target x86_64-apple-darwin \
		-p llm-router

build-macos-aarch64:
	@echo "Building for macOS aarch64 (Apple Silicon)..."
	cargo build --release --target aarch64-apple-darwin \
		-p llm-router

build-macos-all: build-macos-x86_64 build-macos-aarch64
	@echo "All macOS builds completed successfully!"

# PoCs
poc-gptoss-metal:
	./poc/gpt-oss-metal/run.sh

poc-gptoss-cuda:
	./poc/gpt-oss-cuda/run.sh

poc-gptoss:
	@case "$$(uname -s)" in \
		Darwin) $(MAKE) poc-gptoss-metal ;; \
		Linux) $(MAKE) poc-gptoss-cuda ;; \
		*) echo "Unsupported OS for gpt-oss PoC: $$(uname -s)" >&2; exit 1 ;; \
	esac
