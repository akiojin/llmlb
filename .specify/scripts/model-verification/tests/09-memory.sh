#!/usr/bin/env bash
# Test: Memory Usage
# Measures memory consumption during inference
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/_helpers.sh"

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
OUTPUT_FILE="$RESULTS_DIR/memory-output.txt"
timeout_secs="${VERIFY_TIMEOUT_SECS:-120}"
max_wait_secs=$((timeout_secs + 5))

infer_command 100 "$PROMPT" >"$OUTPUT_FILE" 2>&1 &
PID=$!

# Monitor memory during inference
MAX_MEM=0
START_TIME=$(date +%s)
while kill -0 $PID 2>/dev/null; do
  CURRENT_MEM=$(get_memory_mb)
  if [[ $CURRENT_MEM -gt $MAX_MEM ]]; then
    MAX_MEM=$CURRENT_MEM
  fi
  ELAPSED=$(( $(date +%s) - START_TIME ))
  if [[ $ELAPSED -ge $max_wait_secs ]]; then
    kill -9 "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
    break
  fi
  sleep 0.5
done

wait $PID || true

OUTPUT=""
if [[ -f "$OUTPUT_FILE" ]]; then
  OUTPUT=$(head -c 400 "$OUTPUT_FILE" 2>/dev/null || true)
fi
echo "Output: ${OUTPUT}..."
echo "Max memory during inference: ${MAX_MEM}MB"

# Save memory stats
echo "max_memory_mb: $MAX_MEM" > "$RESULTS_DIR/memory-stats.txt"
echo "initial_memory_mb: $INITIAL_MEM" >> "$RESULTS_DIR/memory-stats.txt"

# Memory test always passes - we just record the metrics
echo "PASS: Memory usage recorded"
exit 0
