# SPEC-d7feaa2c: aLLM Manager-based Runtime (Text/Audio/Image)

**ステータス**: 草案

Status: Draft (2026-01-19)

## Summary
This spec replaces the dynamic engine plugin model with in-process managers.
TextManager owns text/embedding runtimes (llama.cpp, safetensors.cpp). AudioManager
and ImageManager wrap whisper.cpp and stable-diffusion.cpp. Responses API is the
recommended interface; Chat Completions remains for compatibility.

## Goals
- Remove dynamic plugin loader, plugin ABI, and engine manifests.
- Establish clear manager boundaries per modality (Text/Audio/Image).
- Keep EngineRegistry as the internal selection mechanism for text engines.
- Align docs/tests with the manager approach.

## Non-goals
- External plugin ABI or hot reload.
- Runtime discovery of shared libraries.

## Architecture (high level)

```
API endpoints
  -> InferenceEngine
     -> TextManager
        -> EngineRegistry
           -> LlamaEngine (llama.cpp)
           -> SafetensorsEngine (safetensors.cpp)
  -> AudioManager (WhisperManager)
  -> ImageManager (SDManager)
```

## Manager responsibilities

### TextManager
- Registers built-in engines on startup.
- Resolves an engine by runtime, format, capability, and architecture.
- Exposes registered runtimes for model sync.

### AudioManager
- Thin wrapper around WhisperManager.

### ImageManager
- Thin wrapper around SDManager.

## Text engine selection
- ModelDescriptor.runtime selects the runtime (e.g., "llama_cpp", "safetensors_cpp").
- format/capability/architecture filters are applied via EngineRegistry.
- If benchmark metadata is missing, the first registered engine is used.

## Model format routing
- `.gguf` -> llama.cpp
- `.safetensors` or `*.safetensors.index.json` -> safetensors.cpp
- Unknown format -> explicit error

## API
- Responses API (preferred): `POST /v1/responses`
- Chat Completions (compat): `POST /v1/chat/completions`
- Audio: `POST /v1/audio/transcriptions`
- Images: `POST /v1/images/generations`

## Breaking changes
- Removed plugin directory and plugin-specific config fields.
- Removed engine plugin shared libraries and `manifest.json` under `allm/engines`.

## Migration
- Remove plugin configuration (e.g., engine plugins directory and restart policy).
- Ensure models are under `~/.llm-router/models` (or configured models dir).
- Prefer Responses API for new integrations.

## Testing requirements
- gpt/nemotron/qwen/glm model families must be covered by mandatory tests.
- Verification can be satisfied via model verification suite or integration/E2E coverage, but it must be recorded.

## Acceptance
- Managers present in code and docs.
- No plugin loader/ABI remains.
- Tests updated to manager assumptions.
- gpt/nemotron/qwen/glm families have explicit test coverage.
