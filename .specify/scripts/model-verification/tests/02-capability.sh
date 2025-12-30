#!/usr/bin/env bash
# Test: Capability Detection
# Verifies model capability matches expected type
set -euo pipefail

echo "=== Test: Capability Detection ==="
echo "Expected capability: $CAPABILITY"

# For now, we trust the declared capability
# Future: detect from model metadata or test output

case "$CAPABILITY" in
  TextGeneration)
    echo "Testing text generation capability..."
    OUTPUT=$("$LLM_NODE" \
      --model "$MODEL" \
      --n-predict 10 \
      --prompt "Hello" \
      2>/dev/null || echo "")
    if [[ -n "$OUTPUT" ]]; then
      echo "PASS: Text generation works"
      exit 0
    fi
    ;;
  Vision)
    echo "Testing vision capability..."
    # Vision models require image input - skip if not available
    echo "SKIP: Vision test requires image input"
    exit 77
    ;;
  Audio)
    echo "Testing audio capability..."
    echo "SKIP: Audio test requires audio input"
    exit 77
    ;;
  Embedding)
    echo "Testing embedding capability..."
    OUTPUT=$("$LLM_NODE" \
      --model "$MODEL" \
      --embedding \
      --prompt "test" \
      2>/dev/null || echo "")
    if [[ -n "$OUTPUT" ]]; then
      echo "PASS: Embedding generation works"
      exit 0
    fi
    ;;
  Reranker)
    echo "Testing reranker capability..."
    echo "SKIP: Reranker test not implemented"
    exit 77
    ;;
  *)
    echo "FAIL: Unknown capability: $CAPABILITY"
    exit 1
    ;;
esac

echo "FAIL: Capability test failed"
exit 1
