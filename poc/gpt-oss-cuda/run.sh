#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
PoC: gpt-oss-20b (CUDA / llama.cpp GGUF)

This PoC runs gpt-oss on NVIDIA GPU by using the GGUF variant (llama.cpp CUDA backend).

Env vars:
  HF_TOKEN              (optional) Hugging Face token
  MODEL_REPO            (default: ggml-org/gpt-oss-20b-GGUF)
  MODEL_FILENAME        (default: gpt-oss-20b-mxfp4.gguf)
  ROUTER_PORT           (default: 18080)
  NODE_PORT             (default: 11435)   # runtime_port = NODE_PORT - 1
  ROUTER_BIN            (default: target/debug/llm-router)
  NODE_BIN              (default: node/build/llm-node)

Request shaping:
  USER_MESSAGE          (default: Say hello in one short sentence.)
  SYSTEM_MESSAGE        (default: empty)
  MESSAGES_FILE         (optional) Path to JSON array: [{"role":"user","content":"..."}]
  REQUEST_FILE          (optional) Path to full OpenAI-compatible request JSON object.
                       If set, it is used as-is (model is injected/overridden).

Generation params:
  TEMPERATURE           (default: 0.2)
  MAX_TOKENS            (default: 64)
  SEED                  (default: 0)
  STREAM                (default: 0)  # 1 to use SSE streaming

Process control:
  KEEP_RUNNING          (default: 0)  # 1 to keep router/node running after completion
  LLM_NODE_LOG_LEVEL    (default: info)
  SHOW_MANIFEST         (default: 0)  # 1 to print model manifest.json for diagnostics

Examples:
  ./poc/gpt-oss-cuda/run.sh
  USER_MESSAGE='こんにちは！一文で挨拶して' SEED=1 ./poc/gpt-oss-cuda/run.sh
  MESSAGES_FILE=./poc/gpt-oss-cuda/messages.json ./poc/gpt-oss-cuda/run.sh
  REQUEST_FILE=./poc/gpt-oss-cuda/request.json ./poc/gpt-oss-cuda/run.sh
  STREAM=1 MAX_TOKENS=16 ./poc/gpt-oss-cuda/run.sh
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"

POC_ROOT="${POC_ROOT:-"$REPO_ROOT/tmp/poc-gptoss-cuda"}"
ROUTER_HOME="${ROUTER_HOME:-"$POC_ROOT/router-home"}"
NODE_HOME="${NODE_HOME:-"$POC_ROOT/node-home"}"

ROUTER_PORT="${ROUTER_PORT:-18080}"
NODE_PORT="${NODE_PORT:-11435}"
TEMPERATURE="${TEMPERATURE:-0.2}"
MAX_TOKENS="${MAX_TOKENS:-64}"
SEED="${SEED:-0}"
STREAM="${STREAM:-0}"
USER_MESSAGE="${USER_MESSAGE:-Say hello in one short sentence.}"
SYSTEM_MESSAGE="${SYSTEM_MESSAGE:-}"
KEEP_RUNNING="${KEEP_RUNNING:-0}"

ROUTER_BIN="${ROUTER_BIN:-"$REPO_ROOT/target/debug/llm-router"}"
NODE_BIN="${NODE_BIN:-"$REPO_ROOT/node/build/llm-node"}"

MODEL_REPO="${MODEL_REPO:-ggml-org/gpt-oss-20b-GGUF}"
MODEL_FILENAME="${MODEL_FILENAME:-gpt-oss-20b-mxfp4.gguf}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[ERROR] missing command: $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq
require_cmd git

if [[ ! -x "$ROUTER_BIN" ]]; then
  echo "[ERROR] Router binary not found: $ROUTER_BIN" >&2
  echo "        Build it with: cargo build -p llm-router" >&2
  exit 1
fi

if [[ ! -x "$NODE_BIN" ]]; then
  echo "[ERROR] Node binary not found: $NODE_BIN" >&2
  echo "        Build it with: cmake --build node/build" >&2
  exit 1
fi

mkdir -p "$POC_ROOT" "$ROUTER_HOME" "$NODE_HOME"

