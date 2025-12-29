#!/usr/bin/env bash
# Test: Memory Usage
# Measures memory consumption during inference
set -euo pipefail

echo "=== Test: Memory Usage ==="

# Get initial memory (platform-specific)
get_memory_mb() {
  if [[ "$(uname)" == "Darwin" ]]; then
    # macOS: use vm_stat or ps
    ps -o rss= -p $$ 2>/dev/null | awk '{print int($1/1024)}' || echo "0"
  else
    # Linux: use /proc/meminfo
    free -m 2>/dev/null | awk '/^Mem:/{print $3}' || echo "0"
  fi
}

INITIAL_MEM=$(get_memory_mb)
echo "Initial memory: ${INITIAL_MEM}MB"

# Run inference
PROMPT="Write a short paragraph about memory management."
OUTPUT=$("$LLM_NODE" \
  --model "$MODEL" \
  --n-predict 100 \
  --prompt "$PROMPT" \
  2>&1) &
PID=$!

# Monitor memory during inference
MAX_MEM=0
while kill -0 $PID 2>/dev/null; do
  CURRENT_MEM=$(get_memory_mb)
  if [[ $CURRENT_MEM -gt $MAX_MEM ]]; then
    MAX_MEM=$CURRENT_MEM
  fi
  sleep 0.5
done

wait $PID || true

echo "Output: ${OUTPUT:0:200}..."
echo "Max memory during inference: ${MAX_MEM}MB"

# Save memory stats
echo "max_memory_mb: $MAX_MEM" > "$RESULTS_DIR/memory-stats.txt"
echo "initial_memory_mb: $INITIAL_MEM" >> "$RESULTS_DIR/memory-stats.txt"

# Memory test always passes - we just record the metrics
echo "PASS: Memory usage recorded"
exit 0
