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
2. Run `python3 ".codex/skills/gwt-pr-check/scripts/check_pr_status.py" --repo "."`.
3. Ensure `gh auth status` succeeds before running PR checks.
4. When all PRs for the head are merged, validate merge commit ancestry before counting post-merge commits.
5. If the merge commit is missing or not an ancestor of `HEAD`, compare `origin/<head>..HEAD` before any base-branch fallback.
6. If both upstream and base comparisons fail, return `MANUAL CHECK` instead of inferring `CREATE PR`.
7. Return the human-readable summary from the script:
   - Result
   - Recommended next step
   - Why
   - Context and key evidence
8. Append JSON only if the user explicitly asks for machine-readable output.
9. Do not push or create/edit PRs in this command.

## Examples

```text
/gwt:gwt-pr-check
```

```text
/gwt:gwt-pr-check check current branch after merge
```
