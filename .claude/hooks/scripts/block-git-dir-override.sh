#!/bin/bash

# Claude Code PreToolUse Hook: Block GIT_DIR environment variable override
# このスクリプトは GIT_DIR 環境変数の書き換えをブロックします
# Worktree環境ではGIT_DIRの変更により意図しないリポジトリ操作が発生する可能性があるため

# stdinからJSON入力を読み取り
json_input=$(cat)

# ツール名を確認
tool_name=$(jq -r '.tool_name // empty' <<< "$json_input")

# Bashツール以外は許可
if [ "$tool_name" != "Bash" ]; then
    exit 0
fi

# コマンドを取得
command=$(jq -r '.tool_input.command // empty' <<< "$json_input")

# GIT_DIR の設定パターンをチェック
# パターン:
#   - export GIT_DIR=...
#   - GIT_DIR=...
#   - env GIT_DIR=...
#   - declare -x GIT_DIR=...
if grep -qE '(^|[;&|]|[[:space:]])(export[[:space:]]+)?GIT_DIR[[:space:]]*=|env[[:space:]]+[^;]*GIT_DIR[[:space:]]*=|declare[[:space:]]+-x[[:space:]]+GIT_DIR[[:space:]]*=' <<< "$command"; then
    # JSON応答を返す（jqで安全にエスケープ）
    jq -n \
      --arg cmd "$command" \
      '{
        decision: "block",
        reason: "🚫 GIT_DIR environment variable override is not allowed",
        stopReason: ("Modifying GIT_DIR in a worktree environment can cause unintended repository operations.\n\nBlocked command: " + $cmd + "\n\nWorktrees have their own .git file pointing to the main repository worktree directory. Overriding GIT_DIR may break this relationship and cause git commands to operate on the wrong repository.")
      }'

    # stderrにもメッセージを出力
    echo "🚫 Blocked: $command" >&2
    echo "Reason: GIT_DIR override is not allowed in worktree environment." >&2

    exit 2  # ブロック
fi

# GIT_WORK_TREE の設定も同様にブロック（GIT_DIRと組み合わせて使われることが多い）
if grep -qE '(^|[;&|]|[[:space:]])(export[[:space:]]+)?GIT_WORK_TREE[[:space:]]*=|env[[:space:]]+[^;]*GIT_WORK_TREE[[:space:]]*=|declare[[:space:]]+-x[[:space:]]+GIT_WORK_TREE[[:space:]]*=' <<< "$command"; then
    # JSON応答を返す（jqで安全にエスケープ）
    jq -n \
      --arg cmd "$command" \
      '{
        decision: "block",
        reason: "🚫 GIT_WORK_TREE environment variable override is not allowed",
        stopReason: ("Modifying GIT_WORK_TREE in a worktree environment can cause unintended repository operations.\n\nBlocked command: " + $cmd + "\n\nWorktrees have their own working directory configuration. Overriding GIT_WORK_TREE may cause git commands to operate on the wrong directory.")
      }'

    # stderrにもメッセージを出力
    echo "🚫 Blocked: $command" >&2
    echo "Reason: GIT_WORK_TREE override is not allowed in worktree environment." >&2

    exit 2  # ブロック
fi

# 許可
exit 0
