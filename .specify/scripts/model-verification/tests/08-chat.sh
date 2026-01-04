#!/usr/bin/env bash
# Test: Multi-turn Chat
# Verifies model can handle conversation context
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Multi-turn Chat ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

CHAT_PROMPT=$(format_chat_prompt)
echo "Chat template: ${CHAT_TEMPLATE:-plain}"
echo "Prompt preview:"
echo "$CHAT_PROMPT"

set +e
OUTPUT=$(infer_command 80 "$CHAT_PROMPT" 2>&1)
EXIT_CODE=$?
set -e

if [[ $EXIT_CODE -ne 0 ]]; then
  echo "FAIL: Chat inference failed (exit $EXIT_CODE)"
  exit 1
fi

OUTPUT_LEN=${#OUTPUT}
if [[ $OUTPUT_LEN -lt 10 ]]; then
  echo "FAIL: Chat output too short ($OUTPUT_LEN chars)"
  exit 1
fi

echo "PASS: Chat test completed (output: $OUTPUT_LEN chars)"
exit 0
