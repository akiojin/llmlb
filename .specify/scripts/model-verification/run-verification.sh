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
LLM_NODE="${SCRIPT_DIR}/../../../node/build/llm-node"
LLAMA_CLI="${SCRIPT_DIR}/../../../node/third_party/llama.cpp/build/bin/llama-cli"

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --model) MODEL="$2"; shift 2;;
    --format) FORMAT="$2"; shift 2;;
    --capability) CAPABILITY="$2"; shift 2;;
    --platform) PLATFORM="$2"; shift 2;;
    --llm-node) LLM_NODE="$2"; shift 2;;
    --results-dir) RESULTS_DIR="$2"; shift 2;;
    -h|--help)
      echo "Usage: $0 --model <path> [options]"
      echo "Options:"
      echo "  --model       Path to model file (required)"
      echo "  --format      Model format: safetensors|gguf (default: safetensors)"
      echo "  --capability  Model capability: TextGeneration|Vision|Audio|Embedding|Reranker"
      echo "  --platform    Target platform: macos-metal|linux-cuda|windows-directml"
      echo "  --llm-node    Path to llm-node binary"
      echo "  --results-dir Directory to store results"
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

if [[ "$FORMAT" == "safetensors" ]]; then
  if [[ ! -x "$LLM_NODE" ]]; then
    echo "Error: llm-node not found or not executable: $LLM_NODE"
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
export MODEL FORMAT CAPABILITY PLATFORM ENGINE LLM_NODE LLAMA_CLI RESULTS_DIR SCRIPT_DIR

echo "=============================================="
echo "       Model Verification Suite"
echo "=============================================="
echo "Model:      $MODEL"
echo "Format:     $FORMAT"
echo "Engine:     $ENGINE"
echo "Capability: $CAPABILITY"
echo "Platform:   $PLATFORM"
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

  if bash "$test_script" > "$RESULTS_DIR/${test_name}.log" 2>&1; then
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
    else
      echo "❌ FAILED (see $RESULTS_DIR/${test_name}.log)"
      RESULTS+=("$test_name:FAILED")
      ((FAILED++))
    fi
    return $exit_code
  fi
}

# Run all tests in order
for test_script in "$SCRIPT_DIR/tests/"*.sh; do
  if [[ -f "$test_script" ]]; then
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
