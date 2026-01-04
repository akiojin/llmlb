# PLANS.md

## 2026-01-04

- Focus: Implement Windows DirectML runtime (self-hosted) for Nemotron safetensors execution.
- Scope references: specs/SPEC-3fc2c1e4/spec.md, specs/SPEC-d7feaa2c/spec.md.

### Done
- Confirmed Nemotron DirectML uses gpt-oss compatible DLL API (assumption).
- Implemented Nemotron DirectML engine path (runtime loading, artifact selection, error handling).
- Added Nemotron engine plugin (manifest + entry) and CMake wiring.
- Added Windows DirectML runtime DLL skeleton (gptoss_directml/nemotron_directml) and loader updates.
- Updated DirectML artifact strategy to prefer safetensors with optional directml bin.
- Updated tests and docs/specs for the new DirectML runtime flow.
- Added DirectML device initialization (DXGI/D3D12/DML) in the in-tree runtime.
- Parsed gpt-oss DirectML artifacts (header + tokenizer blob) for model load.

### Plan B (proposed) + acceptance line
- Goal: Ship a self-hosted DirectML runtime that is shared by gpt-oss and Nemotron and validates the DirectML stack.
- Acceptance line:
  - gptoss_directml.dll and nemotron_directml.dll build from the same source.
  - The runtime verifies DirectML availability (DXGI + D3D12 + DirectML.dll) and returns a clear unsupported status if unavailable.
  - Model load succeeds when config/tokenizer exist; engine error codes surface missing artifacts.
  - Unit tests that cover DirectML loader paths remain green (skip on non-Windows).

### Remaining
- Implement DirectML inference (D3D12/DML graph + kernels) for gpt-oss/nemotron.
- Implement safetensors→DirectML graph build and execution in the runtime.
- Add Windows GPU inference integration tests + runtime build/deployment notes.
