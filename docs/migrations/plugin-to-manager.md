# Migration: Engine Plugins to Built-in Managers

This migration applies to xLLM runtime deployments that previously used engine plugins.

## Summary
- The plugin loader and `LLM_RUNTIME_ENGINE_PLUGINS_DIR` were removed.
- Text runtimes are built-in: `llama_cpp` and `safetensors_cpp`.
- Engine manifests and shared libraries under `xllm/engines` are no longer used.

## Required actions
1. Remove `LLM_RUNTIME_ENGINE_PLUGINS_DIR` from your environment and config.
2. Remove any plugin-specific entries from runtime config files.
3. Ensure model metadata uses the built-in runtime identifiers:
   - `llama_cpp` for GGUF models
   - `safetensors_cpp` for safetensors models

## Optional checks
- `GET /v0/runtimes` shows the built-in runtimes.
- Run `make quality-checks` after updating your deployment.
