# Development Guide

Steps for working on this repository locally.

## Prerequisites

- Rust toolchain (stable)
- CMake + C++20 compiler for C++ node
- Docker (optional)
- pnpm (for markdownlint)

## Setup
```bash
git clone https://github.com/akiojin/llm-router.git
cd llm-router
pnpm install --frozen-lockfile   # for lint tooling; node_modules already vendored
```

## Everyday Commands

- Format/lint/test everything: `make quality-checks`
- OpenAI-only tests: `make openai-tests`
- Router dev run: `cargo run -p llm-router`
- C++ node build: `npm run build:node`
- C++ node run: `npm run start:node`

## TDD Expectations
1. Write a failing test (contract/integration first, then unit).
2. Implement the minimum to make it pass.
3. Refactor with tests green.

## Environment Variables
- Router: `ROUTER_PORT`, `DATABASE_URL`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`,
  `ANTHROPIC_API_KEY`.
- Node: `LLM_ROUTER_URL`, `LLM_NODE_PORT`, `LLM_ALLOW_NO_GPU=false`
  by default.

## Debugging Tips

- Set `RUST_LOG=debug` for verbose router output.
- Dashboard stats endpoint `/v0/dashboard/stats` shows cloud key presence.
- For cloud routing, confirm the key is logged as present at startup.

## Token Statistics

The router tracks token usage for all requests (prompt_tokens, completion_tokens,
total_tokens). Statistics are persisted to SQLite and available via dashboard API.

- **Data source**: Node response `usage` field (preferred), tiktoken estimation (fallback)
- **Streaming**: Tokens accumulated per chunk, final usage from last chunk
- **API endpoints**: `/v0/dashboard/stats/tokens`, `/v0/dashboard/stats/tokens/daily`,
  `/v0/dashboard/stats/tokens/monthly`
- **Dashboard**: Statistics tab shows daily/monthly breakdown

## Submodules
- `allm/third_party/stable-diffusion.cpp` is pinned to the public fork
  `https://github.com/akiojin/stable-diffusion.cpp.git` to carry project-specific
  crash/compatibility fixes.
- Upstream (leejet/stable-diffusion.cpp) updates are synced manually on demand
  (cherry-pick/merge into the fork) to keep control of breaking changes.
- We do not plan to open upstream PRs for these changes unless explicitly requested.
- Third-party OSS should be added as git submodules.
- Do not modify submodule contents directly. If changes are required, use a fork
  and update the submodule pointer.
