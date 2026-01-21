# SPEC-d7feaa2c: Tasks

## Phase 0: Spec alignment
- [x] T000 Update spec/plan/data-model to manager approach.

## Phase 1: Managers (core)
- [x] T001 Add TextManager (EngineRegistry wrapper).
- [x] T002 Register LlamaEngine and SafetensorsEngine in TextManager.
- [x] T003 Add AudioManager/ImageManager wrappers (aliases).
- [x] T004 Refactor InferenceEngine to resolve via TextManager.
- [x] T005 Update main to advertise runtimes from TextManager.

## Phase 2: Remove plugin system
- [x] T006 Remove EngineHost / plugin ABI / plugin logger.
- [x] T007 Remove plugin wrappers and engine manifests.
- [x] T008 Update CMake to stop building plugin shared libraries.
- [x] T009 Remove plugin config fields.
- [x] T010 Remove plugin-based tests.
- [ ] T011 Scan remaining docs/specs for plugin references (optional).

## Phase 3: Docs/API
- [x] T012 Update README and README.ja for manager approach.
- [x] T013 Update SPEC quickstart.
- [x] T014 Update DEVELOPMENT.md and related docs if needed.
- [x] T015 Add migration guide / breaking change note.

## Phase 4: Tests
- [x] T016 Add TextManager unit tests.
- [ ] T017 Add manager-based integration coverage.
- [x] T018 Add mandatory test coverage for gpt/nemotron/qwen/glm model families.
- [x] T019 Run quality checks.

## Notes
- Tasks refreshed for manager migration (2026-01-19).
