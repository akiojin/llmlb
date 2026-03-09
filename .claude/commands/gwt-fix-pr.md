---
description: >-
  Inspect GitHub PR for CI failures, merge conflicts, update-branch requirements,
  reviewer comments, change requests, and unresolved review threads.
  Create fix plans and implement after user approval.
  Use when: (1) user explicitly asks to fix CI/PR issues,
  (2) after creating or pushing to a PR and CI checks fail or reviews are requested,
  (3) user says 'CIを直して/fix ci/fix pr/マージできない'.
  PROACTIVE: After any PR creation or push, if CI failures or merge blockers
  are detected, automatically invoke this skill without waiting for user request.
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GitHub CI Fix Command

Use this command to diagnose and fix CI failures for a PR.

## Usage

```
/gwt:gwt-fix-pr [pr-number|pr-url|optional context]
```

## Steps

1. Load `.claude/skills/gwt-fix-pr/SKILL.md` and follow the workflow.
2. Run the inspection script to gather CI results.
3. Propose fixes and apply them after user approval.

## Proactive Trigger Examples

<example>
Context: LLM just created a PR using gwt-pr, and CI checks may fail
user: "PRを作成して"
assistant: (gwt-pr でPR作成完了後) "PR #123 を作成しました。CI チェックの状態を確認します。"
<commentary>
PR作成直後、自動的に gwt-fix-pr で検査する。
</commentary>
</example>

<example>
Context: User pushed commits and mentions the PR isn't passing
user: "pushしたけどCIが通らない"
assistant: "gwt-fix-pr で PR の状態を診断します。"
</example>

<example>
Context: User mentions PR can't be merged
user: "PRがマージできない"
assistant: "gwt-fix-pr で blocking items を診断します。"
</example>

## Examples

```
/gwt:gwt-fix-pr 123
```

```
/gwt:gwt-fix-pr https://github.com/org/repo/pull/123
```
