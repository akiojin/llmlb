---
description: Create or update GitHub PRs with the gh CLI using the gwt-pr skill
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GitHub PR Command

Use this command to draft or update a GitHub PR with the gh CLI.

## Usage

```
/gwt:gwt-pr [optional context]
```

## Steps

1. Load `.claude/skills/gwt-pr/SKILL.md` and follow the workflow.
2. Ensure `gh auth status` succeeds before running PR commands.
3. Run the local working tree preflight from the skill (`git status --porcelain`); if changes exist, confirm with the user before push/PR operations.
4. Run the branch sync preflight from the skill (`git rev-list --left-right --count "HEAD...origin/$base"`); if the branch is behind, merge `origin/$base` into the current branch and push before PR creation.
5. If that merge produces conflicts, inspect the conflict carefully and only ask the user when it cannot be resolved with high confidence.
6. When all PRs for the head are merged, validate merge commit ancestry before counting post-merge commits.
7. If the merge commit is missing or not an ancestor of `HEAD`, compare `origin/<head>..HEAD` before any base-branch fallback.
8. If both upstream and base comparisons fail, stop with `MANUAL CHECK`; do not create a PR by guesswork.
9. Generate or update the PR body using the provided templates.
10. If `gh pr create` or `gh pr edit` fails with `was submitted too quickly` or a secondary rate limit, retry through the REST pull-request endpoint with `gh api repos/<owner>/<repo>/pulls`.

## Examples

```
/gwt:gwt-pr create draft for current branch
```

```
/gwt:gwt-pr update PR body only
```
