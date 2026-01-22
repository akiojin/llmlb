# LLM Load Balancer

A centralized management system for coordinating LLM inference runtimes across multiple machines

English | [日本語](./README.ja.md)

## Overview

LLM Load Balancer is a powerful centralized system that provides unified management and a single API endpoint for multiple LLM inference runtimes running across different machines. It features intelligent load balancing, automatic failure detection, real-time monitoring capabilities, and seamless integration for enhanced scalability.

### Vision

LLM Load Balancer is designed to serve three primary use cases:

1. **Private LLM Server** - For individuals and small teams who want to run their own LLM infrastructure with full control over their data and models
2. **Enterprise Gateway** - For organizations requiring centralized management, access control, and monitoring of LLM resources across departments
3. **Cloud Provider Integration** - Seamlessly route requests to OpenAI, Google, or Anthropic APIs through the same unified endpoint

### Multi-Engine Architecture

LLM Load Balancer supports a pluggable multi-engine architecture:

| Engine | Status | Models | Hardware |
|--------|--------|--------|----------|
| **llama.cpp** | Production | GGUF format (LLaMA, Mistral, etc.) | CPU, CUDA, Metal |
| **GPT-OSS** | Production (Metal/CUDA) | Safetensors (official GPU artifacts) | Apple Silicon, Windows |
| **Whisper** | Production | Speech-to-Text (ASR) | CPU, CUDA, Metal |
| **Stable Diffusion** | Production | Image Generation | CUDA, Metal |
| **Nemotron** | Validation | Safetensors format | CUDA |

**Engine Selection Policy**:

- **Models with GGUF available** → Use llama.cpp (Metal/CUDA ready)
- **Models with safetensors only** → Implement built-in engine (Metal/CUDA support required)

### Safetensors Architecture Support (Implementation-Aligned)

| Architecture | Status | Notes |
|-------------|--------|-------|
| **gpt-oss (MoE + MXFP4)** | Implemented | Uses `mlp.router.*` and `mlp.experts.*_(blocks\|scales\|bias)` with MoE forward |
| **nemotron3 (Mamba-Transformer MoE)** | Staged (not wired) | Not connected to the forward pass yet |

See `specs/SPEC-69549000/spec.md` for the authoritative list and updates.

### GGUF Architecture Coverage (llama.cpp, Examples)

These are representative examples of model families supported via GGUF/llama.cpp. This list is
non-exhaustive and follows upstream llama.cpp compatibility.

| Architecture | Example models | Notes |
|-------------|----------------|-------|
| **llama** | Llama 3.1, Llama 3.2, Llama 3.3, DeepSeek-R1-Distill-Llama | Meta Llama family |
| **mistral** | Mistral, Mistral-Nemo | Mistral AI family |
| **gemma** | Gemma3, Gemma3n, Gemma3-QAT, FunctionGemma, EmbeddingGemma | Google Gemma family |
| **qwen** | Qwen2.5, Qwen3, QwQ, Qwen3-VL, Qwen3-Coder, Qwen3-Embedding, Qwen3-Reranker | Alibaba Qwen family |
| **phi** | Phi-4 | Microsoft Phi family |
| **nemotron** | Nemotron | NVIDIA Nemotron family |
| **deepseek** | DeepSeek-V3.2, DeepCoder-Preview | DeepSeek family |
| **gpt-oss** | GPT-OSS, GPT-OSS-Safeguard | OpenAI GPT-OSS family |
| **granite** | Granite-4.0-H-Small/Tiny/Micro, Granite-Docling | IBM Granite family |
| **smollm** | SmolLM2, SmolLM3, SmolVLM | HuggingFace SmolLM family |
| **kimi** | Kimi-K2 | Moonshot Kimi family |
| **moondream** | Moondream2 | Moondream vision family |
| **devstral** | Devstral-Small | Mistral derivative (coding-focused) |
| **magistral** | Magistral-Small-3.2 | Mistral derivative (multimodal) |

### Multimodal Support

Beyond text generation, LLM Load Balancer provides OpenAI-compatible APIs for:

- **Text-to-Speech (TTS)**: `/v1/audio/speech` - Generate natural speech from text
- **Speech-to-Text (ASR)**: `/v1/audio/transcriptions` - Transcribe audio to text
- **Image Generation**: `/v1/images/generations` - Generate images from text prompts
- **Image Understanding**: `/v1/chat/completions` - Analyze images via `image_url` content parts (Vision models)

Text generation should use the **Responses API** (`/v1/responses`) by default. Chat Completions remains
available for compatibility.

## Key Features