ROUTER_LOG="$POC_ROOT/router.log"
NODE_LOG="$POC_ROOT/node.log"

cleanup() {
  set +e
  if [[ "$KEEP_RUNNING" == "1" ]]; then
    echo "[INFO] KEEP_RUNNING=1; leaving router/node running"
    return 0
  fi
  if [[ -n "${NODE_PID:-}" ]]; then
    kill "$NODE_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${ROUTER_PID:-}" ]]; then
    kill "$ROUTER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "[INFO] Starting router on :$ROUTER_PORT (logs: $ROUTER_LOG)"
HOME="$ROUTER_HOME" \
  LLM_ROUTER_PORT="$ROUTER_PORT" \
  LLM_ROUTER_ADMIN_USERNAME="admin" \
  LLM_ROUTER_ADMIN_PASSWORD="test" \
  RUST_LOG="info" \
  "$ROUTER_BIN" >"$ROUTER_LOG" 2>&1 &
ROUTER_PID=$!

echo "[INFO] Waiting for router to become ready..."
router_ready=0
for _ in $(seq 1 200); do
  if curl -fsS -H "Authorization: Bearer sk_debug" "http://127.0.0.1:$ROUTER_PORT/v1/models" >/dev/null 2>&1; then
    router_ready=1
    break
  fi
  sleep 0.25
done
if [[ "$router_ready" -ne 1 ]]; then
  echo "[ERROR] router did not become ready (see $ROUTER_LOG)" >&2
  exit 1
fi

ROUTER_MODELS_DIR="$ROUTER_HOME/.llm-router/models"
mkdir -p "$ROUTER_MODELS_DIR"

echo "[INFO] Starting node on :$NODE_PORT (logs: $NODE_LOG)"
HOME="$NODE_HOME" \
  LLM_ROUTER_URL="http://127.0.0.1:$ROUTER_PORT" \
  LLM_NODE_PORT="$NODE_PORT" \
  LLM_NODE_MODELS_DIR="$ROUTER_MODELS_DIR" \
  LLM_NODE_API_KEY="sk_debug_node" \
  LLM_NODE_LOG_LEVEL="${LLM_NODE_LOG_LEVEL:-info}" \
  "$NODE_BIN" >"$NODE_LOG" 2>&1 &
NODE_PID=$!

echo "[INFO] Waiting for node to become ready..."
node_ready=0
for _ in $(seq 1 2000); do
  if curl -fsS "http://127.0.0.1:$NODE_PORT/startup" >/dev/null 2>&1; then
    node_ready=1
    break
  fi
  sleep 0.5
done
if [[ "$node_ready" -ne 1 ]]; then
  echo "[ERROR] node did not become ready (see $NODE_LOG)" >&2
  exit 1
fi

echo "[INFO] Approving node (required for routing)"
NODE_RUNTIME_PORT="$((NODE_PORT - 1))"
nodes_tmp="$POC_ROOT/nodes.json"
curl -fsS -H "Authorization: Bearer sk_debug_admin" "http://127.0.0.1:$ROUTER_PORT/v0/nodes" >"$nodes_tmp"
node_json="$(
  jq -c --arg ip "127.0.0.1" --argjson port "$NODE_RUNTIME_PORT" '
    map(select(.ip_address==$ip and .runtime_port==$port))
    | sort_by(.last_seen)
    | last
  ' "$nodes_tmp"
)"
node_id="$(echo "$node_json" | jq -r '.id // empty')"
node_status="$(echo "$node_json" | jq -r '.status // empty')"
if [[ -z "$node_id" || "$node_id" == "null" ]]; then
  echo "[ERROR] failed to find node in router registry" >&2
  cat "$nodes_tmp" | jq .
  exit 1
fi
if [[ "$node_status" == "pending" ]]; then
  approve_tmp="$POC_ROOT/approve.json"
  approve_code="$(
    curl -sS -o "$approve_tmp" -w "%{http_code}" -X POST "http://127.0.0.1:$ROUTER_PORT/v0/nodes/$node_id/approve" \
      -H "Authorization: Bearer sk_debug_admin"
  )"
  if [[ "$approve_code" =~ ^2 ]]; then
    cat "$approve_tmp" | jq .
  else
    echo "[ERROR] node approval failed (HTTP $approve_code)" >&2
    cat "$approve_tmp" | jq . || cat "$approve_tmp"
    exit 1
  fi
