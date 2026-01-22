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
  "$@" &
  local pid=$!
  local elapsed=0
  while kill -0 "$pid" >/dev/null 2>&1; do
    if [[ "$elapsed" -ge "$seconds" ]]; then
      kill -9 "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
      return 124
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  wait "$pid"
}

run_xllm() {
  local timeout_secs="${VERIFY_TIMEOUT_SECS:-120}"
  if [[ -z "${XLLM_BIN:-}" ]]; then
    echo "FAIL: XLLM_BIN is not set" >&2
    exit 1
  fi
  if [[ ! -x "$XLLM_BIN" ]]; then
    echo "FAIL: aLLM not found: $XLLM_BIN" >&2
    exit 1
  fi
  run_with_timeout "$timeout_secs" "$XLLM_BIN" "$@"
}

infer_command() {
  local n_predict="$1"
  local prompt="$2"
  shift 2
  local timeout_secs="${VERIFY_TIMEOUT_SECS:-120}"

  if [[ "${FORMAT:-}" == "gguf" ]]; then
    if [[ -z "${LLAMA_CLI:-}" ]]; then
      echo "FAIL: LLAMA_CLI is not set" >&2
      exit 1
    fi
    if [[ ! -x "$LLAMA_CLI" ]]; then
      echo "FAIL: llama-cli not found: $LLAMA_CLI" >&2
      exit 1
    fi
    run_with_timeout "$timeout_secs" "$LLAMA_CLI" -m "$MODEL" -n "$n_predict" -p "$prompt" --single-turn "$@"
    return $?
  fi

  run_xllm --model "$MODEL" --n-predict "$n_predict" --prompt "$prompt" "$@"
}

format_chat_prompt() {
  local template="${CHAT_TEMPLATE:-plain}"
  local system_prompt="${CHAT_SYSTEM_PROMPT:-You are a helpful assistant.}"
  local user_prompt_1="${CHAT_USER_PROMPT_1:-Hello.}"
  local assistant_reply_1="${CHAT_ASSISTANT_REPLY_1:-Hello!}"
  local user_prompt_2="${CHAT_USER_PROMPT_2:-What is 1+1?}"

  case "$template" in
    chatml|gpt-oss)
      printf "<|im_start|>system\n%s\n<|im_end|>\n<|im_start|>user\n%s\n<|im_end|>\n<|im_start|>assistant\n%s\n<|im_end|>\n<|im_start|>user\n%s\n<|im_end|>\n<|im_start|>assistant\n" \
        "$system_prompt" "$user_prompt_1" "$assistant_reply_1" "$user_prompt_2"
      ;;
    plain|*)
      printf "System: %s\nUser: %s\nAssistant: %s\nUser: %s\nAssistant:" \
        "$system_prompt" "$user_prompt_1" "$assistant_reply_1" "$user_prompt_2"
      ;;
  esac
}
