---
name: gwt-spec-to-issue-migration
description: "Migrate legacy spec sources to artifact-first GitHub Issue specs. Supports local `specs/SPEC-*` directories and body-canonical `gwt-spec` Issues using the bundled migration script."
---

# gwt Spec to Issue Migration

## Overview

Use this skill for legacy spec migrations:

- local `specs/SPEC-*` directories from a pre-Issue-first workflow
- existing `gwt-spec` Issues that still keep the canonical bundle in the Issue body

Migrate legacy sources to artifact-first `gwt-spec` Issues, then remove or retire the old source of truth.

This skill uses:
- `.codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs`
- `crates/gwt-core/src/git/issue_spec.rs`

## Preconditions

- Run in repository root
- `gh auth status` is authenticated for target repo
- Branch policy is respected (no branch creation/switching unless user requests)
- `$GWT_PROJECT_ROOT` environment variable is available; prefer it over CWD for repo resolution

## Standard Workflow

1. Inspect source specs directory (auto-detection or explicit `--specs-dir`)
2. Run dry-run automatically and review planned migration count and deletion targets
3. If the user explicitly asked to migrate or convert, continue into actual migration after the dry-run
4. Verify migrated issues exist (`gwt-spec` label)
5. Confirm legacy local spec files or body-canonical bundles were retired on success
6. Ask the user only when migration intent is unclear or the requested scope does not obviously include the detected destructive changes

## Commands

### Dry-run

```bash
node ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs" --dry-run
```

### Dry-run with explicit specs directory

```bash
node ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs" --dry-run --specs-dir "<path-to-specs>"
```

### Execute migration

```bash
node ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs"
```

### Dry-run existing body-canonical issue migration

```bash
node ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs" --dry-run --convert-existing-issues
```

### Execute existing body-canonical issue migration

```bash
node ".codex/skills/gwt-spec-to-issue-migration/scripts/migrate-specs-to-issues.mjs" --convert-existing-issues
```

### Verify report

```bash
cat migration-report.json
```

Note: `migration-report.json` is deleted automatically after a fully successful migration cleanup. It remains available for dry-run and failure cases.

### Verify created issues

```bash
gh issue list --label gwt-spec --state all --limit 200
```

## Expected Behavior

- Auto-detects local `specs/` under `$GWT_PROJECT_ROOT` or the current repository
- If no `SPEC-*` directory exists, exits successfully with empty report (`[]`)
- Migrates local sections from `spec.md`, `plan.md`, `tasks.md` and related artifacts
- Can rewrite body-canonical `gwt-spec` Issues into artifact-first format
- Writes per-spec result to `migration-report.json`
- Shows planned deletions during `--dry-run`
- Deletes migrated local spec directories, detected legacy workflow leftovers, and `migration-report.json` after a fully successful cleanup
- Treats an explicit "migrate/convert" request as approval to execute after the dry-run summary, without an extra confirmation loop
- Uses REST-safe body-file writes and retry/backoff for GitHub issue create/edit/comment operations where available

## Notes

- For safety, always run `--dry-run` first.
- Artifact files in `contracts/` and `checklists/` are migrated as issue comments.
- `doc:*` artifacts are created for `spec.md`, `plan.md`, `tasks.md`, `research.md`, `data-model.md`, and `quickstart.md`.
- After migration, ongoing spec updates should use Issue-first operations (`gwt-spec-ops`).
- This skill is for external legacy import only; gwt's normal spec workflow should never recreate repository-local spec bundles.
