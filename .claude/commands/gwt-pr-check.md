---
description: Check GitHub PR status and post-merge commits using the gwt-pr-check skill
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GitHub PR Check Command

Use this command to inspect PR status for the current branch with the gh CLI.

## Usage

```text
/gwt:gwt-pr-check [optional context]
```

## Steps

1. Load `.claude/skills/gwt-pr-check/SKILL.md` and follow the workflow.
2. Ensure `gh auth status` succeeds before running PR checks.
3. Run checks and return a human-readable summary:
   - Result
   - Recommended next step
   - Why
   - Context and key evidence
4. Append JSON only if the user explicitly asks for machine-readable output.
5. Do not push or create/edit PRs in this command.

## Examples

```text
/gwt:gwt-pr-check
```

```text
/gwt:gwt-pr-check check current branch after merge
```