else
  echo "[INFO] node status is '$node_status'; skipping approval"
fi

echo "[INFO] Registering model (GGUF): $MODEL_REPO ($MODEL_FILENAME)"
register_tmp="$POC_ROOT/register.json"
register_code="$(
  curl -sS -o "$register_tmp" -w "%{http_code}" -X POST "http://127.0.0.1:$ROUTER_PORT/v0/models/register" \
    -H "Authorization: Bearer sk_debug_admin" \
    -H "Content-Type: application/json" \
    -d "{\"repo\":\"$MODEL_REPO\",\"format\":\"gguf\",\"filename\":\"$MODEL_FILENAME\"}"
)"
if [[ "$register_code" == "201" || "$register_code" == "200" ]]; then
  cat "$register_tmp" | jq .
else
  # idempotent: if already registered, continue.
  if cat "$register_tmp" | jq -e '.error? | test("already registered"; "i")' >/dev/null 2>&1; then
    echo "[INFO] Model already registered; continuing"
  else
    echo "[ERROR] model registration failed (HTTP $register_code)" >&2
    cat "$register_tmp" | jq . || cat "$register_tmp"
    exit 1
  fi
fi

MODEL_ID="$(echo "$MODEL_REPO" | tr '[:upper:]' '[:lower:]')"
echo "[INFO] Waiting for router to finish caching: $MODEL_ID"
router_cached=0
for i in $(seq 1 3600); do
  models_json="$(curl -fsS -H "Authorization: Bearer sk_debug" "http://127.0.0.1:$ROUTER_PORT/v1/models")"
  status="$(echo "$models_json" | jq -r --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .lifecycle_status // empty')"
  ready="$(echo "$models_json" | jq -r --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .ready // false')"
  if [[ "$status" == "registered" && "$ready" == "true" ]]; then
    router_cached=1
    break
  fi
  if [[ "$status" == "error" ]]; then
    echo "[ERROR] model caching failed (see $ROUTER_LOG)" >&2
    exit 1
  fi
  if ((i % 15 == 0)); then
    echo "[INFO] caching... status=$status ready=$ready"
  fi
  sleep 2
done
if [[ "$router_cached" -ne 1 ]]; then
  echo "[ERROR] model did not become ready in time (see $ROUTER_LOG)" >&2
  exit 1
fi

echo "[INFO] Waiting for node to pick up the model: $MODEL_ID"
node_has_model=0
for i in $(seq 1 2000); do
  if curl -fsS "http://127.0.0.1:$NODE_PORT/v1/models" | jq -e --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .id' >/dev/null 2>&1; then
    node_has_model=1
    break
  fi
  if ((i % 30 == 0)); then
    echo "[INFO] waiting for node model sync..."
  fi
  sleep 1
done
if [[ "$node_has_model" -ne 1 ]]; then
  echo "[ERROR] node did not pick up the model in time (see $NODE_LOG)" >&2
  exit 1
fi

echo "[INFO] Fetching model manifest for diagnostics..."
manifest_tmp="$POC_ROOT/manifest.json"
encoded_model_id="$(jq -rn --arg s "$MODEL_ID" '$s|@uri')"
manifest_code="$(
  curl -sS -o "$manifest_tmp" -w "%{http_code}" \
    -H "Authorization: Bearer sk_debug_node" \
    "http://127.0.0.1:$ROUTER_PORT/v0/models/registry/$encoded_model_id/manifest.json"
)"
if [[ "$manifest_code" =~ ^2 ]]; then
  manifest_runtimes="$(jq -r '[.files[]?.runtimes[]?] | unique | join(",")' "$manifest_tmp")"
  node_runtimes="$(echo "$node_json" | jq -r '.supported_runtimes | join(",")')"
  files_count="$(jq -r '.files | length' "$manifest_tmp")"
  echo "[INFO] manifest files=$files_count runtimes=${manifest_runtimes:-"(none)"}"
  echo "[INFO] node supported_runtimes=$node_runtimes"
  if [[ -n "${manifest_runtimes:-}" ]]; then
    supported=0
    IFS=',' read -r -a required_runtimes <<<"$manifest_runtimes"
    for rt in "${required_runtimes[@]}"; do
      if [[ ",$node_runtimes," == *",$rt,"* ]]; then
        supported=1
        break
      fi
    done
    if [[ "$supported" -ne 1 ]]; then
      echo "[WARN] node does not advertise required runtimes: $manifest_runtimes" >&2
    fi
  fi
  if [[ "${SHOW_MANIFEST:-0}" == "1" ]]; then
    cat "$manifest_tmp" | jq .
  fi
