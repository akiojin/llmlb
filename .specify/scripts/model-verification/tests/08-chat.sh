#!/usr/bin/env bash
# Test: Multi-turn Chat
# Verifies model can handle conversation context
set -euo pipefail

echo "=== Test: Multi-turn Chat ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

# This test is optional - skip if chat template not available
echo "SKIP: Multi-turn chat test requires chat template support"
echo "Future: Implement chat template detection and multi-turn testing"
exit 77

# Future implementation:
# 1. Detect chat template from model
# 2. Format multi-turn conversation
# 3. Verify context is maintained
