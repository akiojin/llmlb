# SPEC-d7feaa2c: Plan (Manager migration)

## Phase 0: Align spec and docs
- Replace plugin-based design with manager-based design in spec/docs.
- Confirm scope: Text/Audio/Image managers and Responses API guidance.

## Phase 1: Core managers
- Implement TextManager with EngineRegistry and built-in engines.
- Provide AudioManager/ImageManager wrappers.
- Refactor InferenceEngine to resolve via TextManager.

## Phase 2: Remove plugin system
- Delete EngineHost, plugin ABI, and plugin logger.
- Remove engine plugin wrappers and manifests.
- Update CMake to stop building plugin shared libs.
- Remove plugin config fields from node config.

## Phase 3: Routing and API docs
- Ensure supported runtimes are derived from TextManager.
- Update API docs to prefer Responses API and mark Chat Completions as compat.

## Phase 4: Tests and QA
- Remove plugin-based tests.
- Add manager-based unit/integration coverage.
- Require coverage for gpt/nemotron/qwen/glm model families (verification or integration/E2E).
- Run quality checks when feasible.

## Phase 5: Migration guide
- Document breaking changes and migration steps.