- **Unified API Endpoint**: Access multiple LLM runtime instances through a single URL
- **Automatic Load Balancing**: Latency-based request distribution across available endpoints
- **Endpoint Management**: Centralized management of Ollama, vLLM, xLLM and other OpenAI-compatible servers
- **Model Sync**: Automatic model discovery via `GET /v1/models` from registered endpoints
- **Automatic Failure Detection**: Detect offline endpoints and exclude them from routing
- **Real-time Monitoring**: Comprehensive visualization of endpoint states and performance metrics via web dashboard
- **Request History Tracking**: Complete request/response logging with 7-day retention
- **WebUI Management**: Manage endpoints, monitoring, and control through browser-based dashboard
- **Cross-Platform Support**: Works on Windows 10+, macOS 12+, and Linux
- **GPU-Aware Routing**: Intelligent request routing based on GPU capabilities and availability
- **Cloud Model Prefixes**: Add `openai:` `google:` or `anthropic:` in the model name to proxy to the corresponding cloud provider while keeping the same OpenAI-compatible endpoint.

## MCP Server for LLM Assistants

LLM assistants (like Claude Code) can interact with LLM Load Balancer through a dedicated
MCP server. This is the recommended approach over using Bash with curl commands
directly.
The MCP server is installed and run with npm/npx; the repository root uses pnpm
for workspace tasks.

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
npm install -g @llmlb/mcp-server
# or
npx @llmlb/mcp-server
```

### Configuration (.mcp.json)

```json
{
  "mcpServers": {
    "llmlb": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@llmlb/mcp-server"],
      "env": {
        "LLMLB_URL": "http://localhost:32768",
        "LLMLB_API_KEY": "sk_your_api_key"
      }
    }
  }
}
```

For detailed documentation, see [mcp-server/README.md](./mcp-server/README.md).

## Quick Start

### Router (llmlb)

```bash
# Build
cargo build --release -p llmlb

# Run
./target/release/llmlb
# Default: http://0.0.0.0:32768

# Access dashboard
# Open http://localhost:32768/dashboard in browser
```

**Environment Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LLMLB_HOST` | `0.0.0.0` | Bind address |
| `LLMLB_PORT` | `32768` | Listen port |
| `LLMLB_LOG_LEVEL` | `info` | Log level |
| `LLMLB_JWT_SECRET` | (auto-generated) | JWT signing secret |
| `LLMLB_ADMIN_USERNAME` | `admin` | Initial admin username |
| `LLMLB_ADMIN_PASSWORD` | (required) | Initial admin password |

**Backward compatibility:** Legacy env var names (`ROUTER_PORT` etc.) are supported but deprecated.

**System Tray (Windows/macOS only):**

On Windows 10+ and macOS 12+, the router displays a system tray icon.
Double-click to open the dashboard. Docker/Linux runs as a headless CLI process.

### CLI Reference

The router CLI currently exposes only basic flags (`--help`, `--version`).
Day-to-day management is done via the Dashboard UI (`/dashboard`) or the HTTP APIs.

### Runtime (C++)

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
npm run build:xllm

# Build (Linux / CUDA)
npm run build:xllm:cuda

# Run
npm run start:xllm

# Or manually:
# cd xllm && cmake -B build -S . && cmake --build build --config Release
# # Linux / CUDA:
# # cd xllm && cmake -B build -S . -DBUILD_WITH_CUDA=ON && cmake --build build --config Release
# LLMLB_URL=http://localhost:32768 ./xllm/build/xllm
```

**Environment Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `LLMLB_URL` | `http://127.0.0.1:32768` | Router URL to register with |
| `LLM_RUNTIME_PORT` | `32769` | Runtime listen port |
| `LLM_RUNTIME_MODELS_DIR` | `~/.llmlb/models` | Model storage directory |
| `LLM_RUNTIME_ORIGIN_ALLOWLIST` | `huggingface.co/*,cdn-lfs.huggingface.co/*` | Allowlist for direct origin downloads (comma-separated) |
| `LLM_RUNTIME_BIND_ADDRESS` | `0.0.0.0` | Bind address |
| `LLM_RUNTIME_HEARTBEAT_SECS` | `10` | Heartbeat interval (seconds) |
| `LLM_RUNTIME_LOG_LEVEL` | `info` | Log level |

**Backward compatibility:** Legacy env var names (`LLM_MODELS_DIR` etc.) are supported but deprecated.

**Docker:**

```bash
# Build
docker build --build-arg CUDA=cpu -t xllm:latest xllm/

# Run
docker run --rm -p 32769:32769 \
  -e LLMLB_URL=http://host.docker.internal:32768 \
  xllm:latest
```

## Load Balancing

LLM Load Balancer supports multiple load balancing strategies to optimize request distribution across runtimes.

### Strategies

#### 1. Metrics-Based Load Balancing (Recommended)

Selects runtimes based on real-time metrics (CPU usage, memory usage, active requests). This intelligent mode provides optimal performance by dynamically routing requests to the least loaded runtime, ensuring efficient resource utilization.

