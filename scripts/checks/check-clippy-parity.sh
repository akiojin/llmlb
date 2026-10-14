#!/usr/bin/env bash

# check-clippy-parity.sh - ローカル clippy と CI clippy のコマンド整合チェック
#
# Makefile の clippy ターゲットが、GitHub Actions ワークフローで実行される
# cargo clippy コマンドと同一であることを検証する（Issue #704 / #625 の再発防止）。
#
# 戻り値:
#   0: すべての CI clippy コマンドが Makefile と一致
#   1: 不一致あり
#   2: エラー（コマンドが見つからない等）

set -euo pipefail

MAKEFILE="${MAKEFILE:-Makefile}"
WORKFLOWS_DIR="${WORKFLOWS_DIR:-.github/workflows}"

extract_clippy() {
    grep -hE '^[[:space:]]*(run:[[:space:]]*)?cargo clippy' "$@" \
        | sed -E 's/^[[:space:]]*(run:[[:space:]]*)?//; s/[[:space:]]+$//' \
        | sort -u
}

local_cmd=$(extract_clippy "$MAKEFILE" || true)
if [ -z "$local_cmd" ]; then
    echo "✗ Makefile に cargo clippy コマンドが見つかりません: $MAKEFILE" >&2
    exit 2
fi
if [ "$(printf '%s\n' "$local_cmd" | wc -l | tr -d ' ')" -ne 1 ]; then
    echo "✗ Makefile に複数の cargo clippy コマンドがあります:" >&2
    printf '%s\n' "$local_cmd" >&2
    exit 2
fi

ci_cmds=$(extract_clippy "$WORKFLOWS_DIR"/*.yml || true)
if [ -z "$ci_cmds" ]; then
    echo "✗ CI ワークフローに cargo clippy コマンドが見つかりません: $WORKFLOWS_DIR" >&2
    exit 2
fi

status=0
while IFS= read -r ci_cmd; do
    if [ "$ci_cmd" != "$local_cmd" ]; then
        echo "✗ clippy コマンド不一致" >&2
        echo "  Makefile: $local_cmd" >&2
        echo "  CI:       $ci_cmd" >&2
        status=1
    fi
done <<< "$ci_cmds"

if [ "$status" -eq 0 ]; then
    echo "✓ clippy parity OK: $local_cmd"
fi
exit "$status"
