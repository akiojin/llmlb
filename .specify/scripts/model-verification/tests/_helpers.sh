#!/usr/bin/env bash
set -euo pipefail

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "FAIL: missing command: $1" >&2
    exit 1
  fi
}

resolve_timeout_bin() {
  if command -v timeout >/dev/null 2>&1; then
    echo "timeout"
    return 0
  fi
  if command -v gtimeout >/dev/null 2>&1; then
    echo "gtimeout"
    return 0
  fi
  echo ""
}

run_with_timeout() {
  local seconds="$1"
  shift
  local timeout_bin
  timeout_bin="$(resolve_timeout_bin)"
  if [[ -n "$timeout_bin" ]]; then
    "$timeout_bin" "$seconds" "$@"
    return $?
  fi
  "$@"
}

infer_command() {
  local n_predict="$1"
  local prompt="$2"
  shift 2

  if [[ "${FORMAT:-}" == "gguf" ]]; then
    if [[ -z "${LLAMA_CLI:-}" ]]; then
      echo "FAIL: LLAMA_CLI is not set" >&2
      exit 1
    fi
    if [[ ! -x "$LLAMA_CLI" ]]; then
      echo "FAIL: llama-cli not found: $LLAMA_CLI" >&2
      exit 1
    fi
    "$LLAMA_CLI" -m "$MODEL" -n "$n_predict" -p "$prompt" "$@"
    return $?
  fi

  if [[ -z "${LLM_NODE:-}" ]]; then
    echo "FAIL: LLM_NODE is not set" >&2
    exit 1
  fi
  if [[ ! -x "$LLM_NODE" ]]; then
    echo "FAIL: llm-node not found: $LLM_NODE" >&2
    exit 1
  fi
  "$LLM_NODE" --model "$MODEL" --n-predict "$n_predict" --prompt "$prompt" "$@"
}
