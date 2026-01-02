#!/usr/bin/env bash
# Test: Performance (tokens/sec)
# Measures inference speed
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

echo "=== Test: Performance ==="

# Skip for non-text models
if [[ "$CAPABILITY" != "TextGeneration" ]]; then
  echo "SKIP: Not a text generation model"
  exit 77
fi

PROMPT="Explain the concept of artificial intelligence in detail."
N_PREDICT=100

echo "Prompt: $PROMPT"
echo "Generating $N_PREDICT tokens..."

# Run inference with timing
START_TIME=$(date +%s%N)

if [[ "${FORMAT:-}" == "gguf" ]]; then
  OUTPUT=$(infer_command "$N_PREDICT" "$PROMPT" 2>&1)
else
  OUTPUT=$("$LLM_NODE" \
    --model "$MODEL" \
    --n-predict $N_PREDICT \
    --prompt "$PROMPT" \
    2>&1)
fi

END_TIME=$(date +%s%N)

# Calculate metrics
DURATION_NS=$((END_TIME - START_TIME))
DURATION_SEC=$(echo "scale=3; $DURATION_NS / 1000000000" | bc)
WORD_COUNT=$(echo "$OUTPUT" | wc -w | tr -d ' ')

# Estimate tokens (roughly 1.3x word count for English)
ESTIMATED_TOKENS=$(echo "scale=0; $WORD_COUNT * 1.3" | bc)

if [[ "$DURATION_SEC" != "0" ]] && [[ "$DURATION_SEC" != ".000" ]]; then
  TOKENS_PER_SEC=$(echo "scale=2; $ESTIMATED_TOKENS / $DURATION_SEC" | bc)
else
  TOKENS_PER_SEC="N/A"
fi

echo "Output length: $WORD_COUNT words (~$ESTIMATED_TOKENS tokens)"
echo "Duration: ${DURATION_SEC}s"
echo "Speed: ${TOKENS_PER_SEC} tokens/sec"

# Save performance stats
{
  echo "duration_sec: $DURATION_SEC"
  echo "word_count: $WORD_COUNT"
  echo "estimated_tokens: $ESTIMATED_TOKENS"
  echo "tokens_per_sec: $TOKENS_PER_SEC"
  echo "platform: $PLATFORM"
} > "$RESULTS_DIR/performance-stats.txt"

# Performance test always passes - we just record the metrics
echo "PASS: Performance metrics recorded"
exit 0
