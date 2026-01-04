#!/bin/bash

# Claude Code PreToolUse Hook: Block quality check commands in Task tool
# Task toolでcargo fmt, cargo clippy, cargo test, make quality-checks等の品質チェックコマンドを
# 実行するとコンテキストを大量消費するため、ブロックします。
# これらのコマンドは直接Bashツールで出力制限付きで実行してください。

# stdinからJSON入力を読み取り
json_input=$(cat)

# JSONから値を取得するヘルパー関数
get_json_value() {
    local query="$1"
    if command -v jq >/dev/null 2>&1; then
        printf '%s' "$json_input" | jq -r "$query" 2>/dev/null
        return
    fi
    if command -v python >/dev/null 2>&1; then
        JSON_INPUT="$json_input" QUERY="$query" python - <<'PY' 2>/dev/null
import json
import os

data = os.environ.get("JSON_INPUT", "")
query = os.environ.get("QUERY", "")
try:
    obj = json.loads(data)
except Exception:
    print("")
    raise SystemExit

# ドット記法を解析してネストした値を取得
keys = query.strip('.').split('.')
value = obj
for key in keys:
    if isinstance(value, dict):
        value = value.get(key, "")
    else:
        value = ""
        break

print("" if value is None else str(value))
PY
        return
    fi
    printf '%s' ""
}

tool_name=$(get_json_value 'tool_name')

# Task ツール以外は許可
if [ "$tool_name" != "Task" ]; then
    exit 0
fi

# プロンプトを取得
prompt=$(get_json_value 'tool_input.prompt')
prompt_lower=$(echo "$prompt" | tr '[:upper:]' '[:lower:]')

# ブロックするパターン
blocked_patterns=(
    "cargo fmt"
    "cargo clippy"
    "cargo test"
    "make quality"
    "quality-checks"
    "markdownlint"
    "check-tasks"
    "check-commits"
)

for pattern in "${blocked_patterns[@]}"; do
    if echo "$prompt_lower" | grep -qi "$pattern"; then
        cat <<EOF
{
  "decision": "block",
  "reason": "🚫 Quality check commands should not be run via Task tool",
  "stopReason": "品質チェックコマンド ($pattern) は Task tool で実行するとコンテキストを大量消費します。代わりに直接 Bash ツールで出力制限付きで実行してください。例: cargo fmt --check > /dev/null 2>&1 && echo '✓ fmt OK' || echo '✗ fmt FAIL'"
}
EOF
        echo "🚫 Blocked: Task tool with quality check command ($pattern)" >&2
        exit 2
    fi
done

# 許可
exit 0