else
  echo "[WARN] failed to fetch manifest (HTTP $manifest_code)" >&2
  cat "$manifest_tmp" | jq . >&2 || cat "$manifest_tmp" >&2
fi

echo "[INFO] Building request body..."
request_tmp="$POC_ROOT/request.json"
if [[ -n "${REQUEST_FILE:-}" ]]; then
  jq -c --arg model "$MODEL_ID" '.model=$model' "$REQUEST_FILE" >"$request_tmp"
else
  if [[ -n "${MESSAGES_FILE:-}" ]]; then
    messages="$(jq -c '.' "$MESSAGES_FILE")"
  else
    messages="$(jq -c -n --arg system "$SYSTEM_MESSAGE" --arg user "$USER_MESSAGE" '
      [
        ($system | select(length > 0) | {role:"system", content:.}),
        {role:"user", content:$user}
      ]
      | map(select(. != null))
    ')"
  fi

  jq -c -n \
    --arg model "$MODEL_ID" \
    --argjson messages "$messages" \
    --arg temperature "$TEMPERATURE" \
    --arg max_tokens "$MAX_TOKENS" \
    --arg seed "$SEED" \
    --arg stream "$STREAM" \
    '{
      model: $model,
      messages: $messages,
      temperature: ($temperature | tonumber),
      max_tokens: ($max_tokens | tonumber),
      seed: ($seed | tonumber),
      stream: ($stream == "1")
    }' >"$request_tmp"
fi

echo "[INFO] Sending chat.completions to router..."
is_stream="$(jq -r '.stream // false' "$request_tmp")"
if [[ "$is_stream" == "true" ]]; then
  echo "[INFO] stream=true; printing SSE response as-is"
  set +e
  curl --fail-with-body -sS -N "http://127.0.0.1:$ROUTER_PORT/v1/chat/completions" \
    -H "Authorization: Bearer sk_debug" \
    -H "Content-Type: application/json" \
    --data-binary @"$request_tmp"
  curl_rc=$?
  set -e
  echo
  if [[ "$curl_rc" -ne 0 ]]; then
    echo "[ERROR] chat.completions (stream) failed (curl rc=$curl_rc)" >&2
    echo "[INFO] router log tail ($ROUTER_LOG):" >&2
    tail -n 200 "$ROUTER_LOG" >&2 || true
    echo "[INFO] node log tail ($NODE_LOG):" >&2
    tail -n 200 "$NODE_LOG" >&2 || true
    exit 1
  fi
else
  chat_tmp="$POC_ROOT/chat.json"
  chat_code="$(
    curl -sS -o "$chat_tmp" -w "%{http_code}" "http://127.0.0.1:$ROUTER_PORT/v1/chat/completions" \
      -H "Authorization: Bearer sk_debug" \
      -H "Content-Type: application/json" \
      --data-binary @"$request_tmp"
  )"

  if [[ "$chat_code" =~ ^2 ]]; then
    cat "$chat_tmp" | jq .
  else
    echo "[ERROR] chat.completions failed (HTTP $chat_code)" >&2
    cat "$chat_tmp" | jq . || cat "$chat_tmp"
    echo "[INFO] router log tail ($ROUTER_LOG):" >&2
    tail -n 200 "$ROUTER_LOG" >&2 || true
    echo "[INFO] node log tail ($NODE_LOG):" >&2
    tail -n 200 "$NODE_LOG" >&2 || true
    exit 1
  fi
fi

echo "[INFO] Done"
