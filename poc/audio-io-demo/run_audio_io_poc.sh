#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
POC_DIR="${ROOT_DIR}/poc/audio-io-demo"

ORT_CMAKE_DIR="${ORT_CMAKE_DIR:-/tmp/onnxruntime-coreml/install/lib/cmake/onnxruntime}"
NODE_BUILD_DIR="${NODE_BUILD_DIR:-/tmp/llm_node_build_audio_poc}"
MODEL_DIR="${MODEL_DIR:-/tmp/llm_router_audio_poc_models}"
WHISPER_MODEL_NAME="${WHISPER_MODEL_NAME:-ggml-tiny.en.bin}"
ASR_WAV_PATH="${ASR_WAV_PATH:-${ROOT_DIR}/node/third_party/whisper.cpp/samples/jfk.wav}"
ASR_LANGUAGE="${ASR_LANGUAGE:-en}"
TTS_MODEL="${TTS_MODEL:-macos_say}"
TTS_TEXT="${TTS_TEXT:-}"
TTS_VOICE="${TTS_VOICE:-default}"
VIBEVOICE_VENV_DIR="${VIBEVOICE_VENV_DIR:-/tmp/llm_router_vibevoice_poc_venv}"
VIBEVOICE_DEVICE="${VIBEVOICE_DEVICE:-mps}"
VIBEVOICE_MODEL_ID="${VIBEVOICE_MODEL_ID:-microsoft/VibeVoice-Realtime-0.5B}"
PYTHON_BIN="${PYTHON_BIN:-python3}"
VENV_DIR="${VENV_DIR:-/tmp/llm_router_audio_poc_venv}"
PLAY_TTS="${PLAY_TTS:-1}"

ROUTER_HOST="${ROUTER_HOST:-127.0.0.1}"
ROUTER_PORT_ENV_SET=0
if [[ -n "${ROUTER_PORT+x}" ]]; then
  ROUTER_PORT_ENV_SET=1
fi
ROUTER_PORT="${ROUTER_PORT:-18080}"
NODE_HOST="${NODE_HOST:-127.0.0.1}"
NODE_PORT_ENV_SET=0
if [[ -n "${NODE_PORT+x}" ]]; then
  NODE_PORT_ENV_SET=1
fi
NODE_PORT="${NODE_PORT:-11435}"

pick_free_port() {
  local start_port="${1}"
  local max_tries=50
  local port="${start_port}"

  if ! command -v lsof >/dev/null 2>&1; then
    echo "Error: lsof is required to check port availability on macOS." >&2
    exit 1
  fi

  for _ in $(seq 1 "${max_tries}"); do
    if ! lsof -iTCP:"${port}" -sTCP:LISTEN -n -P >/dev/null 2>&1; then
      echo "${port}"
      return 0
    fi
    port=$((port + 1))
  done

  echo "Error: could not find a free port starting from ${start_port}" >&2
  exit 1
}

