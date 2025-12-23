#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"

POC_ROOT="${POC_ROOT:-"$REPO_ROOT/tmp/poc-gptoss-metal"}"
ROUTER_HOME="${ROUTER_HOME:-"$POC_ROOT/router-home"}"
NODE_HOME="${NODE_HOME:-"$POC_ROOT/node-home"}"

ROUTER_PORT="${ROUTER_PORT:-18080}"
NODE_PORT="${NODE_PORT:-11435}"
TEMPERATURE="${TEMPERATURE:-0.2}"
MAX_TOKENS="${MAX_TOKENS:-64}"
SEED="${SEED:-0}"

ROUTER_BIN="${ROUTER_BIN:-"$REPO_ROOT/target/debug/llm-router"}"
NODE_BIN="${NODE_BIN:-"$REPO_ROOT/node/build/llm-node"}"

MODEL_REPO="${MODEL_REPO:-openai/gpt-oss-20b}"
MODEL_FILENAME="${MODEL_FILENAME:-model.safetensors.index.json}"

if [[ -z "${HF_TOKEN:-}" ]]; then
  echo "[ERROR] HF_TOKEN is not set" >&2
  exit 1
fi

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
node_id="$(cat "$nodes_tmp" | jq -r --arg ip "127.0.0.1" --argjson port "$NODE_RUNTIME_PORT" '.[] | select(.ip_address==$ip and .runtime_port==$port) | .id' | head -n 1)"
if [[ -z "$node_id" || "$node_id" == "null" ]]; then
  echo "[ERROR] failed to find node in router registry" >&2
  cat "$nodes_tmp" | jq .
  exit 1
fi
curl -fsS -X POST "http://127.0.0.1:$ROUTER_PORT/v0/nodes/$node_id/approve" \
  -H "Authorization: Bearer sk_debug_admin" | jq .

echo "[INFO] Registering model: $MODEL_REPO ($MODEL_FILENAME)"
register_tmp="$POC_ROOT/register.json"
register_code="$(
  curl -sS -o "$register_tmp" -w "%{http_code}" -X POST "http://127.0.0.1:$ROUTER_PORT/v0/models/register" \
    -H "Authorization: Bearer sk_debug_admin" \
    -H "Content-Type: application/json" \
    -d "{\"repo\":\"$MODEL_REPO\",\"format\":\"safetensors\",\"filename\":\"$MODEL_FILENAME\"}"
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
for _ in $(seq 1 20000); do
  models_json="$(curl -fsS -H "Authorization: Bearer sk_debug" "http://127.0.0.1:$ROUTER_PORT/v1/models")"
  status="$(echo "$models_json" | jq -r --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .lifecycle_status // empty')"
  ready="$(echo "$models_json" | jq -r --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .ready // false')"
  if [[ "$status" == "registered" && "$ready" == "true" ]]; then
    break
  fi
  if [[ "$status" == "error" ]]; then
    echo "[ERROR] model caching failed (see $ROUTER_LOG)" >&2
    exit 1
  fi
  sleep 2
done

echo "[INFO] Waiting for node to pick up the model: $MODEL_ID"
for _ in $(seq 1 2000); do
  if curl -fsS "http://127.0.0.1:$NODE_PORT/v1/models" | jq -e --arg id "$MODEL_ID" '.data[] | select(.id==$id) | .id' >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

echo "[INFO] Sending chat.completions to router..."
curl -fsS "http://127.0.0.1:$ROUTER_PORT/v1/chat/completions" \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"$MODEL_ID\",
    \"messages\": [{\"role\":\"user\",\"content\":\"Say hello in one short sentence.\"}],
    \"temperature\": $TEMPERATURE,
    \"max_tokens\": $MAX_TOKENS,
    \"seed\": $SEED,
    \"stream\": false
  }" | jq .

echo "[INFO] Done"
