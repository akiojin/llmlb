---
name: gwt-project-index
description: Semantic search over project files and GitHub Issues using vector embeddings.
---

# Project Structure Index

gwt maintains a vector search index of all project files and GitHub Issues using ChromaDB embeddings.

## File search command

Run in terminal to find files related to a feature or concept:

```bash
~/.gwt/runtime/chroma-venv/bin/python3 ~/.gwt/runtime/chroma_index_runner.py \
  --action search \
  --db-path "$GWT_PROJECT_ROOT/.gwt/index" \
  --query "your search query" \
  --n-results 10
```

On Windows, use `~/.gwt/runtime/chroma-venv/Scripts/python.exe` as the Python executable.

## File search output format

JSON object with ranked results:

```json
{"ok": true, "results": [
  {"path": "src/git/issue.rs", "description": "GitHub Issue commands", "distance": 0.12},
  {"path": "src/lib/components/IssuePanel.svelte", "description": "Issue list panel", "distance": 0.25}
]}
```

## GitHub Issues search command

First, update the Issues index (fetches `gwt-spec` Issues via `gh` CLI):

```bash
~/.gwt/runtime/chroma-venv/bin/python3 ~/.gwt/runtime/chroma_index_runner.py \
  --action index-issues \
  --db-path "$GWT_PROJECT_ROOT/.gwt/index"
```

Then search Issues semantically:

```bash
~/.gwt/runtime/chroma-venv/bin/python3 ~/.gwt/runtime/chroma_index_runner.py \
  --action search-issues \
  --db-path "$GWT_PROJECT_ROOT/.gwt/index" \
  --query "your search query" \
  --n-results 10
```

## Issues search output format

```json
{"ok": true, "issueResults": [
  {"number": 42, "title": "Add vector search for Issues", "url": "https://github.com/...", "state": "open", "labels": ["gwt-spec"], "distance": 0.08}
]}
```

## When to use

- Task start: search for files and Issues related to the assigned feature
- Bug investigation: find files and spec Issues that might relate to the bug
- Feature addition: locate existing similar implementations and relevant specs
- Architecture understanding: discover how components are organized

## Environment

- `GWT_PROJECT_ROOT`: absolute path to the project root (set by gwt at pane launch)

## Notes

- File index is auto-generated when the project is opened in gwt
- Issue index must be updated manually (via GUI "Update Index" button or `index-issues` action)
- Both use semantic similarity (not just keyword matching)
- Lower distance values indicate higher relevance
