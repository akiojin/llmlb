# Development Guide

Steps for working on this repository locally.

## Prerequisites

- Rust toolchain (stable)
- CMake + C++20 compiler for xLLM
- Docker (optional)
- pnpm (for workspace tooling such as markdownlint)
- Python 3 (optional, for Codex `.skill` packaging helper scripts)

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
Use explicit E2E coverage and record results in the PR.

- E2E coverage: `LLM_TEST_MODEL=<model-id> npx bats tests/e2e/test-openai-api.bats`

## Live endpoint detection smoke tests

The integration suite includes opt-in tests against real upstream servers.
They verify automatic identification, registration through the authenticated
llmlb API, and SQLite persistence. They use an isolated llmlb database.
Normal `cargo test` runs ignore these tests; they never download models or
start an inference engine in CI. Start the upstream server yourself, then run:

```bash
LLMLB_STRATA_URL=http://127.0.0.1:19281 \
  cargo test -p llmlb --test integration_tests \
  strata_family_live_test::test_live_strata_identification_and_registration \
  -- --ignored --nocapture

LLMLB_MAYA_URL=http://127.0.0.1:19291 \
  cargo test -p llmlb --test integration_tests \
  strata_family_live_test::test_live_project_maya_identification_and_registration \
  -- --ignored --nocapture
```

For an upstream requiring authentication, also set `LLMLB_STRATA_API_KEY` or
`LLMLB_MAYA_API_KEY`. Explicitly running an ignored test without its URL fails
with a configuration error. A configured but unreachable server or incorrect
identity fails the test.

Strata v0.1.44.1 was captured on 2026-10-14 using its official `--lazy` mode
with the unmodified HTTP server configured for the freshly built Metal engine.
Model weights remained unloaded and inference was not tested. Its
[capture manifest](llmlb/tests/fixtures/detection/strata/manifest.json)
records versions, commands, and all `/health`, `/v1/models`, and `/props`
responses with authentication disabled, absent, valid, and invalid.
Project Maya v1.1.11 was captured on 2026-10-15 on Windows 11 / RTX 4090,
using the unmodified official CUDA engine with Maya-S24 loaded from
`C:/Maya-models/Maya-S24` on NVMe. Its
[capture manifest](llmlb/tests/fixtures/detection/project-maya/manifest.json)
records all 12 responses, raw HTTP bytes, body digests, and authentication
conditions. The [runtime evidence](llmlb/tests/fixtures/detection/project-maya/runtime.json)
and live smoke logs record `project_maya` detection, HTTP 201 registration,
and SQLite persistence with authentication disabled, without a Bearer token,
and with a valid token. The captured `/health` marker is `mcp_tools: false`;
MCP-enabled detection remains a separate synthetic boundary test.
Model shard digests are checked against the official release's published
values. The existing Strata service is restored after the Maya checks.
Inference quality and throughput are outside this identification test.

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
