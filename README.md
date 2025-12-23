# LLM Router

A centralized management system for coordinating LLM inference nodes across multiple machines

English | [日本語](./README.ja.md)

## Overview

LLM Router is a powerful centralized system that provides unified management and a single API endpoint for multiple LLM inference nodes running across different machines. It features intelligent load balancing, automatic failure detection, real-time monitoring capabilities, and seamless integration for enhanced scalability.

## Key Features

- **Unified API Endpoint**: Access multiple LLM runtime instances through a single URL
- **Automatic Load Balancing**: Intelligently distribute requests across available nodes
- **Automatic Failure Detection**: Detect offline nodes and exclude them from routing
- **Real-time Monitoring**: Comprehensive visualization of node states and performance metrics via web dashboard
- **Request History Tracking**: Complete request/response logging with 7-day retention
- **Self-registering Nodes**: Nodes automatically register with the Router
- **Node-driven Model Sync**: Nodes pull models via router `/v0/models` and `/v0/models/blob/:model_name` (no push-based distribution)
- **WebUI Management**: Manage node settings, monitoring, and control through
  browser-based dashboard
- **Cross-Platform Support**: Works on Windows 10+, macOS 12+, and Linux
- **GPU-Aware Routing**: Intelligent request routing based on GPU capabilities
  and availability
- **Cloud Model Prefixes**: Add `openai:` `google:` or `anthropic:` in the
  model name to proxy to the corresponding cloud provider while keeping the
  same OpenAI-compatible endpoint.

## MCP Server for LLM Assistants

LLM assistants (like Claude Code) can interact with LLM Router through a dedicated
MCP server. This is the recommended approach over using Bash with curl commands
directly.

### Why MCP Server over Bash + curl?

| Feature | MCP Server | Bash + curl |
|---------|------------|-------------|
| Authentication | Auto-injected | Manual header management |
| Security | Host whitelist, injection prevention | No built-in protection |
| Shell injection | Protected (shell: false) | Vulnerable |
| API documentation | Built-in as MCP resources | External reference needed |
| Credential handling | Automatic masking in logs | Exposed in command history |
| Timeout management | Configurable per-request | Manual implementation |
| Error handling | Structured JSON responses | Raw text parsing |

### Installation

```bash
npm install -g @llm-router/mcp-server
# or
npx @llm-router/mcp-server
```

### Configuration (.mcp.json)

```json
{
  "mcpServers": {
    "llm-router": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@llm-router/mcp-server"],
      "env": {
        "LLM_ROUTER_URL": "http://localhost:8080",
        "LLM_ROUTER_API_KEY": "sk_your_api_key"
      }
    }
  }
}
```

For detailed documentation, see [mcp-server/README.md](./mcp-server/README.md).

## Quick Start

### Router (llm-router)

```bash
# Build
cargo build --release -p llm-router

# Run
./target/release/llm-router
# Default: http://0.0.0.0:8080

# Access dashboard
# Open http://localhost:8080/dashboard in browser
```

**Environment Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_ROUTER_HOST` | `0.0.0.0` | Bind address |
| `LLM_ROUTER_PORT` | `8080` | Listen port |
| `LLM_ROUTER_LOG_LEVEL` | `info` | Log level |
| `LLM_ROUTER_JWT_SECRET` | (auto-generated) | JWT signing secret |
| `LLM_ROUTER_ADMIN_USERNAME` | `admin` | Initial admin username |
| `LLM_ROUTER_ADMIN_PASSWORD` | (required) | Initial admin password |

**Backward compatibility:** Legacy env var names (`ROUTER_PORT` etc.) are supported but deprecated.

**System Tray (Windows/macOS only):**

On Windows 10+ and macOS 12+, the router displays a system tray icon.
Double-click to open the dashboard. Docker/Linux runs as a headless CLI process.

### CLI Reference

The router CLI currently exposes only basic flags (`--help`, `--version`).
Day-to-day management is done via the Dashboard UI (`/dashboard`) or the HTTP APIs.

### Node (C++)

**Prerequisites:**

```bash
# macOS
brew install cmake

# Ubuntu/Debian
sudo apt install cmake build-essential

# Windows
# Download from https://cmake.org/download/
```

**Build & Run:**

```bash
# Build (Metal is enabled by default on macOS)
npm run build:node

# Build (Linux / CUDA)
npm run build:node:cuda

# Run
npm run start:node

