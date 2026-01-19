#!/usr/bin/env bash
# Model Verification Suite
# Comprehensive test suite for verifying model compatibility with llm-router engines
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Default values
MODEL=""
FORMAT="safetensors"
CAPABILITY="TextGeneration"
PLATFORM="macos-metal"
ENGINE=""
RESULTS_DIR=""
CHAT_TEMPLATE=""
ALLM_BIN="${SCRIPT_DIR}/../../../node/build/llm-node"
LLAMA_CLI="${SCRIPT_DIR}/../../../node/third_party/llama.cpp/build/bin/llama-cli"
TEST_TIMEOUT_SEC="${TEST_TIMEOUT_SEC:-900}"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --model) MODEL="$2"; shift 2;;
    --format) FORMAT="$2"; shift 2;;
    --capability) CAPABILITY="$2"; shift 2;;
    --platform) PLATFORM="$2"; shift 2;;
    --chat-template) CHAT_TEMPLATE="$2"; shift 2;;
    --allm) ALLM_BIN="$2"; shift 2;;
    --results-dir) RESULTS_DIR="$2"; shift 2;;
    --timeout) TEST_TIMEOUT_SEC="$2"; shift 2;;
    -h|--help)
      echo "Usage: $0 --model <path> [options]"
      echo "Options:"
      echo "  --model       Path to model file (required)"
      echo "  --format      Model format: safetensors|gguf (default: safetensors)"
      echo "  --capability  Model capability: TextGeneration|Vision|Audio|Embedding|Reranker"
      echo "  --platform    Target platform: macos-metal|linux-cuda|windows-directml"
      echo "  --chat-template Chat prompt style for test 08: plain|chatml (optional)"
      echo "  --allm    Path to aLLM binary"
      echo "  --results-dir Directory to store results"
      echo "  --timeout    Per-test timeout seconds (default: ${TEST_TIMEOUT_SEC})"
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1;;
  esac
done

# Validate required arguments
if [[ -z "$MODEL" ]]; then
  echo "Error: --model is required"
  exit 1
fi

if [[ ! -f "$MODEL" ]]; then
  echo "Error: Model file not found: $MODEL"
  exit 1
fi

if ! [[ "$TEST_TIMEOUT_SEC" =~ ^[0-9]+$ ]] || [[ "$TEST_TIMEOUT_SEC" -le 0 ]]; then
  echo "Error: --timeout must be a positive integer (seconds)"
  exit 1
fi

if [[ "$FORMAT" == "safetensors" ]]; then
  if [[ ! -x "$ALLM_BIN" ]]; then
    echo "Error: aLLM not found or not executable: $ALLM_BIN"
    exit 1
  fi
else
  if [[ ! -x "$LLAMA_CLI" ]]; then
    echo "Error: llama-cli not found or not executable: $LLAMA_CLI"
    echo "Build it with: cmake --build node/third_party/llama.cpp/build"
    exit 1
  fi
fi

# Determine engine based on format
if [[ "$FORMAT" == "safetensors" ]]; then
  ENGINE="gptoss_cpp"
else
  ENGINE="llama_cpp"
fi

# Create results directory
if [[ -z "$RESULTS_DIR" ]]; then
  RESULTS_DIR="$SCRIPT_DIR/results/$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "$RESULTS_DIR"

# Export for test scripts
export MODEL FORMAT CAPABILITY PLATFORM ENGINE ALLM_BIN LLAMA_CLI RESULTS_DIR SCRIPT_DIR CHAT_TEMPLATE

echo "=============================================="
echo "       Model Verification Suite"
echo "=============================================="
echo "Model:      $MODEL"
echo "Format:     $FORMAT"
echo "Engine:     $ENGINE"
echo "Capability: $CAPABILITY"
echo "Platform:   $PLATFORM"
echo "Timeout:    ${TEST_TIMEOUT_SEC}s"
if [[ -n "$CHAT_TEMPLATE" ]]; then
  echo "Chat tmpl:  $CHAT_TEMPLATE"
