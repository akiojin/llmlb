---
description: Semantic search over project files and GitHub Issues using the gwt-project-index skill
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
2. If the request is about existing specs, related issues, or where to integrate a change, run **Issues search first**.
3. If index status is unknown, check index health before searching.
4. Run semantic search and return top results with short rationale:
   - path
   - relevance summary
   - next file(s) to inspect
5. If index is missing/outdated, explain that and provide the shortest recovery action.

## Examples

```text
/gwt:gwt-project-index where branch naming is built
```

```text
/gwt:gwt-project-index project mode pty orchestration
```

```text
/gwt:gwt-project-index related project index spec
```
