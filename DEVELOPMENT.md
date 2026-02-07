# Development Guide

Steps for working on this repository locally.

## Prerequisites

- Rust toolchain (stable)
- CMake + C++20 compiler for xLLM
- Docker (optional)
- pnpm (for workspace tooling such as markdownlint)
- npm (for mcp-server)

## Setup
```bash
git clone https://github.com/akiojin/llmlb.git
cd llmlb
pnpm install --frozen-lockfile   # for lint tooling; node_modules already vendored
```

## Everyday Commands

- Format/lint/test everything: `make quality-checks`
- Security advisories (Rust): `make security-checks`
- OpenAI-only tests: `make openai-tests`
- Router dev run (default): `cargo run -p llmlb`
- Router dev run (headless): `cargo run -p llmlb -- serve --no-tray`
- xLLM build: `npm run build:node`
- xLLM run: `npm run start:node`

## TDD Expectations
1. Write a failing test (contract/integration first, then unit).
2. Implement the minimum to make it pass.
3. Refactor with tests green.

## Required model-family tests
For gpt/nemotron/qwen/glm model families, verification is mandatory before merge.
Use the model verification suite or explicit E2E coverage and record results in the PR.

- Model verification: `.specify/scripts/model-verification/run-verification.sh --model <path> --format <gguf|safetensors> --capability TextGeneration --platform <platform>`
- E2E coverage: `LLM_TEST_MODEL=<model-id> npx bats tests/e2e/test-openai-api.bats`

## Environment Variables
- Router: `LLMLB_HOST`, `LLMLB_PORT`, `LLMLB_DATABASE_URL` (or `DATABASE_URL`),
  `LLMLB_LOG_LEVEL`, `LLMLB_ADMIN_USERNAME`, `LLMLB_ADMIN_PASSWORD`,
  `OPENAI_API_KEY`, `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`.
- xLLM: `LLMLB_URL`, `XLLM_PORT`, `LLM_ALLOW_NO_GPU=false`
  by default.

## Debugging Tips

- Set `RUST_LOG=debug` for verbose load balancer output.
- Dashboard stats endpoint `/api/dashboard/stats` shows cloud key presence.
- For cloud routing, confirm the key is logged as present at startup.

## Token Statistics

The load balancer tracks token usage for all requests (prompt_tokens, completion_tokens,
total_tokens). Statistics are persisted to SQLite and available via dashboard API.

- **Data source**: Node response `usage` field (preferred), tiktoken estimation (fallback)
- **Streaming**: Tokens accumulated per chunk, final usage from last chunk
- **API endpoints**: `/api/dashboard/stats/tokens`, `/api/dashboard/stats/tokens/daily`,
  `/api/dashboard/stats/tokens/monthly`
- **Dashboard**: Statistics tab shows daily/monthly breakdown

## Submodules
- xLLM has moved to <https://github.com/akiojin/xLLM>.
  Submodule policies for runtime dependencies are documented there.
