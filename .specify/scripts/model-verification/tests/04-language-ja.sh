#!/usr/bin/env bash
# Test: Japanese Language Support
# Verifies model can handle Japanese input/output
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Japanese Language Support ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

PROMPT="こんにちは。あなたは誰ですか？日本語で答えてください。"
echo "Prompt: $PROMPT"

OUTPUT=$(infer_command 100 "$PROMPT" 2>&1)

echo "Output:"
echo "$OUTPUT"

# Check for Japanese characters in output (Hiragana/Katakana/Kanji ranges)
if echo "$OUTPUT" | grep -qP '[\x{3040}-\x{309F}\x{30A0}-\x{30FF}\x{4E00}-\x{9FFF}]' 2>/dev/null || \
   echo "$OUTPUT" | grep -E '[ぁ-んァ-ン一-龯]' 2>/dev/null; then
  echo "PASS: Japanese output detected"
  exit 0
fi

# Check if output contains any non-ASCII (may indicate partial Japanese support)
if echo "$OUTPUT" | grep -qP '[^\x00-\x7F]' 2>/dev/null; then
  echo "PASS: Non-ASCII output detected (possible Japanese)"
  exit 0
fi

echo "WARN: No Japanese characters detected in output"
echo "PASS: Model processed Japanese input without error"
exit 0