# Or manually:
# cd node && cmake -B build -S . && cmake --build build --config Release
# # Linux / CUDA:
# # cd node && cmake -B build -S . -DBUILD_WITH_CUDA=ON && cmake --build build --config Release
# LLM_ROUTER_URL=http://localhost:8080 ./node/build/llm-node
```

**Environment Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_ROUTER_URL` | `http://127.0.0.1:8080` | Router URL to register with |
| `LLM_NODE_PORT` | `11435` | Node listen port |
| `LLM_NODE_MODELS_DIR` | `~/.llm-router/models` | Model storage directory |
| `LLM_NODE_BIND_ADDRESS` | `0.0.0.0` | Bind address |
| `LLM_NODE_HEARTBEAT_SECS` | `10` | Heartbeat interval (seconds) |
| `LLM_NODE_LOG_LEVEL` | `info` | Log level |

**Backward compatibility:** Legacy env var names (`LLM_MODELS_DIR` etc.) are supported but deprecated.

**Docker:**

```bash
# Build
docker build --build-arg CUDA=cpu -t llm-node:latest node/

# Run
docker run --rm -p 11435:11435 \
  -e LLM_ROUTER_URL=http://host.docker.internal:8080 \
  llm-node:latest
```

## Load Balancing

LLM Router supports multiple load balancing strategies to optimize request distribution across nodes.

### Strategies

#### 1. Metrics-Based Load Balancing (Recommended)

Selects nodes based on real-time metrics (CPU usage, memory usage, active requests). This intelligent mode provides optimal performance by dynamically routing requests to the least loaded node, ensuring efficient resource utilization.

**Configuration:**
```bash
# Enable metrics-based load balancing
LLM_ROUTER_LOAD_BALANCER_MODE=metrics cargo run -p llm-router
```

**Load Score Calculation:**
```
score = cpu_usage + memory_usage + (active_requests × 10)
```

The node with the **lowest score** is selected. If all nodes have CPU usage > 80%, the system automatically falls back to round-robin.

**Example:**
- Node A: CPU 20%, Memory 30%, Active 1 → Score = 60 ✓ Selected
- Node B: CPU 70%, Memory 50%, Active 5 → Score = 170

#### 2. Advanced Load Balancing (Default)

Combines multiple factors including response time, active requests, and CPU usage to provide sophisticated node selection with adaptive performance optimization.

**Configuration:**
```bash
# Use default advanced load balancing (or omit LOAD_BALANCER_MODE)
LLM_ROUTER_LOAD_BALANCER_MODE=auto cargo run -p llm-router
```

### Health / Metrics API

Nodes report health + metrics to the Router for node status and load balancing decisions.

**Endpoint:** `POST /v0/health` (requires `X-Node-Token` + API key with `node:register`)

**Headers:**
- `Authorization: Bearer <api_key>`
- `X-Node-Token: <node_token>`

**Request:**
```json
{
  "node_id": "550e8400-e29b-41d4-a716-446655440000",
  "cpu_usage": 45.5,
  "memory_usage": 60.2,
  "active_requests": 3,
  "average_response_time_ms": 250.5,
  "loaded_models": ["gpt-oss-20b"],
  "loaded_embedding_models": [],
  "initializing": false,
  "ready_models": [1, 1]
}
```

**Response:** `200 OK`

## Architecture

LLM Router coordinates local llama.cpp nodes and optionally proxies to cloud LLM providers via model prefixes.

### Components
- **Router (Rust)**: Receives OpenAI-compatible traffic, chooses a path, and proxies requests. Exposes dashboard, metrics, and admin APIs.
- **Local Nodes (C++ / llama.cpp)**: Serve GGUF models; register and send heartbeats to the router.
- **Cloud Proxy**: When a model name starts with `openai:` `google:` or `anthropic:` the router forwards to the corresponding cloud API.
- **Storage**: SQLite for router metadata; model files live on each node.
- **Observability**: Prometheus metrics, structured logs, dashboard stats.

### System Overview

![System Overview](docs/diagrams/architecture.readme.en.svg)

Draw.io source: `docs/diagrams/architecture.drawio` (Page: System Overview (README.md))

### Request Flow
```
Client
  │ POST /v1/chat/completions
  ▼
Router (OpenAI-compatible)
  ├─ Prefix? → Cloud API (OpenAI / Google / Anthropic)
  └─ No prefix → Scheduler → Local Node
                       └─ llama.cpp inference → Response
```

### Communication Flow (Proxy Pattern)

LLM Router uses a **Proxy Pattern** - clients only need to know the Router URL.

