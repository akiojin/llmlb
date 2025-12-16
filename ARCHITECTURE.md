# Architecture

LLM Router coordinates local ONNX Runtime-based nodes and optionally proxies to cloud
LLM providers via model prefixes. This document outlines the high-level
components; no source code is included here.

## Components
- **Router (Rust)**: Receives OpenAI-compatible traffic, chooses a path, and
  proxies requests. Exposes dashboard, metrics, and admin APIs.
- **Local Nodes (C++ / ONNX Runtime)**: Execute local inference and register/send
  heartbeats to the router. (ASR uses `whisper.cpp`; other modalities use ONNX
  Runtime-based backends.)
- **Cloud Proxy**: When a model name starts with `openai:` `google:` or
  `anthropic:` the router forwards to the corresponding cloud API.
- **Storage**: SQLite for router metadata; model files live on each node.
- **Observability**: Prometheus metrics, structured logs, dashboard stats.

## Request Flow
```
Client
  │ POST /v1/chat/completions
  ▼
Router (OpenAI-compatible)
  ├─ Prefix? → Cloud API (OpenAI / Google / Anthropic)
  └─ No prefix → Scheduler → Local Node
                       └─ local inference → Response
```

## Scheduling & Health
- Nodes register via `/v0/nodes`; router rejects nodes without GPUs by default.
- Heartbeats carry CPU/GPU/memory metrics used for load balancing.
- Dashboard surfaces `*_key_present` flags so operators see which cloud keys
  are configured.

## Configuration Surface
- Router environment: `ROUTER_PORT`, `DATABASE_URL`, cloud keys and base URLs.
- Node environment: `LLM_ROUTER_URL`, `LLM_NODE_PORT`, etc. (GPU is required for
  node registration/operation.)

## Deployment Options
- Bare metal: build router with `cargo build -p llm-router --release`.
- Docker: `docker build -t llm-router .` then run with `--gpus all` when
  GPUs are required.
- Nodes can be packaged as RPM/DEB/Homebrew/MSI; see installers/ for scripts.