**Configuration:**
```bash
# Enable metrics-based load balancing
LLMLB_LOAD_BALANCER_MODE=metrics cargo run -p llmlb
```

**Load Score Calculation:**
```
score = cpu_usage + memory_usage + (active_requests × 10)
```

The runtime with the **lowest score** is selected. If all runtimes have CPU usage > 80%, the system automatically falls back to round-robin.

**Example:**
- Runtime A: CPU 20%, Memory 30%, Active 1 → Score = 60 ✓ Selected
- Runtime B: CPU 70%, Memory 50%, Active 5 → Score = 170

#### 2. Advanced Load Balancing (Default)

Combines multiple factors including response time, active requests, and CPU usage to provide sophisticated runtime selection with adaptive performance optimization.

**Configuration:**
```bash
# Use default advanced load balancing (or omit LOAD_BALANCER_MODE)
LLMLB_LOAD_BALANCER_MODE=auto cargo run -p llmlb
```

### Health / Metrics API

Runtimes report health + metrics to the Router for runtime status and load balancing decisions.

**Endpoint:** `POST /v0/health` (requires `X-Runtime-Token` + API key with `runtime`)

**Headers:**
- `Authorization: Bearer <api_key>`
- `X-Runtime-Token: <runtime_token>`

**Request:**
```json
{
  "runtime_id": "550e8400-e29b-41d4-a716-446655440000",
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

LLM Load Balancer coordinates local llama.cpp runtimes and optionally proxies to cloud LLM providers via model prefixes.

### Components
- **Router (Rust)**: Receives OpenAI-compatible traffic, chooses a path, and proxies requests. Exposes dashboard, metrics, and admin APIs.
- **Local Runtimes (C++ / llama.cpp)**: Serve GGUF models; register and send heartbeats to the router.
- **Cloud Proxy**: When a model name starts with `openai:` `google:` or `anthropic:` the router forwards to the corresponding cloud API.
- **Storage**: SQLite for router metadata; model files live on each runtime.
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
  └─ No prefix → Scheduler → Local Runtime
                       └─ llama.cpp inference → Response
```

### Communication Flow (Proxy Pattern)

LLM Load Balancer uses a **Proxy Pattern** - clients only need to know the Router URL.

#### Traditional Method (Without Router)
```bash
# Direct access to each runtime API (default: runtime_port=32769)
curl http://machine1:32769/v1/responses -d '...'
curl http://machine2:32769/v1/responses -d '...'
curl http://machine3:32769/v1/responses -d '...'
```

#### With Router (Proxy)
```bash
# Unified access to Router - automatic routing to the optimal runtime
curl http://router:32768/v1/responses -d '...'
curl http://router:32768/v1/responses -d '...'
curl http://router:32768/v1/responses -d '...'
```

**Detailed Request Flow:**

1. **Client → Router**
   ```
   POST http://router:32768/v1/responses
   Content-Type: application/json

   {"model": "llama2", "input": "Hello!"}
   ```

2. **Router Internal Processing**
   - Select optimal runtime (Load Balancing)
   - Forward request to selected runtime via HTTP client

3. **Router → Runtime (Internal Communication)**
   ```
   POST http://runtime1:32769/v1/responses
   Content-Type: application/json

   {"model": "llama2", "input": "Hello!"}
   ```

4. **Runtime Local Processing**
   - Runtime loads model on-demand (from local cache or router-provided source)
   - Runtime runs llama.cpp inference and returns an OpenAI-compatible response

5. **Router → Client (Return Response)**
   ```json
  {
    "id": "resp_123",
    "object": "response",
    "output": [
      {
        "type": "message",
        "role": "assistant",
        "content": [
          { "type": "output_text", "text": "Hello!" }
        ]
      }
    ]
  }
   ```

> **Note**: LLM Load Balancer supports OpenAI-compatible APIs and **recommends** the
> Responses API (`/v1/responses`). Chat Completions remains available for
> compatibility.

**From Client's Perspective**:
- Router appears as the only OpenAI-compatible API server
- No need to be aware of multiple internal runtimes
- Complete with a single HTTP request

### Model Sync (No Push Distribution)

- The router never pushes models to runtimes.
- Runtimes resolve models on-demand in this order:
  - local cache (`LLM_RUNTIME_MODELS_DIR`)
  - allowlisted origin download (Hugging Face, etc.; configure via `LLM_RUNTIME_ORIGIN_ALLOWLIST`)
  - manifest-based selection from the router (`GET /v0/models/registry/:model_name/manifest.json`)

### Scheduling & Health
- Runtimes register via `/v0/runtimes`; router rejects runtimes without GPUs by default.
- Heartbeats carry CPU/GPU/memory metrics used for load balancing.
- Dashboard surfaces `*_key_present` flags so operators see which cloud keys are configured.

### Benefits of Proxy Pattern