#### Traditional Method (Without Router)
```bash
# Direct access to each node API (default: node_port=11435)
curl http://machine1:11435/v1/chat/completions -d '...'
curl http://machine2:11435/v1/chat/completions -d '...'
curl http://machine3:11435/v1/chat/completions -d '...'
```

#### With Router (Proxy)
```bash
# Unified access to Router - automatic routing to the optimal node
curl http://router:8080/v1/chat/completions -d '...'
curl http://router:8080/v1/chat/completions -d '...'
curl http://router:8080/v1/chat/completions -d '...'
```

**Detailed Request Flow:**

1. **Client → Router**
   ```
   POST http://router:8080/v1/chat/completions
   Content-Type: application/json

   {"model": "llama2", "messages": [...]}
   ```

2. **Router Internal Processing**
   - Select optimal node (Load Balancing)
   - Forward request to selected node via HTTP client

3. **Router → Node (Internal Communication)**
   ```
   POST http://node1:11435/v1/chat/completions
   Content-Type: application/json

   {"model": "llama2", "messages": [...]}
   ```

4. **Node Local Processing**
   - Node loads model on-demand (from local cache or router-provided source)
   - Node runs llama.cpp inference and returns an OpenAI-compatible response

5. **Router → Client (Return Response)**
   ```json
   {
     "id": "chatcmpl-xxx",
     "object": "chat.completion",
     "choices": [{
       "index": 0,
       "message": {"role": "assistant", "content": "..."},
       "finish_reason": "stop"
     }]
   }
   ```

> **Note**: LLM Router exclusively supports **OpenAI-compatible API format**.
> All responses follow the OpenAI Chat Completions API specification.

**From Client's Perspective**:
- Router appears as the only OpenAI-compatible API server
- No need to be aware of multiple internal nodes
- Complete with a single HTTP request

### Model Sync (No Push Distribution)

- The router never pushes models to nodes.
- Nodes pull the router's model list via `GET /v1/models`.
- For each model, nodes either:
  - use the router-provided `path` directly (shared storage), or
  - fetch metadata from `GET /v0/models` and download the model from `GET /v0/models/blob/:model_name`.

### Scheduling & Health
- Nodes register via `/v0/nodes`; router rejects nodes without GPUs by default.
- Heartbeats carry CPU/GPU/memory metrics used for load balancing.
- Dashboard surfaces `*_key_present` flags so operators see which cloud keys are configured.

### Benefits of Proxy Pattern

1. **Unified Endpoint**
   - Clients only need to know the Router URL
   - No need to know each node location

2. **Transparent Load Balancing**
   - Router automatically selects the optimal node
   - Clients benefit from load distribution without awareness

3. **Automatic Retry on Failure**
   - If Node1 fails → Router automatically tries Node2
   - No re-request needed from client

4. **Security**
   - Node IP addresses not exposed to clients
   - Only Router needs to be publicly accessible

5. **Scalability**
   - Adding nodes automatically increases processing capacity
   - No changes needed on client side

## Project Structure

```
llm-router/
├── common/              # Shared library (types, protocol, errors)
├── router/              # Rust router (HTTP APIs, dashboard, proxy)
├── node/                # C++ node (llama.cpp, OpenAI-compatible /v1/*)
├── mcp-server/          # MCP server (for LLM assistants like Claude Code)
└── specs/               # Specifications (Spec-Driven Development)
```

## Dashboard

The dashboard is served by the router at `/dashboard`.
Use it to monitor nodes, view request history, inspect logs, and manage models.

### Quick usage

1. Start the router:
   ```bash
   cargo run -p llm-router
   ```
2. Open:
   ```
   http://localhost:8080/dashboard
   ```

## Hugging Face registration (safetensors / GGUF)

- Optional env vars: set `HF_TOKEN` to raise Hugging Face rate limits; set `HF_BASE_URL` when using a mirror/cache.
- Web (recommended):
  - Dashboard → **Models** → **Register**
  - Choose `format`: `safetensors` (native engine: TBD) or `gguf` (llama.cpp fallback).
    - If the repo contains both `safetensors` and `.gguf`, `format` is required.
    - Note: text generation via `safetensors` is TBD (engine implementation will be decided later). Use `gguf` if you need to run the model now.
  - Enter a Hugging Face repo (e.g. `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16`).
  - For `format=gguf`:
    - Either specify an exact `.gguf` `filename`, or choose `gguf_policy` (`quality` / `memory` / `speed`)
      to auto-pick from GGUF siblings.
  - For `format=safetensors`:
    - The HF snapshot must include `config.json` and `tokenizer.json`.
    - Sharded weights must include an `.index.json`.
  - Model IDs are the Hugging Face repo ID (e.g. `org/model`).
  - `/v1/models` lists models including queued/caching/error with `lifecycle_status` + `download_progress`.
  - Nodes pull models on-demand via the model registry endpoints:
    - `GET /v0/models/registry/:model_name/manifest.json`
    - `GET /v0/models/registry/:model_name/files/:file_name`
    - (Legacy) `GET /v0/models/blob/:model_name` for single-file GGUF.

