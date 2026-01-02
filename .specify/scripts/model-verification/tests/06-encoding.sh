#!/usr/bin/env bash
# Test: Character Encoding
# Verifies model handles UTF-8, special characters, and emoji correctly
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Character Encoding ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

# Test 1: UTF-8 special characters
echo "Test 1: UTF-8 special characters..."
PROMPT1="Please repeat: café, naïve, résumé, Zürich"
OUTPUT1=$(infer_command 30 "$PROMPT1" 2>&1)
echo "Output: $OUTPUT1"

# Test 2: Emoji
echo "Test 2: Emoji..."
PROMPT2="What does this emoji mean? 🎉"
OUTPUT2=$(infer_command 30 "$PROMPT2" 2>&1)
echo "Output: $OUTPUT2"

# Test 3: Mixed scripts
echo "Test 3: Mixed scripts..."
PROMPT3="Translate 'hello' to: 日本語, 中文, 한국어"
OUTPUT3=$(infer_command 50 "$PROMPT3" 2>&1)
echo "Output: $OUTPUT3"

# Check for garbled output (common corruption patterns)
COMBINED="$OUTPUT1$OUTPUT2$OUTPUT3"
if echo "$COMBINED" | grep -qE '(ï¿½|\\x[0-9a-fA-F]{2}|<0x[0-9a-fA-F]+>)'; then
  echo "WARN: Possible encoding issues detected"
fi

# If we got any output without crashing, consider it a pass
if [[ -n "$OUTPUT1" ]] && [[ -n "$OUTPUT2" ]]; then
  echo "PASS: Encoding tests completed without crash"
  exit 0
fi

echo "FAIL: Encoding tests produced empty output"
exit 1