1. **Unified Endpoint**
   - Clients only need to know the Router URL
   - No need to know each runtime location

2. **Transparent Load Balancing**
   - Router automatically selects the optimal runtime
   - Clients benefit from load distribution without awareness

3. **Automatic Retry on Failure**
   - If Runtime1 fails → Router automatically tries Runtime2
   - No re-request needed from client

4. **Security**
   - Runtime IP addresses not exposed to clients
   - Only Router needs to be publicly accessible

5. **Scalability**
   - Adding runtimes automatically increases processing capacity
   - No changes needed on client side

## Project Structure

```
llmlb/
├── common/              # Shared library (types, protocol, errors)
├── router/              # Rust router (HTTP APIs, dashboard, proxy)
├── xllm/                # C++ xLLM inference engine (llama.cpp, OpenAI-compatible /v1/*)
├── mcp-server/          # MCP server (for LLM assistants like Claude Code)
└── specs/               # Specifications (Spec-Driven Development)
```

## Dashboard

The dashboard is served by the router at `/dashboard`.
Use it to monitor endpoints, view request history, inspect logs, and manage models.

### Quick usage

1. Start the router:
   ```bash
   cargo run -p llmlb
   ```
1. Open:
   ```text
   http://localhost:32768/dashboard
   ```

## Endpoint Management

The router centrally manages external inference servers (Ollama, vLLM, xLLM, etc.) as "endpoints".

### Supported Endpoints

| Type | Description | Health Check |
|------|-------------|--------------|
| **xLLM** | In-house inference server (llama.cpp/whisper.cpp) | `GET /v1/models` |
| **Ollama** | Ollama server | `GET /v1/models` |
| **vLLM** | vLLM inference server | `GET /v1/models` |
| **OpenAI-compatible** | Other OpenAI-compatible APIs | `GET /v1/models` |

### Registration via Dashboard

1. Dashboard → Sidebar "Endpoints"
2. Click "New Endpoint"
3. Enter name and base URL (e.g., `http://192.168.1.100:11434`)
4. "Connection Test" → "Save"

### Registration via REST API

```bash
# Register endpoint
curl -X POST http://localhost:32768/v0/endpoints \
  -H "Authorization: Bearer sk_your_api_key" \
  -H "Content-Type: application/json" \
  -d '{"name": "Ollama Server A", "base_url": "http://192.168.1.100:11434"}'

# List endpoints
curl http://localhost:32768/v0/endpoints \
  -H "Authorization: Bearer sk_your_api_key"

# Sync models
curl -X POST http://localhost:32768/v0/endpoints/{id}/sync \
  -H "Authorization: Bearer sk_your_api_key"
```

### Status Transitions

- **pending**: Just registered (awaiting health check)
- **online**: Health check successful
- **offline**: Health check failed
- **error**: Connection error

For details, see [specs/SPEC-66555000/quickstart.md](./specs/SPEC-66555000/quickstart.md).

## Hugging Face registration (safetensors / GGUF)

- Optional env vars: set `HF_TOKEN` to raise Hugging Face rate limits; set `HF_BASE_URL` when using a mirror/cache.
- Web (recommended):
  - Dashboard → **Models** → **Register**
  - Choose `format`: `safetensors` (native engines) or `gguf` (llama.cpp fallback).
    - If the repo contains both `safetensors` and `.gguf`, `format` is required.
    - Safetensors text generation is available only when the safetensors.cpp engine is enabled
      (Metal/CUDA). Use `gguf` for GGUF-only models.
  - Enter a Hugging Face repo or file URL (e.g. `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16`).
  - For `format=gguf`:
    - Either specify an exact `.gguf` `filename`, or choose `gguf_policy` (`quality` / `memory` / `speed`)
      to auto-pick from GGUF siblings.
  - For `format=safetensors`:
    - The HF snapshot must include `config.json` and `tokenizer.json`.
    - Sharded weights must include an `.index.json`.
    - If official GPU artifacts are provided (for example `model.metal.bin`), they may be used as
      execution cache when supported. Otherwise, safetensors are used directly.
    - Windows requires CUDA builds (`BUILD_WITH_CUDA=ON`). DirectML is not supported.
  - Router stores **metadata + manifest only** (no binary download).
  - Model IDs are the Hugging Face repo ID (e.g. `org/model`).
  - `/v1/models` lists models including queued/caching/error with `lifecycle_status` + `download_progress`.
  - Runtimes pull models on-demand via the model registry endpoints:
    - `GET /v0/models/registry/:model_name/manifest.json`
    - `GET /v0/models/registry/:model_name/files/:file_name`
    - (Legacy) `GET /v0/models/blob/:model_name` for single-file GGUF.
- API:
  - `POST /v0/models/register` with `repo` and optional `filename`.
- `/v1/models` lists registered models; `ready` reflects runtime sync status.

