---
description: Orchestrate Project Mode panes via PTY communication using the gwt-pty-communication skill
author: akiojin
allowed-tools: Read, Glob, Grep, Bash
---

# GWT PTY Communication Command

Use this command to coordinate Lead/Coordinator/Developer panes in Project Mode.

## Usage

```text
/gwt:gwt-pty-communication [context]
```

## Steps

1. Load `.claude/skills/gwt-pty-communication/SKILL.md` and follow the workflow.
2. Inspect active panes first (`list_terminals`) before sending instructions.
3. Prefer targeted routing (`send_keys_to_pane`) over broadcast when possible.
4. Confirm progress by reading pane output (`capture_scrollback_tail`).
5. Escalate or stop stuck panes with explicit reason.

## Examples

```text
/gwt:gwt-pty-communication leadがdeveloperへタスク配布したい
```

```text
/gwt:gwt-pty-communication coordinatorの進捗を確認
```
