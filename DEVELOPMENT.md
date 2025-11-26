# Development Guide

Steps for working on this repository locally.

## Prerequisites

- Rust toolchain (stable)
- CMake + C++20 compiler for C++ node
- Docker (optional)
- pnpm (for markdownlint)

## Setup
```bash
git clone https://github.com/akiojin/ollama-router.git
cd ollama-router
pnpm install --frozen-lockfile   # for lint tooling; node_modules already vendored
```

## Everyday Commands

- Format/lint/test everything: `make quality-checks`
- OpenAI-only tests: `make openai-tests`
- Router dev run: `cargo run -p or-router`
- C++ node build: `npm run build:node`
- C++ node run: `npm run start:node`

## TDD Expectations
1. Write a failing test (contract/integration first, then unit).
2. Implement the minimum to make it pass.
3. Refactor with tests green.

## Environment Variables
- Router: `ROUTER_PORT`, `DATABASE_URL`, `OPENAI_API_KEY`, `GOOGLE_API_KEY`,
  `ANTHROPIC_API_KEY`.
- Node: `OLLAMA_ROUTER_URL`, `OLLAMA_NODE_PORT`, `OLLAMA_ALLOW_NO_GPU=false`
  by default.

## Debugging Tips
- Set `RUST_LOG=debug` for verbose router output.
- Dashboard stats endpoint `/api/dashboard/stats` shows cloud key presence.
- For cloud routing, confirm the key is logged as present at startup.