## Installation

### Prerequisites

- Linux/macOS/Windows x64 (GPU recommended)
- Rust toolchain (stable) and cargo
- Docker (optional)
- CUDA Driver (for NVIDIA GPU) - see [CUDA Setup](#cuda-setup-nvidia-gpu)

### CUDA Setup (NVIDIA GPU)

For NVIDIA GPU acceleration, you need:

| Component | Build Environment | Runtime Environment |
|-----------|-------------------|---------------------|
| **CUDA Driver** | Required | Required |
| **CUDA Toolkit** | Required (for `nvcc`) | Not required |

#### Installing CUDA Driver

The CUDA Driver is typically installed with NVIDIA graphics drivers.

```bash
# Verify driver installation
nvidia-smi
```

If `nvidia-smi` shows your GPU, the driver is installed.

#### Installing CUDA Toolkit (Build Environment Only)

Required only for building the runtime with CUDA support (`BUILD_WITH_CUDA=ON`).

**Windows:**

1. Download from [CUDA Toolkit Downloads](https://developer.nvidia.com/cuda-downloads)
2. Select: Windows → x86_64 → 11 → exe (local)
3. Run the installer (Express installation recommended)
4. Verify: Open new terminal and run `nvcc --version`

**Linux (Ubuntu/Debian):**

```bash
# Add NVIDIA package repository
wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
sudo dpkg -i cuda-keyring_1.1-1_all.deb
sudo apt update

# Install CUDA Toolkit
sudo apt install cuda-toolkit-12-4

# Add to PATH (add to ~/.bashrc)
export PATH=/usr/local/cuda/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH

# Verify
nvcc --version
```

**Note:** Runtime environments (runtimes running pre-built binaries) only need the CUDA
Driver, not the full Toolkit.

### 1) Build from Rust source (Recommended)
```bash
git clone https://github.com/akiojin/llmlb.git
cd llmlb
make quality-checks   # fmt/clippy/test/markdownlint
cargo build -p llmlb --release
```
Artifact: `target/release/llmlb`

### 2) Run with Docker
```bash
docker build -t llmlb:latest .
docker run --rm -p 32768:32768 --gpus all \
  -e OPENAI_API_KEY=... \
  llmlb:latest
```
If not using GPU, remove `--gpus all` or set `CUDA_VISIBLE_DEVICES=""`.

### 3) C++ Runtime Build
See [Runtime (C++)](#runtime-c) section in Quick Start.

### Requirements

- **Router**: Rust toolchain (stable)
- **Runtime**: CMake + a C++ toolchain, and a supported GPU (NVIDIA / AMD / Apple Silicon)

## Usage

### Basic Usage

1. **Start Router**
   ```bash
   ./target/release/llmlb
   # Default: http://0.0.0.0:32768
   ```

2. **Start Runtimes on Multiple Machines**
   ```bash
   # Machine 1
   LLMLB_URL=http://router:32768 \
   # Replace with your actual API key (scope: runtime)
   LLM_RUNTIME_API_KEY=sk_your_runtime_register_key \
   ./xllm/build/xllm

   # Machine 2
   LLMLB_URL=http://router:32768 \
   # Replace with your actual API key (scope: runtime)
   LLM_RUNTIME_API_KEY=sk_your_runtime_register_key \
   ./xllm/build/xllm
   ```

3. **Send Inference Requests to Router (OpenAI-compatible, Responses API recommended)**
   ```bash
   curl http://router:32768/v1/responses \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer sk_your_api_key" \
     -d '{
       "model": "gpt-oss-20b",
       "input": "Hello!"
     }'
   ```

   **Image generation example**
   ```bash
   curl http://router:32768/v1/images/generations \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer sk_your_api_key" \
     -d '{
       "model": "stable-diffusion/v1-5-pruned-emaonly.safetensors",
       "prompt": "A white cat sitting on a windowsill",
       "size": "512x512",
       "n": 1,
       "response_format": "b64_json"
     }'
   ```

   **Image understanding example**
   ```bash
   curl http://router:32768/v1/chat/completions \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer sk_your_api_key" \
     -d '{
       "model": "llava-v1.5-7b",
       "messages": [
         {
           "role": "user",
           "content": [
             {"type": "text", "text": "What is in this image?"},
             {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="}}
           ]
         }
       ],
       "max_tokens": 300
     }'
   ```
4. **List Registered Runtimes**
   ```bash
   curl http://router:32768/v0/runtimes \
     # Replace with your actual API key (scope: admin)
     -H "Authorization: Bearer sk_your_admin_key"
   ```

### Environment Variables

#### Router (llmlb)

| Variable | Default | Description | Legacy / Notes |
|----------|---------|-------------|----------------|
| `LLMLB_HOST` | `0.0.0.0` | Bind address | `ROUTER_HOST` |
| `LLMLB_PORT` | `32768` | Listen port | `ROUTER_PORT` |
| `LLMLB_DATABASE_URL` | `sqlite:~/.llmlb/router.db` | Database URL | `DATABASE_URL` |
| `LLMLB_DATA_DIR` | `~/.llmlb` | Base directory for DB/log defaults | - |
| `LLMLB_JWT_SECRET` | (auto-generated) | JWT signing secret | `JWT_SECRET` |
| `LLMLB_ADMIN_USERNAME` | `admin` | Initial admin username | `ADMIN_USERNAME` |
| `LLMLB_ADMIN_PASSWORD` | (required, first run) | Initial admin password | `ADMIN_PASSWORD` |
| `LLMLB_LOG_LEVEL` | `info` | Log level (`EnvFilter`) | `LLM_LOG_LEVEL`, `RUST_LOG` |
| `LLMLB_LOG_DIR` | `~/.llmlb/logs` | Log directory | `LLM_LOG_DIR` (deprecated) |
| `LLMLB_LOG_RETENTION_DAYS` | `7` | Log retention days | `LLM_LOG_RETENTION_DAYS` |
| `LLMLB_HEALTH_CHECK_INTERVAL` | `30` | Runtime health check interval (seconds) | `HEALTH_CHECK_INTERVAL` |
| `LLMLB_NODE_TIMEOUT` | `60` | Runtime request timeout (seconds) | `NODE_TIMEOUT` |
| `LLMLB_LOAD_BALANCER_MODE` | `auto` | Load balancer mode (`auto` / `metrics`) | `LOAD_BALANCER_MODE` |
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
| `LLMLB_API_KEY` | - | API key used by e2e tests/clients | client/test use |

#### Runtime (llm-runtime)

| Variable | Default | Description | Legacy / Notes |
|----------|---------|-------------|----------------|
| `LLMLB_URL` | `http://127.0.0.1:32768` | Router URL to register with | - |
| `LLM_RUNTIME_API_KEY` | - | API key for runtime registration / model registry download | scope: `runtime` |
| `LLM_RUNTIME_PORT` | `32769` | Runtime listen port | - |
| `LLM_RUNTIME_MODELS_DIR` | `~/.llmlb/models` | Model storage directory | `LLM_MODELS_DIR` |
| `LLM_RUNTIME_ORIGIN_ALLOWLIST` | `huggingface.co/*,cdn-lfs.huggingface.co/*` | Allowlist for direct origin downloads (comma-separated) | `LLM_ORIGIN_ALLOWLIST` |
| `LLM_RUNTIME_BIND_ADDRESS` | `0.0.0.0` | Bind address | `LLM_BIND_ADDRESS` |
| `LLM_RUNTIME_IP` | auto-detected | Runtime IP reported to router | - |
| `LLM_RUNTIME_HEARTBEAT_SECS` | `10` | Heartbeat interval (seconds) | `LLM_HEARTBEAT_SECS` |
| `LLM_RUNTIME_LOG_LEVEL` | `info` | Log level | `LLM_LOG_LEVEL`, `LOG_LEVEL` |
| `LLM_RUNTIME_LOG_DIR` | `~/.llmlb/logs` | Log directory | `LLM_LOG_DIR` |
| `LLM_RUNTIME_LOG_RETENTION_DAYS` | `7` | Log retention days | `LLM_LOG_RETENTION_DAYS` |
| `LLM_RUNTIME_CONFIG` | `~/.llmlb/config.json` | Path to runtime config file | - |
| `LLM_MODEL_IDLE_TIMEOUT` | unset | Seconds before unloading idle models | enabled when set |
| `LLM_MAX_LOADED_MODELS` | unset | Cap on simultaneously loaded models | enabled when set |
| `LLM_MAX_MEMORY_BYTES` | unset | Max memory for loaded models | enabled when set |

**Backward compatibility**: Legacy names are read for fallback but are deprecated—prefer the new names above.

Note: Engine plugins were removed in favor of built-in managers. See `docs/migrations/plugin-to-manager.md`.

## Troubleshooting

### GPU not found at startup
- Check: `nvidia-smi` or `CUDA_VISIBLE_DEVICES`
- Disable via env var: Runtime side `LLM_ALLOW_NO_GPU=true` (disabled by default)
- If it still fails, check for NVML library presence

### Cloud models return 401/400
- Check if `OPENAI_API_KEY` / `GOOGLE_API_KEY` / `ANTHROPIC_API_KEY` are set on the router side
- If `*_key_present` is false in Dashboard `/v0/dashboard/stats`, it's not set
- Models without prefixes are routed locally, so do not add a prefix if you don't have cloud keys

### Port conflict
- Router: Change `LLMLB_PORT` (e.g., `LLMLB_PORT=18080`)
- Runtime: Change `LLM_RUNTIME_PORT` or use `--port`

### SQLite file creation failed
- Check write permissions for the directory in `LLMLB_DATABASE_URL` path
- On Windows, check if the path contains spaces

### Dashboard does not appear
- Clear browser cache
- Try `cargo clean` -> `cargo run` to check if bundled static files are broken
- Check static delivery settings for `/dashboard/*` if using a reverse proxy

### OpenAI compatible API returns 503 / Model not registered
- Returns 503 if all runtimes are `initializing`. Wait for runtime model load or check status at `/v0/dashboard/runtimes`
- If specified model does not exist locally, wait for runtime to auto-pull

### Too many / too few logs
- Control via `LLMLB_LOG_LEVEL` or `RUST_LOG` env var (e.g., `LLMLB_LOG_LEVEL=info` or `RUST_LOG=or_router=debug`)
- Runtime logs use `spdlog`. Structured logs can be configured via `tracing_subscriber`

## Development

For detailed development guidelines, testing procedures, and contribution workflow, see
[CLAUDE.md](./CLAUDE.md).

```bash
# Full quality gate
make quality-checks
```

### PoCs

- gpt-oss (auto): `make poc-gptoss`
- gpt-oss (macOS / Metal): `make poc-gptoss-metal`
- gpt-oss (Linux / CUDA via GGUF, experimental): `make poc-gptoss-cuda`
  - Logs/workdir are created under `tmp/poc-gptoss-cuda/` (router/runtime logs, request JSON, etc.)

Notes:
- gpt-oss-20b uses safetensors (index + shards + config/tokenizer) as the source of truth.
- GPU is required. Supported backends: macOS (Metal) and Windows (CUDA). Linux/CUDA is experimental.

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

LLM Load Balancer automatically logs all requests and responses for debugging,
auditing, and analysis purposes.

### Features

- **Complete Request/Response Logging**: Captures full request bodies,
response bodies, and metadata
- **Automatic Retention**: Keeps history for 7 days with automatic cleanup
- **Web Dashboard**: View, filter, and search request history through the
web interface
- **Export Capabilities**: Export history as CSV
- **Filtering Options**: Filter by model, runtime, status, and time range

### Accessing Request History

#### Via Web Dashboard

1. Open the router dashboard: `http://localhost:32768/dashboard`
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

Request history is stored in SQLite at:
- Linux/macOS: `~/.llmlb/router.db`
- Windows: `%USERPROFILE%\.llmlb\router.db`

Legacy `request_history.json` files (if present) are automatically imported on startup and renamed
to `.migrated`.

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
| `endpoints` | Endpoint management (`/v0/endpoints/*`) |
| `runtime` | Runtime registration + health + model sync (`POST /v0/runtimes`, `POST /v0/health`, `GET /v0/models`, `GET /v0/models/registry/:model_name/manifest.json`) - Legacy |
| `api` | OpenAI-compatible inference APIs (`/v1/*` except `/v1/models` via runtime token) |
| `admin` | All management APIs (`/v0/users`, `/v0/api-keys`, `/v0/models/*`, `/v0/runtimes/*`, `/v0/endpoints/*`, `/v0/dashboard/*`, `/v0/metrics/*`) |

Debug builds accept `sk_debug`, `sk_debug_runtime`, `sk_debug_api`, `sk_debug_admin` (see `docs/authentication.md`).

#### User Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/users` | List users | JWT+Admin or API key (admin) |
| POST | `/v0/users` | Create user | JWT+Admin or API key (admin) |
| PUT | `/v0/users/:id` | Update user | JWT+Admin or API key (admin) |
| DELETE | `/v0/users/:id` | Delete user | JWT+Admin or API key (admin) |

#### API Key Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/api-keys` | List API keys | JWT+Admin or API key (admin) |
| POST | `/v0/api-keys` | Create API key | JWT+Admin or API key (admin) |
| PUT | `/v0/api-keys/:id` | Update API key | JWT+Admin or API key (admin) |
| DELETE | `/v0/api-keys/:id` | Delete API key | JWT+Admin or API key (admin) |

#### Endpoint Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/endpoints` | Register endpoint | JWT+Admin or API key (admin) |
| GET | `/v0/endpoints` | List endpoints | JWT+Admin/Viewer or API key (admin/endpoints) |
| GET | `/v0/endpoints/:id` | Get endpoint details | JWT+Admin/Viewer or API key (admin/endpoints) |
| PUT | `/v0/endpoints/:id` | Update endpoint | JWT+Admin or API key (admin) |
| DELETE | `/v0/endpoints/:id` | Delete endpoint | JWT+Admin or API key (admin) |
| POST | `/v0/endpoints/:id/test` | Connection test | JWT+Admin or API key (admin) |
| POST | `/v0/endpoints/:id/sync` | Sync models | JWT+Admin or API key (admin) |

#### Runtime Management Endpoints (Legacy)

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/runtimes` | Register runtime (GPU required) | API key (runtime) |
| GET | `/v0/runtimes` | List runtimes | JWT+Admin or API key (admin) |
| DELETE | `/v0/runtimes/:runtime_id` | Delete runtime | JWT+Admin or API key (admin) |
| POST | `/v0/runtimes/:runtime_id/disconnect` | Force runtime offline | JWT+Admin or API key (admin) |
| PUT | `/v0/runtimes/:runtime_id/settings` | Update runtime settings | JWT+Admin or API key (admin) |
| GET | `/v0/runtimes/metrics` | List runtime metrics | JWT+Admin or API key (admin) |
| GET | `/v0/metrics/summary` | System statistics summary | JWT+Admin or API key (admin) |

#### Health Check Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v0/health` | Receive health check from runtime | Runtime Token + API key (runtime) |

#### OpenAI-Compatible Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| POST | `/v1/chat/completions` | Chat completions API | API Key |
| POST | `/v1/completions` | Text completions API | API Key |
| POST | `/v1/embeddings` | Embeddings API | API Key |
| GET | `/v1/models` | List models (Azure-style capabilities) | API Key / Runtime Token |
| GET | `/v1/models/:model_id` | Get specific model info | API Key / Runtime Token |

#### Model Management Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/models` | List registered models (runtime sync) | API key (runtime or admin) |
| POST | `/v0/models/register` | Register model (HF) | JWT+Admin or API key (admin) |
| DELETE | `/v0/models/*model_name` | Delete model | JWT+Admin or API key (admin) |
| GET | `/v0/models/registry/:model_name/manifest.json` | Get model manifest (file list) | API key (runtime or admin) |

#### Dashboard Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/dashboard/runtimes` | Runtime info list | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/stats` | System statistics | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/request-history` | Request history | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/overview` | Dashboard overview | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/metrics/:runtime_id` | Runtime metrics history | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/request-responses` | Request/response list | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/request-responses/:id` | Request/response details | JWT+Admin or API key (admin) |
| GET | `/v0/dashboard/request-responses/export` | Export request/responses | JWT+Admin or API key (admin) |

#### Log Endpoints

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/v0/dashboard/logs/router` | Router logs | JWT+Admin or API key (admin) |
| GET | `/v0/runtimes/:runtime_id/logs` | Runtime logs | JWT+Admin or API key (admin) |

#### Static Files & Metrics

| Method | Path | Description | Auth |
|--------|------|-------------|------|
| GET | `/dashboard` | Dashboard UI | None |
| GET | `/dashboard/*path` | Dashboard static files | None |
| GET | `/playground` | Chat Playground UI | None |
| GET | `/playground/*path` | Playground static files | None |
| GET | `/v0/metrics/cloud` | Prometheus metrics export | JWT+Admin or API key (admin) |

### Runtime API (C++)

#### OpenAI-Compatible Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | Chat completions (streaming supported) |
| POST | `/v1/completions` | Text completions |
| POST | `/v1/embeddings` | Embeddings generation |

#### Runtime Management Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/startup` | Startup status check |
| GET | `/metrics` | Metrics (JSON format) |
| GET | `/metrics/prom` | Prometheus metrics |
| GET | `/v0/logs?tail=200` | Tail runtime logs (JSON) |
| GET | `/log/level` | Get current log level |
| POST | `/log/level` | Change log level |
| GET | `/internal-error` | Intentional error (debug) |

### Request/Response Examples

#### POST /v0/runtimes

Register a runtime.

**Request:**

**Headers:** `Authorization: Bearer <runtime_api_key>`

```json
{
  "machine_name": "my-machine",
  "ip_address": "192.168.1.100",
  "runtime_version": "0.1.0",
  "runtime_port": 32768,
  "gpu_available": true,
  "gpu_devices": [
    { "model": "NVIDIA RTX 4090", "count": 2 }
  ]
}
```

**Response:**

```json
{
  "runtime_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "registered",
  "runtime_api_port": 32769,
  "runtime_token": "nt_xxx"
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
      "ready": true
    }
  ]
}
```

> **Note**: `capabilities` uses Azure OpenAI-style boolean object format.
> `ready` is a router extension derived from runtime sync state.

#### POST /v1/responses

Responses API (recommended).

**Request:**

```json
{
  "model": "gpt-oss-20b",
  "input": "Hello!"
}
```

**Response:**

```json
{
  "id": "resp_123",
  "object": "response",
  "output": [
    {
      "type": "message",
      "role": "assistant",
      "content": [
        { "type": "output_text", "text": "Hello! How can I help you?" }
      ]
    }
  ]
}
```

> **Compatibility**: `/v1/chat/completions` remains available for legacy clients.
> **Important**: LLM Load Balancer only supports OpenAI-compatible response format.

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
