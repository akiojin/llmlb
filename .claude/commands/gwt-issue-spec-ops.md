---
description: Manage Issue-first specs (gwt-spec) using the gwt-issue-spec-ops skill
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GWT Issue Spec Ops Command

Use this command to create/update Issue-first SPEC artifacts on GitHub Issues.

## Usage

```text
/gwt:gwt-issue-spec-ops [issue-number|context]
```

## Steps

1. Load `.claude/skills/gwt-issue-spec-ops/SKILL.md` and follow the workflow.
2. Ensure `gh auth status` is valid before issue operations.
3. Create or update the Spec/Plan/Tasks sections on the target `gwt-spec` issue.
4. Keep SPEC ID as the GitHub issue number and preserve section structure.
5. Report what was changed and what remains unresolved.

## Examples

```text
/gwt:gwt-issue-spec-ops 1288
```

```text
/gwt:gwt-issue-spec-ops 新機能のspecを作成して
```
