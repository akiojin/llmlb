---
name: gwt-spec-to-issue-migration
description: "Migrate existing local SPEC directories (specs/SPEC-*) to GitHub Issue-first specs (gwt-spec label) using the bundled migration script. Use when asked to replace local spec.md/plan.md/tasks.md based workflow with GitHub Issue based management."
---

# gwt Spec to Issue Migration

## Overview

Migrate local `specs/SPEC-*` directories to GitHub Issues (`gwt-spec` label) and switch to Issue-first spec management.

This skill uses:
- `.codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.sh`
- `crates/gwt-core/src/git/issue_spec.rs`

## Preconditions

- Run in repository root
- `gh auth status` is authenticated for target repo
- Branch policy is respected (no branch creation/switching unless user requests)
- `$GWT_PROJECT_ROOT` environment variable is available; prefer it over CWD for repo resolution

## Standard Workflow

1. Inspect source specs directory (auto-detection or explicit `--specs-dir`)
2. Run dry-run and review planned migration count
3. Run actual migration
4. Review `migration-report.json`
5. Verify migrated issues exist (`gwt-spec` label)

## Commands

### Dry-run

```bash
bash ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.sh" --dry-run
```

### Dry-run with explicit specs directory

```bash
bash ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.sh" --dry-run --specs-dir "<path-to-specs>"
```

### Execute migration

```bash
bash ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.sh"
```

### Verify report

```bash
cat migration-report.json
```

### Verify created issues

```bash
gh issue list --label gwt-spec --state all --limit 200
```

## Expected Behavior

- Auto-detects local `specs/` under `$GWT_PROJECT_ROOT` or the current repository
- If no `SPEC-*` directory exists, exits successfully with empty report (`[]`)
- Migrates sections from `spec.md`, `plan.md`, `tasks.md` and related artifacts
- Writes per-spec result to `migration-report.json`

## Notes

- For safety, always run `--dry-run` first.
- Artifact files in `contracts/` and `checklists/` are migrated as issue comments.
- After migration, ongoing spec updates should use Issue-first operations (`gwt-issue-spec-ops`).
