#!/usr/bin/env bash
# Test: Basic Inference
# Verifies basic text generation works
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Basic Inference ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

PROMPT="Hello, who are you?"
echo "Prompt: $PROMPT"

OUTPUT=$(infer_command 50 "$PROMPT" 2>&1)

echo "Output:"
echo "$OUTPUT"

# Check output is not empty and has reasonable length
OUTPUT_LEN=${#OUTPUT}
if [[ $OUTPUT_LEN -lt 10 ]]; then
  echo "FAIL: Output too short ($OUTPUT_LEN chars)"
  exit 1
fi

echo "PASS: Basic inference works (output: $OUTPUT_LEN chars)"
exit 0
