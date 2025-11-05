#!/bin/bash

# Claude Code PreToolUse Hook: Block git branch operations
# このスクリプトは git checkout, git switch, git branch, git worktree コマンドをブロックします

set -euo pipefail

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Error: required command not found: $1" >&2
        exit 1
    fi
}

require_command jq

# stdinからJSON入力を読み取り
json_input=$(cat)

if [[ -z "$json_input" ]]; then
    echo "Error: missing JSON input for PreToolUse hook" >&2
    exit 1
fi

# ツール名を確認（jqのパース失敗時は安全側で終了）
tool_name=$(printf '%s' "$json_input" | jq -er '.tool_name // ""' 2>/dev/null) || {
    echo "Error: invalid JSON payload (tool_name)" >&2
    exit 1
}

if [[ -z "$tool_name" ]]; then
    echo "Error: tool_name is empty" >&2
    exit 1
fi

# Bashツール以外は許可
if [[ "$tool_name" != "Bash" ]]; then
    exit 0
fi

# コマンドを取得（jq失敗時は安全側で終了）
command_str=$(printf '%s' "$json_input" | jq -er '.tool_input.command // ""' 2>/dev/null) || {
    echo "Error: invalid JSON payload (tool_input.command)" >&2
    exit 1
}

if [[ -z "$command_str" ]]; then
    echo "Error: command string is empty" >&2
    exit 1
fi

# コマンド全体を対象に git 操作を検知する
blocked_pattern='(^|[[:space:];|&()])git[[:space:]]+(checkout|switch|branch|worktree)([[:space:]]|$)'

if [[ "$command_str" =~ $blocked_pattern ]]; then
    reason_text='🚫 ブランチ切り替え・作成・worktreeコマンドは禁止されています / Branch switching, creation, and worktree commands are not allowed'
    stop_reason_base=$'Worktreeは起動したブランチで作業を完結させる設計です。git checkout、git switch、git branch、git worktree 等の操作は実行できません。\n\nReason: Worktree is designed to complete work on the launched branch. Branch operations such as git checkout, git switch, git branch, and git worktree cannot be executed.\n\nBlocked command: '

    blocked_response=$(jq -n --arg reason "$reason_text" --arg base "$stop_reason_base" --arg cmd "$command_str" '{decision:"block", reason:$reason, stopReason: ($base + $cmd)}')
    printf '%s\n' "$blocked_response"

    echo "🚫 ブロック: $command_str" >&2
    echo "理由: Worktreeは起動したブランチで作業を完結させる設計です。" >&2

    exit 2  # ブロック
fi

# 許可
exit 0
