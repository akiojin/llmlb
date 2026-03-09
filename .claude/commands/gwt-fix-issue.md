---
description: >-
  Analyze a GitHub Issue, extract error context, search the codebase for
  relevant files, and propose a concrete fix plan.
  Use when: (1) user explicitly asks to fix/analyze an issue,
  (2) user provides an issue number or URL and asks for help,
  (3) user says 'Issueを直して/fix issue/analyze issue/investigate #123'.
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GitHub Issue Fix Command

Use this command to analyze a GitHub Issue and propose a fix plan.

## Usage

```
/gwt:gwt-fix-issue [issue-number|issue-url|optional context]
```

## Steps

1. Load `.claude/skills/gwt-fix-issue/SKILL.md` and follow the workflow.
2. Run the inspection script to gather issue data and extract context.
3. Search the codebase for relevant files and definitions.
4. Produce an Issue Analysis Report.
5. Propose fixes and apply them after user approval.

## Proactive Trigger Examples

<example>
Context: User mentions an issue number and asks for help
user: "#42 のバグを直して"
assistant: "gwt-fix-issue で Issue #42 を分析します。"
<commentary>
Issue 番号が指定されたので gwt-fix-issue で分析を開始する。
</commentary>
</example>

<example>
Context: User provides an issue URL
user: "https://github.com/org/repo/issues/123 を調査して"
assistant: "gwt-fix-issue で Issue #123 の内容を分析し、修正計画を提案します。"
</example>

<example>
Context: User asks to investigate a bug
user: "この Issue を修正したい"
assistant: "gwt-fix-issue で Issue を分析し、エラーコンテキストを抽出して修正計画を提案します。"
</example>

## Examples

```
/gwt:gwt-fix-issue 42
```

```
/gwt:gwt-fix-issue https://github.com/org/repo/issues/123
```