## Installation

### Prerequisites
- Linux/macOS/Windows x64 (GPU recommended)
- Rust toolchain (stable) and cargo
- Docker (optional)
- CUDA driver (for NVIDIA GPU)

### 1) Build from Rust source (Recommended)
```bash
git clone https://github.com/akiojin/llm-router.git
cd llm-router
make quality-checks   # fmt/clippy/test/markdownlint
cargo build -p llm-router --release
```
Artifact: `target/release/llm-router`

### 2) Run with Docker
```bash
docker build -t llm-router:latest .
docker run --rm -p 8080:8080 --gpus all \
  -e OPENAI_API_KEY=... \
  llm-router:latest
```
If not using GPU, remove `--gpus all` or set `CUDA_VISIBLE_DEVICES=""`.

### 3) C++ Node Build
See [Node (C++)](#node-c) section in Quick Start.

### Requirements

- **Router**: Rust toolchain (stable)
- **Node**: CMake + a C++ toolchain, and a supported GPU (NVIDIA / AMD / Apple Silicon)

## Usage

### Basic Usage

1. **Start Router**
   ```bash
   ./target/release/llm-router
   # Default: http://0.0.0.0:8080
   ```

2. **Start Nodes on Multiple Machines**
   ```bash
   # Machine 1
   LLM_ROUTER_URL=http://router:8080 \
   # Replace with your actual API key (scope: node:register)
   LLM_NODE_API_KEY=sk_your_node_register_key \
   ./node/build/llm-node

   # Machine 2
   LLM_ROUTER_URL=http://router:8080 \
   # Replace with your actual API key (scope: node:register)
   LLM_NODE_API_KEY=sk_your_node_register_key \
   ./node/build/llm-node
   ```

3. **Send Inference Requests to Router (OpenAI-compatible)**
   ```bash
   curl http://router:8080/v1/chat/completions \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer sk_your_api_key" \
     -d '{
       "model": "gpt-oss-20b",
       "messages": [{"role": "user", "content": "Hello!"}],
       "stream": false
     }'
   ```

4. **List Registered Nodes**
   ```bash
   curl http://router:8080/v0/nodes \
     # Replace with your actual API key (scope: admin:*)
     -H "Authorization: Bearer sk_your_admin_key"
   ```

### Environment Variables

#### Router (llm-router)

| Variable | Default | Description | Legacy / Notes |
|----------|---------|-------------|----------------|
| `LLM_ROUTER_HOST` | `0.0.0.0` | Bind address | `ROUTER_HOST` |
| `LLM_ROUTER_PORT` | `8080` | Listen port | `ROUTER_PORT` |
| `LLM_ROUTER_DATABASE_URL` | `sqlite:~/.llm-router/router.db` | Database URL | `DATABASE_URL` |
| `LLM_ROUTER_DATA_DIR` | `~/.llm-router` | Base directory for DB/log defaults | - |
| `LLM_ROUTER_JWT_SECRET` | (auto-generated) | JWT signing secret | `JWT_SECRET` |
| `LLM_ROUTER_ADMIN_USERNAME` | `admin` | Initial admin username | `ADMIN_USERNAME` |
| `LLM_ROUTER_ADMIN_PASSWORD` | (required, first run) | Initial admin password | `ADMIN_PASSWORD` |
| `LLM_ROUTER_LOG_LEVEL` | `info` | Log level (`EnvFilter`) | `LLM_LOG_LEVEL`, `RUST_LOG` |
| `LLM_ROUTER_LOG_DIR` | `~/.llm-router/logs` | Log directory | `LLM_LOG_DIR` (deprecated) |
| `LLM_ROUTER_LOG_RETENTION_DAYS` | `7` | Log retention days | `LLM_LOG_RETENTION_DAYS` |
| `LLM_ROUTER_HEALTH_CHECK_INTERVAL` | `30` | Node health check interval (seconds) | `HEALTH_CHECK_INTERVAL` |
| `LLM_ROUTER_NODE_TIMEOUT` | `60` | Node request timeout (seconds) | `NODE_TIMEOUT` |
| `LLM_ROUTER_LOAD_BALANCER_MODE` | `auto` | Load balancer mode (`auto` / `metrics`) | `LOAD_BALANCER_MODE` |
| `ROUTER_MAX_WAITERS` | `1024` | Admission queue limit | mainly for tests |
| `LLM_QUANTIZE_BIN` | - | Path to `llama-quantize` binary | optional |

Cloud / external services:

| Variable | Default | Description | Notes |
|----------|---------|-------------|-------|
| `OPENAI_API_KEY` | - | API key for `openai:` models | required |
| `OPENAI_BASE_URL` | `https://api.openai.com` | Override OpenAI base URL | optional |
| `GOOGLE_API_KEY` | - | API key for `google:` models | required |
| `GOOGLE_API_BASE_URL` | `https://generativelanguage.googleapis.com/v1beta` | Override Google base URL | optional |
| `ANTHROPIC_API_KEY` | - | API key for `anthropic:` models | required |
| `ANTHROPIC_API_BASE_URL` | `https://api.anthropic.com` | Override Anthropic base URL | optional |
| `HF_TOKEN` | - | Hugging Face token for model pulls | optional |
| `LLM_ROUTER_API_KEY` | - | API key used by e2e tests/clients | client/test use |

#### Node (llm-node)

| Variable | Default | Description | Legacy / Notes |
|----------|---------|-------------|----------------|
| `LLM_ROUTER_URL` | `http://127.0.0.1:8080` | Router URL to register with | - |
| `LLM_NODE_API_KEY` | - | API key for node registration / model registry download | scope: `node:register` |
| `LLM_NODE_PORT` | `11435` | Node listen port | - |
| `LLM_NODE_MODELS_DIR` | `~/.llm-router/models` | Model storage directory | `LLM_MODELS_DIR` |
| `LLM_NODE_BIND_ADDRESS` | `0.0.0.0` | Bind address | `LLM_BIND_ADDRESS` |
| `LLM_NODE_IP` | auto-detected | Node IP reported to router | - |
| `LLM_NODE_HEARTBEAT_SECS` | `10` | Heartbeat interval (seconds) | `LLM_HEARTBEAT_SECS` |
| `LLM_NODE_LOG_LEVEL` | `info` | Log level | `LLM_LOG_LEVEL`, `LOG_LEVEL` |
| `LLM_NODE_LOG_DIR` | `~/.llm-router/logs` | Log directory | `LLM_LOG_DIR` |
| `LLM_NODE_LOG_RETENTION_DAYS` | `7` | Log retention days | `LLM_LOG_RETENTION_DAYS` |
| `LLM_NODE_CONFIG` | `~/.llm-router/config.json` | Path to node config file | - |
| `LLM_MODEL_IDLE_TIMEOUT` | unset | Seconds before unloading idle models | enabled when set |
| `LLM_MAX_LOADED_MODELS` | unset | Cap on simultaneously loaded models | enabled when set |
| `LLM_MAX_MEMORY_BYTES` | unset | Max memory for loaded models | enabled when set |

**Backward compatibility**: Legacy names are read for fallback but are deprecated—prefer the new names above.

## Troubleshooting

### GPU not found at startup
- Check: `nvidia-smi` or `CUDA_VISIBLE_DEVICES`
- Disable via env var: Node side `LLM_ALLOW_NO_GPU=true` (disabled by default)
- If it still fails, check for NVML library presence

### Cloud models return 401/400
- Check if `OPENAI_API_KEY` / `GOOGLE_API_KEY` / `ANTHROPIC_API_KEY` are set on the router side
- If `*_key_present` is false in Dashboard `/v0/dashboard/stats`, it's not set
- Models without prefixes are routed locally, so do not add a prefix if you don't have cloud keys

### Port conflict
- Router: Change `LLM_ROUTER_PORT` (e.g., `LLM_ROUTER_PORT=18080`)
- Node: Change `LLM_NODE_PORT` or use `--port`

### SQLite file creation failed
- Check write permissions for the directory in `LLM_ROUTER_DATABASE_URL` path
- On Windows, check if the path contains spaces

### Dashboard does not appear
- Clear browser cache
- Try `cargo clean` -> `cargo run` to check if bundled static files are broken
- Check static delivery settings for `/dashboard/*` if using a reverse proxy

### OpenAI compatible API returns 503 / Model not registered
- Returns 503 if all nodes are `initializing`. Wait for node model load or check status at `/v0/dashboard/nodes`
- If specified model does not exist locally, wait for node to auto-pull

### Too many / too few logs
- Control via `LLM_ROUTER_LOG_LEVEL` or `RUST_LOG` env var (e.g., `LLM_ROUTER_LOG_LEVEL=info` or `RUST_LOG=or_router=debug`)
- Node logs use `spdlog`. Structured logs can be configured via `tracing_subscriber`

## Development

### Commit Hooks

Install the JavaScript tooling once per clone to enable Husky-managed commit hooks:

```bash
pnpm install
```

- Runs the `prepare` script and configures Husky's Git hook directory.
- Adds a `commit-msg` hook that executes `commitlint --edit "$1"` so invalid messages fail locally instead of in CI.
- Use `pnpm run lint:commits` to lint a range manually (defaults to `origin/main..HEAD`).

### Running Tests

```bash
# Full quality gate (fmt, clippy, workspace tests, specify checks, markdownlint, OpenAI proxy)
make quality-checks

# (Optional) Run only the OpenAI-compatible proxy regression suite
make openai-tests
```

### PoCs

- gpt-oss (auto): `make poc-gptoss`
- gpt-oss (macOS / Metal): `make poc-gptoss-metal`
- gpt-oss (Linux / CUDA via GGUF): `make poc-gptoss-cuda`
  - Logs/workdir are created under `tmp/poc-gptoss-cuda/` (router/node logs, request JSON, etc.)

### Spec-Driven Development

This project follows Spec-Driven Development:

1. `/speckit.specify` - Create feature specification
2. `/speckit.plan` - Create implementation plan
3. `/speckit.tasks` - Break down into tasks
4. Execute tasks (strict TDD cycle)

See [CLAUDE.md](./CLAUDE.md) for details.

### Claude Code Worktree Hooks

This project uses Claude Code PreToolUse Hooks to enforce Worktree environment
boundaries and prevent accidental operations that could disrupt the development workflow.

**Features:**

- **Git Branch Protection**: Blocks `git checkout`, `git switch`, `git worktree`
commands to prevent branch switching
- **Directory Navigation Control**: Blocks `cd` commands that would move outside
the Worktree boundary
- **Smart Allow Lists**: Permits read-only operations like `git branch --list`
- **Fast Execution**: Average response time < 50ms (target: < 100ms)

**Installation & Configuration:**

For detailed setup instructions, manual testing examples, and troubleshooting, see:

- [Quickstart Guide](./specs/SPEC-dc648675/quickstart.md) - Step-by-step setup
and verification
- [Feature Specification](./specs/SPEC-dc648675/spec.md) - Requirements and
acceptance criteria
- [Implementation Plan](./specs/SPEC-dc648675/plan.md) - Technical design and
architecture
- [Performance Report](./specs/SPEC-dc648675/performance.md) - Benchmark results

**Running Hook Tests:**

```bash
# Run all Hook contract tests (13 test cases)
make test-hooks

# Or run manually with Bats
npx bats tests/hooks/test-block-git-branch-ops.bats tests/hooks/test-block-cd-command.bats

# Run performance benchmark
tests/hooks/benchmark-hooks.sh
```

**Automated Testing:**

Hook tests are automatically executed in CI/CD:

- GitHub Actions: `.github/workflows/test-hooks.yml` (standalone)
- Quality Checks: `.github/workflows/quality-checks.yml` (integrated)
- Makefile: `make quality-checks` includes `test-hooks` target

## Request History

LLM Router automatically logs all requests and responses for debugging,
auditing, and analysis purposes.

### Features

- **Complete Request/Response Logging**: Captures full request bodies,
response bodies, and metadata
- **Automatic Retention**: Keeps history for 7 days with automatic cleanup
- **Web Dashboard**: View, filter, and search request history through the
web interface
- **Export Capabilities**: Export history as CSV
- **Filtering Options**: Filter by model, node, status, and time range

### Accessing Request History

#### Via Web Dashboard

1. Open the router dashboard: `http://localhost:8080/dashboard`
2. Navigate to the "Request History" section
3. Use filters to narrow down specific requests
4. Click on any request to view full details including request/response bodies

#### Via API

**List Request History:**
```bash
GET /v0/dashboard/request-responses?page=1&per_page=50
```

**Get Request Details:**
```bash
GET /v0/dashboard/request-responses/{id}
```

**Export History:**
```bash
GET /v0/dashboard/request-responses/export
```

### Storage

Request history is stored in JSON format at:
- Linux/macOS: `~/.llm-router/request_history.json`
- Windows: `%USERPROFILE%\.llm-router\request_history.json`

The file is automatically managed with:
- Atomic writes (temp file + rename) to prevent corruption
- File locking to handle concurrent access
- Automatic cleanup of records older than 7 days

## API Specification

### Router API

#### Authentication Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/auth/login` | User authentication, JWT token issuance | None |
| POST | `/v0/auth/logout` | Logout | JWT |
| GET | `/v0/auth/me` | Get authenticated user info | JWT |

#### Roles & API Key Scopes

**User roles (JWT):**

| Role | Capabilities |
|------|--------------|
| `admin` | Full access to `/v0` management APIs |
| `viewer` | Can authenticate and access `/v0/auth/*` only |

**API key scopes:**

| Scope | Grants |
|-------|--------|
| `node:register` | Node registration + health + model sync (`POST /v0/nodes`, `POST /v0/health`, `GET /v0/models`, `GET /v0/models/registry/*`) |
| `api:inference` | OpenAI-compatible inference APIs (`/v1/*` except `/v1/models` via node token) |
| `admin:*` | All management APIs (`/v0/users`, `/v0/api-keys`, `/v0/models/*`, `/v0/nodes/*`, `/v0/dashboard/*`, `/v0/metrics/*`) |

Debug builds accept `sk_debug`, `sk_debug_node`, `sk_debug_api`, `sk_debug_admin` (see `docs/authentication.md`).

#### User Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/users` | List users | JWT+Admin or API key (admin:*) |
| POST | `/v0/users` | Create user | JWT+Admin or API key (admin:*) |
| PUT | `/v0/users/:id` | Update user | JWT+Admin or API key (admin:*) |
| DELETE | `/v0/users/:id` | Delete user | JWT+Admin or API key (admin:*) |

#### API Key Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/api-keys` | List API keys | JWT+Admin or API key (admin:*) |
| POST | `/v0/api-keys` | Create API key | JWT+Admin or API key (admin:*) |
| PUT | `/v0/api-keys/:id` | Update API key | JWT+Admin or API key (admin:*) |
| DELETE | `/v0/api-keys/:id` | Delete API key | JWT+Admin or API key (admin:*) |

#### Node Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/nodes` | Register node (GPU required) | API key (node:register) |
| GET | `/v0/nodes` | List nodes | JWT+Admin or API key (admin:*) |
| DELETE | `/v0/nodes/:node_id` | Delete node | JWT+Admin or API key (admin:*) |
| POST | `/v0/nodes/:node_id/disconnect` | Force node offline | JWT+Admin or API key (admin:*) |
| PUT | `/v0/nodes/:node_id/settings` | Update node settings | JWT+Admin or API key (admin:*) |
| GET | `/v0/nodes/metrics` | List node metrics | JWT+Admin or API key (admin:*) |
| GET | `/v0/metrics/summary` | System statistics summary | JWT+Admin or API key (admin:*) |

#### Health Check Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/health` | Receive health check from node | Node Token + API key (node:register) |

#### OpenAI-Compatible Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v1/chat/completions` | Chat completions API | API Key |
| POST | `/v1/completions` | Text completions API | API Key |
| POST | `/v1/embeddings` | Embeddings API | API Key |
| GET | `/v1/models` | List models (Azure-style capabilities) | API Key / Node Token |
| GET | `/v1/models/:model_id` | Get specific model info | API Key / Node Token |

#### Model Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/models` | List registered models (node sync) | API key (node:register or admin:*) |
| POST | `/v0/models/register` | Register model (HF) | JWT+Admin or API key (admin:*) |
| DELETE | `/v0/models/*model_name` | Delete model | JWT+Admin or API key (admin:*) |
| POST | `/v0/models/discover-gguf` | Discover GGUF models | JWT+Admin or API key (admin:*) |
| GET | `/v0/models/registry/:model_name/manifest.json` | Get model manifest (file list) | API key (node:register or admin:*) |
| GET | `/v0/models/registry/:model_name/files/:file_name` | Serve model file | API key (node:register or admin:*) |
| GET | `/v0/models/blob/:model_name` | Legacy single-file model download (GGUF) | API key (node:register or admin:*) |

#### Dashboard Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/dashboard/nodes` | Node info list | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/stats` | System statistics | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/request-history` | Request history | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/overview` | Dashboard overview | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/metrics/:node_id` | Node metrics history | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/request-responses` | Request/response list | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/request-responses/:id` | Request/response details | JWT+Admin or API key (admin:*) |
| GET | `/v0/dashboard/request-responses/export` | Export request/responses | JWT+Admin or API key (admin:*) |

#### Log Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/dashboard/logs/router` | Router logs | JWT+Admin or API key (admin:*) |
| GET | `/v0/nodes/:node_id/logs` | Node logs | JWT+Admin or API key (admin:*) |

#### Static Files & Metrics

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/dashboard` | Dashboard UI | None |
| GET | `/dashboard/*path` | Dashboard static files | None |
| GET | `/playground` | Chat Playground UI | None |
| GET | `/playground/*path` | Playground static files | None |
| GET | `/v0/metrics/cloud` | Prometheus metrics export | JWT+Admin or API key (admin:*) |

### Node API (C++)

#### OpenAI-Compatible Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | Chat completions (streaming supported) |
| POST | `/v1/completions` | Text completions |
| POST | `/v1/embeddings` | Embeddings generation |

#### Node Management Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/startup` | Startup status check |
| GET | `/metrics` | Metrics (JSON format) |
| GET | `/metrics/prom` | Prometheus metrics |
| GET | `/v0/logs?tail=200` | Tail node logs (JSON) |
| GET | `/log/level` | Get current log level |
| POST | `/log/level` | Change log level |
| GET | `/internal-error` | Intentional error (debug) |

### Request/Response Examples

#### POST /v0/nodes

Register a node.

**Request:**

**Headers:** `Authorization: Bearer <node_api_key>`

```json
{
  "machine_name": "my-machine",
  "ip_address": "192.168.1.100",
  "runtime_version": "0.1.0",
  "runtime_port": 11434,
  "gpu_available": true,
  "gpu_devices": [
    { "model": "NVIDIA RTX 4090", "count": 2 }
  ]
}
```

**Response:**

```json
{
  "node_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "registered",
  "node_api_port": 11435,
  "node_token": "nt_xxx"
}
```

#### GET /v1/models

List available models with Azure OpenAI-style capabilities.

**Response:**

```json
{
  "object": "list",
  "data": [
    {
      "id": "meta-llama/llama-3.1-8b",
      "object": "model",
      "created": 0,
      "owned_by": "router",
      "capabilities": {
        "chat_completion": true,
        "completion": true,
        "embeddings": false,
        "fine_tune": false,
        "inference": true,
        "text_to_speech": false,
        "speech_to_text": false,
        "image_generation": false
      },
      "lifecycle_status": "registered",
      "download_progress": null,
      "ready": true
    }
  ]
}
```

> **Note**: `capabilities` uses Azure OpenAI-style boolean object format.
> `lifecycle_status`, `download_progress`, and `ready` are router extensions.

#### POST /v1/chat/completions

Chat completions API (OpenAI-compatible).

**Request:**

```json
{
  "model": "gpt-oss-20b",
  "messages": [{"role": "user", "content": "Hello!"}],
  "stream": false
}
```

**Response:**

```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "choices": [{
    "index": 0,
    "message": {"role": "assistant", "content": "Hello! How can I help you?"},
    "finish_reason": "stop"
  }]
}
```

> **Important**: LLM Router only supports OpenAI-compatible response format.

## License

MIT License

## Contributing

Issues and Pull Requests are welcome.

For detailed development guidelines, see [CLAUDE.md](./CLAUDE.md).
### Cloud model prefixes (OpenAI-compatible API)

- Supported prefixes: `openai:`, `google:`, `anthropic:` (alias `ahtnorpic:`)
- Usage: set `model` to e.g. `openai:gpt-4o`, `google:gemini-1.5-pro`, `anthropic:claude-3-opus`
- Environment variables:
  - `OPENAI_API_KEY` (required), `OPENAI_BASE_URL` (optional, default `https://api.openai.com`)
  - `GOOGLE_API_KEY` (required), `GOOGLE_API_BASE_URL` (optional, default `https://generativelanguage.googleapis.com/v1beta`)
  - `ANTHROPIC_API_KEY` (required), `ANTHROPIC_API_BASE_URL` (optional, default `https://api.anthropic.com`)
- Behavior: prefix is stripped before forwarding; responses remain OpenAI-compatible. Streaming is passthrough as SSE.
- Metrics: `/v0/metrics/cloud` exports Prometheus text with per-provider counters (`cloud_requests_total{provider,status}`) and latency histogram (`cloud_request_latency_seconds{provider}`).