fi
echo "Results:    $RESULTS_DIR"
echo "=============================================="
echo ""

# Collect test results
PASSED=0
FAILED=0
SKIPPED=0
declare -a RESULTS=()

# Run test and record result
run_test() {
  local test_script="$1"
  local test_name
  test_name="$(basename "$test_script" .sh)"

  echo -n "Running: $test_name ... "

  if run_with_timeout "$TEST_TIMEOUT_SEC" bash "$test_script" > "$RESULTS_DIR/${test_name}.log" 2>&1; then
    echo "✅ PASSED"
    RESULTS+=("$test_name:PASSED")
    ((PASSED++))
    return 0
  else
    local exit_code=$?
    if [[ $exit_code -eq 77 ]]; then
      echo "⏭️  SKIPPED"
      RESULTS+=("$test_name:SKIPPED")
      ((SKIPPED++))
    elif [[ $exit_code -eq 124 ]]; then
      echo "⏱️  TIMEOUT (see $RESULTS_DIR/${test_name}.log)"
      RESULTS+=("$test_name:TIMEOUT")
      ((FAILED++))
    else
      echo "❌ FAILED (see $RESULTS_DIR/${test_name}.log)"
      RESULTS+=("$test_name:FAILED")
      ((FAILED++))
    fi
    return $exit_code
  fi
}

run_with_timeout() {
  local timeout_sec="$1"
  shift

  if command -v timeout >/dev/null 2>&1; then
    timeout --preserve-status "${timeout_sec}s" "$@"
    return $?
  fi

  if command -v gtimeout >/dev/null 2>&1; then
    gtimeout --preserve-status "${timeout_sec}s" "$@"
    return $?
  fi

  "$@" &
  local cmd_pid=$!
  local start_ts
  start_ts="$(date +%s)"

  while kill -0 "$cmd_pid" >/dev/null 2>&1; do
    local now_ts
    now_ts="$(date +%s)"
    if (( now_ts - start_ts >= timeout_sec )); then
      kill -TERM "$cmd_pid" >/dev/null 2>&1 || true
      sleep 2
      kill -KILL "$cmd_pid" >/dev/null 2>&1 || true
      wait "$cmd_pid" >/dev/null 2>&1 || true
      return 124
    fi
    sleep 1
  done

  wait "$cmd_pid"
  return $?
}

# Run all tests in order
for test_script in "$SCRIPT_DIR/tests/"*.sh; do
  if [[ -f "$test_script" ]]; then
    base_name="$(basename "$test_script")"
    if [[ "$base_name" == _* ]]; then
      continue
    fi
    run_test "$test_script" || true
  fi
done

# Generate summary
echo ""
echo "=============================================="
echo "              Test Summary"
echo "=============================================="
echo "Passed:  $PASSED"
echo "Failed:  $FAILED"
echo "Skipped: $SKIPPED"
echo "Total:   $((PASSED + FAILED + SKIPPED))"
echo ""

# Write results file
{
  echo "# Verification Results"
  echo ""
  echo "- Model: $MODEL"
  echo "- Format: $FORMAT"
  echo "- Engine: $ENGINE"
  echo "- Capability: $CAPABILITY"
  echo "- Platform: $PLATFORM"
  echo "- Date: $(date -Iseconds)"
  echo ""
  echo "## Results"
  echo ""
  for result in "${RESULTS[@]}"; do
    IFS=':' read -r name status <<< "$result"
    case $status in
      PASSED) echo "- ✅ $name";;
      FAILED) echo "- ❌ $name";;
      SKIPPED) echo "- ⏭️  $name";;
      TIMEOUT) echo "- ⏱️  $name";;
    esac
  done
} > "$RESULTS_DIR/summary.md"

# Final result
if [[ $FAILED -eq 0 ]]; then
  echo "✅ All required tests passed!"
  exit 0
else
  echo "❌ Some tests failed"
  exit 1
fi
