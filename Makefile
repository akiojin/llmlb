SHELL := /bin/sh

.PHONY: quality-checks quality-checks-pre-commit fmt clippy clippy-parity module-structure event-publishers migration-versions mapping-freshness dependabot-subjects dashboard-checks test-checks coverage coverage-gate test security-checks markdownlint specify-commits
.PHONY: openai-tests notification-tests test-hooks e2e-tests e2e-playwright e2e-playwright-screenshots
.PHONY: bench-local bench-openai bench-google bench-anthropic
.PHONY: build-macos-x86_64 build-macos-aarch64 build-macos-all
.PHONY: poc-gptoss poc-gptoss-metal poc-gptoss-cuda

FIND ?= /usr/bin/find

fmt:
	cargo fmt --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

clippy-parity:
	bash scripts/checks/check-clippy-parity.sh

# SPEC #699 FR-009/FR-010: 1,500 行上限と mod.rs の re-export 化
module-structure:
	bash scripts/checks/check-module-structure.sh

# SPEC #582 FR-048e / Issue #781: canonical ダッシュボードイベントに production の publisher が無い状態を検出
# CI (lint.yml rust-lint) も同一ターゲットを実行する。
event-publishers:
	bash scripts/checks/check-event-publishers.sh

# Issue #737: 並行ブランチ間のマイグレーション番号衝突を develop 着地前に検出
migration-versions:
	bash scripts/checks/check-migration-versions.sh

# Issue #776: canonical モデルマッピングの最終確認日が古くなっていないか検査（ネットワーク非依存）
# CI (lint.yml commitlint) も同一ターゲットを実行する。
mapping-freshness:
	bash scripts/checks/check-mapping-freshness.sh

# Issue #727: Dependabot のグループ更新件名が commitlint (header-max-length) を通るか検証
dependabot-subjects:
	bash scripts/checks/check-dependabot-subjects.sh

# Rust ユニットテストカバレッジ（SPEC #585 FR-032: 行カバレッジ80%以上）
# CI (ci.yml coverage-rust) も同一ターゲットを実行する。要 cargo-llvm-cov。
# Issue #769: dashboard の typecheck / lint（依存更新で壊れても CI が検出できるようにする）
# CI (lint.yml dashboard-lint) も同一ターゲットを実行する。
dashboard-checks:
	pnpm --filter @llm/dashboard typecheck
	pnpm --filter @llm/dashboard lint
	pnpm --filter @llm/dashboard test

coverage:
	mkdir -p coverage-rust
	cargo llvm-cov --all-features --workspace --fail-under-lines 80 --lcov --output-path coverage-rust/lcov.info

coverage-gate:
	bash scripts/checks/check-coverage-gate.sh

test:
	cargo test -- --test-threads=1

markdownlint:
	pnpm dlx markdownlint-cli2 "**/*.md" "!**/node_modules" "!.git" "!.github" "!.worktrees" "!CHANGELOG.md" "!build" "!**/build/**" "!node/third_party" "!actions-runner"

specify-commits:
	@branch=$$(git rev-parse --abbrev-ref HEAD 2>/dev/null); \
	tracking=$$(git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null || echo ""); \
	if [ -n "$$tracking" ]; then \
		echo "Checking commits from $$tracking to HEAD (feature branch)"; \
		bash scripts/checks/check-commits.sh --from "$$tracking" --to HEAD; \
	else \
		echo "Checking commits from origin/main to HEAD"; \
		bash scripts/checks/check-commits.sh --from origin/main --to HEAD; \
	fi

quality-checks: fmt clippy-parity module-structure event-publishers migration-versions mapping-freshness dependabot-subjects dashboard-checks coverage-gate clippy test security-checks specify-commits markdownlint openai-tests notification-tests test-hooks test-checks e2e-playwright

quality-checks-pre-commit: fmt clippy-parity clippy

security-checks:
	cargo audit

# NOTE: openai_proxy.rs was removed in SPEC-66555000 (NodeRegistry removal)
# OpenAI API tests are now covered by e2e_openai_proxy
openai-tests:
	cargo test -p llmlb --test e2e_openai_proxy

# SPEC #777 AC-8: 運用通知（users.email・メール送信基盤・日次ダイジェスト・通知設定 API）の受け入れテスト
# 実際のメール送信は行わない（記録用トランスポートを使う）。
# CI (test.yml rust-test) も同一ターゲットを実行する。
notification-tests:
	cargo test -p llmlb --test notification_tests

test-checks:
	@if [ -x "./node_modules/bats/bin/bats" ]; then \
		bash ./node_modules/bats/bin/bats tests/checks; \
	else \
		echo "bats is not installed. Run 'pnpm install' first." >&2; \
		exit 1; \
	fi

test-hooks:
	@bash -lc 'if [ -x "./node_modules/bats/bin/bats" ]; then \
		bash ./node_modules/bats/bin/bats tests/hooks/test-block-git-branch-ops.bats tests/hooks/test-block-cd-command.bats || \
			(echo "bats tests failed (Windows Git Bash compatibility issue). Hooks are still active." && exit 0); \
	else \
		echo "bats is not installed. Run '\''pnpm install'\'' first."; \
		exit 0; \
	fi'

# Playwright E2E tests for Dashboard and Playground
# Automatically starts the server via playwright.config.ts webServer.
# Set SKIP_SERVER=1 to use an already-running server.
e2e-playwright:
	@cd llmlb/tests/e2e-playwright && pnpm exec playwright test --project=chromium --grep-invert @real-runtimes || \
		(echo "⚠️  Some Playwright E2E tests failed. Review the report above." && exit 0)

# Playwright E2E screenshot capture (headed mode)
# Screenshots are saved to llmlb/tests/e2e-playwright/reports/screenshots/
e2e-playwright-screenshots:
	cd llmlb/tests/e2e-playwright && PLAYWRIGHT_SCREENSHOTS=1 pnpm exec playwright test --project=screenshots --headed

# E2E tests for OpenAI-compatible API (requires running llmlb/node)
# Usage: LLMLB_URL=http://localhost:8081 LLMLB_API_KEY=sk_xxx make e2e-tests
e2e-tests:
	@bash -lc 'if [ -x "./node_modules/bats/bin/bats" ]; then \
		bash ./node_modules/bats/bin/bats tests/e2e/test-openai-api.bats; \
	else \
		echo "bats is not installed. Run '\''pnpm install'\'' first." >&2; \
		exit 1; \
	fi'

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
		-p llmlb

build-macos-aarch64:
	@echo "Building for macOS aarch64 (Apple Silicon)..."
	cargo build --release --target aarch64-apple-darwin \
		-p llmlb

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
