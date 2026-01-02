#!/usr/bin/env bash
# Test: Model Loading
# Verifies that the model can be loaded by the engine
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Model Loading ==="
echo "Model: $MODEL"
echo "Engine: $ENGINE"

# Check if model file exists and has valid size
if [[ ! -f "$MODEL" ]]; then
  echo "FAIL: Model file not found"
  exit 1
fi

MODEL_SIZE=$(stat -f%z "$MODEL" 2>/dev/null || stat -c%s "$MODEL" 2>/dev/null)
echo "Model size: $MODEL_SIZE bytes"

if [[ "$MODEL_SIZE" -lt 1000000 ]]; then
  echo "FAIL: Model file too small (possibly corrupt)"
  exit 1
fi

# Try to load model with llm-node (quick validation mode)
echo "Loading model..."
run_with_timeout 60 infer_command 1 "test" 2>&1 | head -20

echo "PASS: Model loaded successfully"
exit 0