cleanup() {
  set +e
  if [[ -n "${NODE_PID:-}" ]]; then
    kill "${NODE_PID}" >/dev/null 2>&1 || true
    wait "${NODE_PID}" >/dev/null 2>&1 || true
  fi
  if [[ -n "${ROUTER_PID:-}" ]]; then
    kill "${ROUTER_PID}" >/dev/null 2>&1 || true
    wait "${ROUTER_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Error: this PoC currently targets macOS (Apple Silicon) only." >&2
  exit 1
fi

mkdir -p "${MODEL_DIR}"

PYTHON_RUNTIME="${PYTHON_BIN}"
VIBEVOICE_PYTHON_RUNTIME="${PYTHON_BIN}"

ensure_python() {
  if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1; then
    echo "Error: python is required (PYTHON_BIN=${PYTHON_BIN} not found)" >&2
    exit 1
  fi
}

is_vibevoice() {
  [[ "${TTS_MODEL}" == "vibevoice" || "${TTS_MODEL}" == "microsoft/VibeVoice-Realtime-0.5B" ]]
}

ensure_venv_deps() {
  ensure_python

  if [[ ! -x "${VENV_DIR}/bin/python" ]]; then
    echo "==> Creating python venv: ${VENV_DIR}"
    "${PYTHON_BIN}" -m venv "${VENV_DIR}"
  fi

  PYTHON_RUNTIME="${VENV_DIR}/bin/python"

  if ! "${PYTHON_RUNTIME}" -c "import onnx, numpy" >/dev/null 2>&1; then
    echo "==> Installing python deps into venv (onnx/numpy)"
    "${PYTHON_RUNTIME}" -m pip install --quiet --upgrade pip >/dev/null 2>&1 || true
    "${PYTHON_RUNTIME}" -m pip install --quiet onnx numpy
  fi
}

ensure_vibevoice_venv_deps() {
  ensure_python

  if [[ ! -x "${VIBEVOICE_VENV_DIR}/bin/python" ]]; then
    echo "==> Creating VibeVoice python venv: ${VIBEVOICE_VENV_DIR}"
    "${PYTHON_BIN}" -m venv "${VIBEVOICE_VENV_DIR}"
  fi

  VIBEVOICE_PYTHON_RUNTIME="${VIBEVOICE_VENV_DIR}/bin/python"

  if ! "${VIBEVOICE_PYTHON_RUNTIME}" -c "import torch, transformers, diffusers, accelerate, numpy, soundfile, vibevoice" >/dev/null 2>&1; then
    echo "==> Installing VibeVoice python deps into venv (this can take a while)"
    "${VIBEVOICE_PYTHON_RUNTIME}" -m pip install --quiet --upgrade pip >/dev/null 2>&1 || true
    "${VIBEVOICE_PYTHON_RUNTIME}" -m pip install --quiet -r "${ROOT_DIR}/poc/vibevoice-pytorch/requirements.txt"
    "${VIBEVOICE_PYTHON_RUNTIME}" -m pip install --quiet --no-deps vibevoice==0.0.1
  fi
}

if [[ ! -d "${ORT_CMAKE_DIR}" ]]; then
  echo "==> CoreML-enabled onnxruntime not found at:" >&2
  echo "    ${ORT_CMAKE_DIR}" >&2
  echo "==> Building onnxruntime with CoreML EP enabled..." >&2
  "${ROOT_DIR}/scripts/build-onnxruntime-coreml.sh"
fi

ensure_python

if [[ "${TTS_MODEL}" == "toy_tts.onnx" ]]; then
  ensure_venv_deps
  echo "==> Generating toy TTS ONNX model"
  TOY_TTS_MODEL_PATH="$("${PYTHON_RUNTIME}" "${POC_DIR}/generate_toy_tts_model.py" --out "${MODEL_DIR}/toy_tts.onnx")"
  ls -lh "${TOY_TTS_MODEL_PATH}"
elif is_vibevoice; then
  ensure_vibevoice_venv_deps
else
  # macos_say is a built-in TTS backend in the node (no model file required).
  if [[ "${TTS_MODEL}" != "macos_say" ]]; then
    if [[ "${TTS_MODEL}" = /* ]]; then
      if [[ ! -f "${TTS_MODEL}" ]]; then
        echo "Error: TTS_MODEL not found: ${TTS_MODEL}" >&2
        exit 1
      fi
    else
      if [[ ! -f "${MODEL_DIR}/${TTS_MODEL}" ]]; then
        echo "Error: TTS model file not found in MODEL_DIR: ${MODEL_DIR}/${TTS_MODEL}" >&2
        echo "Hint: set TTS_MODEL=macos_say (macOS built-in) or TTS_MODEL=toy_tts.onnx (toy model)." >&2
        exit 1
      fi
    fi
  fi
fi

if lsof -iTCP:"${ROUTER_PORT}" -sTCP:LISTEN -n -P >/dev/null 2>&1; then
  if [[ "${ROUTER_PORT_ENV_SET}" -eq 1 ]]; then
    echo "Error: ROUTER_PORT=${ROUTER_PORT} is already in use." >&2
    echo "Hint: set ROUTER_PORT to a free port (example: ROUTER_PORT=28080)." >&2
    exit 1
  fi
  NEW_ROUTER_PORT="$(pick_free_port "${ROUTER_PORT}")"
  echo "==> ROUTER_PORT=${ROUTER_PORT} is already in use; using ROUTER_PORT=${NEW_ROUTER_PORT}"
  ROUTER_PORT="${NEW_ROUTER_PORT}"
fi

if lsof -iTCP:"${NODE_PORT}" -sTCP:LISTEN -n -P >/dev/null 2>&1; then
  if [[ "${NODE_PORT_ENV_SET}" -eq 1 ]]; then
    echo "Error: NODE_PORT=${NODE_PORT} is already in use." >&2
    echo "Hint: set NODE_PORT to a free port (example: NODE_PORT=11445)." >&2
    exit 1
  fi
  NEW_NODE_PORT="$(pick_free_port "${NODE_PORT}")"
  echo "==> NODE_PORT=${NODE_PORT} is already in use; using NODE_PORT=${NEW_NODE_PORT}"
  NODE_PORT="${NEW_NODE_PORT}"
fi

echo "==> Downloading whisper model (${WHISPER_MODEL_NAME}) if missing"
if [[ ! -f "${MODEL_DIR}/${WHISPER_MODEL_NAME}" ]]; then
  # Use whisper.cpp's official downloader script (HF mirror).
  "${ROOT_DIR}/node/third_party/whisper.cpp/models/download-ggml-model.sh" tiny.en "${MODEL_DIR}"
fi
ls -lh "${MODEL_DIR}/${WHISPER_MODEL_NAME}"

echo "==> Building llm-node (with CoreML-enabled onnxruntime)"
cmake -S "${ROOT_DIR}/node" -B "${NODE_BUILD_DIR}" -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTS=OFF -Donnxruntime_DIR="${ORT_CMAKE_DIR}"
cmake --build "${NODE_BUILD_DIR}" -j

echo "==> Starting mock router"
"${PYTHON_RUNTIME}" "${POC_DIR}/mock_router.py" --host "${ROUTER_HOST}" --port "${ROUTER_PORT}" &
ROUTER_PID=$!

echo "==> Starting llm-node"
echo "==> Node URL: http://${NODE_HOST}:${NODE_PORT}"
NODE_ENV=(
  "LLM_ROUTER_URL=http://${ROUTER_HOST}:${ROUTER_PORT}"
  "LLM_ROUTER_API_KEY=sk_poc"
  "LLM_NODE_MODELS_DIR=${MODEL_DIR}"
  "LLM_NODE_PORT=${NODE_PORT}"
  "LLM_NODE_BIND_ADDRESS=${NODE_HOST}"
)
if is_vibevoice; then
  # node の /v1/audio/speech が VibeVoice 推論を呼び出せるように、python runner を渡す。
  NODE_ENV+=(
    "LLM_NODE_VIBEVOICE_PYTHON=${VIBEVOICE_PYTHON_RUNTIME}"
    "LLM_NODE_VIBEVOICE_RUNNER=${ROOT_DIR}/poc/vibevoice-pytorch/run.py"
    "LLM_NODE_VIBEVOICE_DEVICE=${VIBEVOICE_DEVICE}"
    "LLM_NODE_VIBEVOICE_MODEL=${VIBEVOICE_MODEL_ID}"
  )
fi

env "${NODE_ENV[@]}" "${NODE_BUILD_DIR}/llm-node" &
NODE_PID=$!

echo "==> Waiting for node to accept requests..."
for _ in $(seq 1 60); do
  if curl -fsS "http://${NODE_HOST}:${NODE_PORT}/startup" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

if ! curl -fsS "http://${NODE_HOST}:${NODE_PORT}/startup" >/dev/null 2>&1; then
  echo "Error: node did not become ready on ${NODE_HOST}:${NODE_PORT}" >&2
  echo "Hint: check node log: ~/.llm-router/logs/llm-node.jsonl.*" >&2
  exit 1
fi

TEST_WAV="${ASR_WAV_PATH}"
if [[ -f "${TEST_WAV}" ]]; then
  echo "==> Using ASR input audio: ${TEST_WAV}"
else
  echo "==> ASR sample not found, generating test WAV (16kHz, 16-bit PCM, 1s sine)"
  TEST_WAV="${MODEL_DIR}/asr_test.wav"
  "${PYTHON_RUNTIME}" - <<'PY' "${TEST_WAV}"
import math
import struct
import sys
import wave

out = sys.argv[1]
sr = 16000
secs = 1.0
freq = 440.0
amp = 0.2
n = int(sr * secs)

with wave.open(out, "wb") as wf:
    wf.setnchannels(1)
    wf.setsampwidth(2)
    wf.setframerate(sr)
    for i in range(n):
        v = amp * math.sin(2.0 * math.pi * freq * (i / sr))
        wf.writeframes(struct.pack("<h", int(max(-1.0, min(1.0, v)) * 32767)))
print(out)
PY
fi

# Normalize ASR input to a format the node currently supports: WAV (16kHz, mono, 16-bit PCM).
ASR_INPUT_WAV="${MODEL_DIR}/asr_input.wav"
if ! command -v afconvert >/dev/null 2>&1; then
  echo "Error: afconvert is required on macOS to normalize ASR input audio." >&2
  exit 1
fi
echo "==> Normalizing ASR input to WAV (16kHz mono 16-bit): ${ASR_INPUT_WAV}"
afconvert -f WAVE -d LEI16@16000 -c 1 "${TEST_WAV}" "${ASR_INPUT_WAV}"
TEST_WAV="${ASR_INPUT_WAV}"
ls -lh "${TEST_WAV}"

echo "==> [ASR input] POST /v1/audio/transcriptions"
ASR_LANG_ARGS=()
if [[ -n "${ASR_LANGUAGE}" && "${ASR_LANGUAGE}" != "auto" ]]; then
  ASR_LANG_ARGS=(-F "language=${ASR_LANGUAGE}")
fi

ASR_TMP="${MODEL_DIR}/asr_response.json"
ASR_STATUS="$(curl -sS "http://${NODE_HOST}:${NODE_PORT}/v1/audio/transcriptions" \
  -F "file=@${TEST_WAV};type=audio/wav" \
  -F "model=${WHISPER_MODEL_NAME}" \
  "${ASR_LANG_ARGS[@]}" \
  -o "${ASR_TMP}" \
  -w "%{http_code}")"

ASR_OK=1
if [[ "${ASR_STATUS}" != "200" ]]; then
  ASR_OK=0
  echo "ASR failed: status=${ASR_STATUS}" >&2
  cat "${ASR_TMP}" >&2 || true
else
  cat "${ASR_TMP}"
fi

ASR_TEXT=""
if [[ "${ASR_OK}" -eq 1 ]]; then
  ASR_TEXT="$("${PYTHON_RUNTIME}" - <<'PY' "${ASR_TMP}"
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)
print(data.get("text", ""))
PY
)"
fi

TTS_TEXT_EFFECTIVE="${TTS_TEXT}"
if [[ -z "${TTS_TEXT_EFFECTIVE}" && "${ASR_OK}" -eq 1 ]]; then
  TTS_TEXT_EFFECTIVE="${ASR_TEXT}"
fi
if [[ -z "${TTS_TEXT_EFFECTIVE}" ]]; then
  TTS_TEXT_EFFECTIVE="hello from llm-router audio io poc"
fi

TTS_MODEL_REQUEST="${TTS_MODEL}"
if [[ "${TTS_MODEL}" == "vibevoice" ]]; then
  TTS_MODEL_REQUEST="${VIBEVOICE_MODEL_ID}"
fi

echo "==> TTS model: ${TTS_MODEL_REQUEST} (voice=${TTS_VOICE})"
echo "==> TTS input text: ${TTS_TEXT_EFFECTIVE}"

TTS_REQ_JSON="${MODEL_DIR}/tts_request.json"
"${PYTHON_RUNTIME}" - <<'PY' "${TTS_REQ_JSON}" "${TTS_MODEL_REQUEST}" "${TTS_TEXT_EFFECTIVE}" "${TTS_VOICE}"
import json
import sys

out_path, model, text, voice = sys.argv[1:5]
payload = {
    "model": model,
    "input": text,
    "voice": voice,
    "response_format": "wav",
    "speed": 1.0,
}
with open(out_path, "w", encoding="utf-8") as f:
    json.dump(payload, f, ensure_ascii=False)
PY

echo "==> [TTS output] POST /v1/audio/speech -> out.wav"
TTS_OUT="${MODEL_DIR}/tts_out.wav"
TTS_STATUS="$(curl -sS "http://${NODE_HOST}:${NODE_PORT}/v1/audio/speech" \
  -H "Content-Type: application/json" \
  --data-binary "@${TTS_REQ_JSON}" \
  --output "${TTS_OUT}" \
  -w "%{http_code}")"

if [[ "${TTS_STATUS}" != "200" ]]; then
  echo "TTS failed: status=${TTS_STATUS}" >&2
  cat "${TTS_OUT}" >&2 || true
  exit 1
fi

if [[ ! -s "${TTS_OUT}" ]]; then
  echo "Error: TTS output is empty" >&2
  exit 1
fi

"${PYTHON_RUNTIME}" - <<'PY' "${TTS_OUT}"
import sys
path = sys.argv[1]
with open(path, "rb") as f:
    head = f.read(4)
if head != b"RIFF":
    raise SystemExit(f"Not a WAV file: {path}")
print(path)
PY

ls -lh "${TTS_OUT}"
echo "To play (macOS):"
echo "  afplay \"${TTS_OUT}\""

if [[ "${PLAY_TTS}" == "1" ]]; then
  if command -v afplay >/dev/null 2>&1; then
    afplay "${TTS_OUT}" || true
  else
    echo "Warning: afplay not found; cannot auto-play." >&2
  fi
fi

if [[ "${ASR_OK}" -ne 1 ]]; then
  exit 1
fi
echo "OK: ASR(JSON) + TTS(WAV) round-trip succeeded."
