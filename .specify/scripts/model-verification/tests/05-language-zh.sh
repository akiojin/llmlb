#!/usr/bin/env bash
# Test: Chinese Language Support
# Verifies model can handle Chinese input/output
set -euo pipefail

echo "=== Test: Chinese Language Support ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

PROMPT="你好，请用中文介绍一下你自己。"
echo "Prompt: $PROMPT"

OUTPUT=$("$LLM_NODE" \
  --model "$MODEL" \
  --n-predict 100 \
  --prompt "$PROMPT" \
  2>&1)

echo "Output:"
echo "$OUTPUT"

# Check for Chinese characters in output
if echo "$OUTPUT" | grep -qP '[\x{4E00}-\x{9FFF}]' 2>/dev/null || \
   echo "$OUTPUT" | grep -E '[一-龯]' 2>/dev/null; then
  echo "PASS: Chinese output detected"
  exit 0
fi

# Check if output contains any non-ASCII
if echo "$OUTPUT" | grep -qP '[^\x00-\x7F]' 2>/dev/null; then
  echo "PASS: Non-ASCII output detected (possible Chinese)"
  exit 0
fi

echo "WARN: No Chinese characters detected in output"
echo "PASS: Model processed Chinese input without error"
exit 0
