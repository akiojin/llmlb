# Contributing

This repository follows spec-driven, test-driven development. Please keep
changes small, well-documented, and fully verified before pushing.

## Ground Rules
- No branch or worktree creation from this environment.
- Keep TDD: write a failing test, make it pass, then refactor.
- Follow Conventional Commits; summary within 50 characters, use scopes when
  helpful (例: `fix(api): cloud key warning`).
- Run the full quality gate locally; do not push on red tests or lint.
- Default to simplicity; avoid adding dependencies unless necessary.

## Workflow
1. Read relevant specs under `specs/` and `.agent/PLANS.md`.
2. List the task in your TODOs, then start with a test (RED).
3. Implement the minimum to turn tests green, refactor if needed.
4. Verify locally:
   - `make quality-checks`
   - `.specify/scripts/checks/check-commits.sh --from origin/main --to HEAD`
5. Commit with Japanese summary and conventional format.
6. Push after tests pass.

## Documentation & Lint
- Markdown: `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`
- Keep README as an entry point; place design details in dedicated docs.
- Prefer ASCII; add non-ASCII only when the file already uses it or when
  unavoidable for clarity.

## Testing Shortcuts
- Rust: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
- JS/TS: `pnpm test` when applicable.
- OpenAI-compatible API: `make openai-tests` (included in `make quality-checks`).
