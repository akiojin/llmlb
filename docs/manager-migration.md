# Manager Migration Guide (Plugin -> Managers)

This project replaced the dynamic engine plugin system with in-process managers.
Use this guide when upgrading from legacy plugin-based deployments.

## Breaking changes
- Engine plugins and their ABI/manifest loading are removed.
- `LLM_RUNTIME_ENGINE_PLUGINS_DIR` and other plugin-only settings are no longer used.
- Built-in managers are the only runtime entry points.

## What replaces plugins
- **TextManager**: `llama_cpp` (GGUF) + `safetensors_cpp` (safetensors)
- **AudioManager**: whisper.cpp
- **ImageManager**: stable-diffusion.cpp

## Migration steps
1. Remove plugin directories and plugin manifests from your deployment.
2. Delete plugin-related environment variables (notably `LLM_RUNTIME_ENGINE_PLUGINS_DIR`).
3. Ensure models are under `~/.llm-router/models` (or `LLM_MODELS_DIR`).
4. Prefer `POST /v1/responses` for new integrations; Chat Completions is kept for compatibility.

## Notes
- Model format routing is automatic: `*.gguf` -> `llama_cpp`, `*.safetensors` -> `safetensors_cpp`.
- If you depended on plugin hot-reload, plan a rolling restart workflow instead.
