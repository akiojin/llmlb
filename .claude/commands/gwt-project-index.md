---
description: Semantic search over project source files using the gwt-project-index skill
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GWT Project Index Command

Use this command to run semantic search against the project structure index.

## Usage

```text
/gwt:gwt-project-index [query]
```

## Steps

1. Load `.claude/skills/gwt-project-index/SKILL.md` and follow the workflow.
2. If index status is unknown, check index health before searching.
3. Run semantic search and return top results with short rationale:
   - path
   - relevance summary
   - next file(s) to inspect
4. If index is missing/outdated, explain that and provide the shortest recovery action.
5. For Issue search, use `/gwt:gwt-issue-search` instead.

## Examples

```text
/gwt:gwt-project-index where branch naming is built
```

```text
/gwt:gwt-project-index project mode pty orchestration
```
