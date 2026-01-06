#!/usr/bin/env bash
set -euo pipefail

TARGET="${WRK_TARGET:-http://localhost:32768}"
ENDPOINT="${WRK_ENDPOINT:-/v1/chat/completions}"

if ! command -v wrk >/dev/null 2>&1; then
  echo "wrk not found. Install wrk and retry." >&2
  exit 1
fi

# If WRK_SCRIPT is provided, use it as-is.
# Otherwise build a minimal POST chat payload with optional WRK_MODEL/WRK_BODY_JSON.
SCRIPT_PATH="${WRK_SCRIPT:-}"

if [ -z "${SCRIPT_PATH}" ]; then
  MODEL="${WRK_MODEL:-gpt-oss:20b}"
  BODY_JSON="${WRK_BODY_JSON:-{\"model\":\"$MODEL\",\"messages\":[{\"role\":\"user\",\"content\":\"benchmark ping\"}],\"stream\":false}}"

  TMP_SCRIPT="$(mktemp)"
  trap 'rm -f "$TMP_SCRIPT"' EXIT
  cat >"$TMP_SCRIPT" <<EOF
wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.body = [[
$BODY_JSON
]]
EOF
  SCRIPT_PATH="$TMP_SCRIPT"
fi

echo "Running wrk against ${TARGET}${ENDPOINT}"
echo "Using script: ${SCRIPT_PATH}"

wrk "$@" -s "${SCRIPT_PATH}" "${TARGET}${ENDPOINT}"
