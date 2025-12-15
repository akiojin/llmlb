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
PYTHON_BIN="${PYTHON_BIN:-python3}"
VENV_DIR="${VENV_DIR:-/tmp/llm_router_audio_poc_venv}"

ROUTER_HOST="${ROUTER_HOST:-127.0.0.1}"
ROUTER_PORT="${ROUTER_PORT:-18080}"
NODE_HOST="${NODE_HOST:-127.0.0.1}"
NODE_PORT="${NODE_PORT:-11435}"

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

ensure_python() {
  if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1; then
    echo "Error: python is required (PYTHON_BIN=${PYTHON_BIN} not found)" >&2
    exit 1
  fi
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

if [[ ! -d "${ORT_CMAKE_DIR}" ]]; then
  echo "==> CoreML-enabled onnxruntime not found at:" >&2
  echo "    ${ORT_CMAKE_DIR}" >&2
  echo "==> Building onnxruntime with CoreML EP enabled..." >&2
  "${ROOT_DIR}/scripts/build-onnxruntime-coreml.sh"
fi

ensure_venv_deps

echo "==> Generating toy TTS ONNX model"
TOY_TTS_MODEL_PATH="$("${PYTHON_RUNTIME}" "${POC_DIR}/generate_toy_tts_model.py" --out "${MODEL_DIR}/toy_tts.onnx")"
ls -lh "${TOY_TTS_MODEL_PATH}"

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
LLM_ROUTER_URL="http://${ROUTER_HOST}:${ROUTER_PORT}" \
LLM_ROUTER_API_KEY="sk_poc" \
LLM_NODE_MODELS_DIR="${MODEL_DIR}" \
LLM_NODE_PORT="${NODE_PORT}" \
LLM_NODE_BIND_ADDRESS="${NODE_HOST}" \
"${NODE_BUILD_DIR}/llm-node" &
NODE_PID=$!

echo "==> Waiting for node to accept requests..."
for _ in $(seq 1 60); do
  if curl -fsS "http://${NODE_HOST}:${NODE_PORT}/v1/models" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

if ! curl -fsS "http://${NODE_HOST}:${NODE_PORT}/v1/models" >/dev/null 2>&1; then
  echo "Error: node did not start listening on ${NODE_HOST}:${NODE_PORT}" >&2
  exit 1
fi

TEST_WAV="${ASR_WAV_PATH}"
if [[ -f "${TEST_WAV}" ]]; then
  echo "==> Using ASR sample WAV: ${TEST_WAV}"
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
ls -lh "${TEST_WAV}"

echo "==> [ASR input] POST /v1/audio/transcriptions"
ASR_LANG_ARGS=()
if [[ -n "${ASR_LANGUAGE}" && "${ASR_LANGUAGE}" != "auto" ]]; then
  ASR_LANG_ARGS=(-F "language=${ASR_LANGUAGE}")
fi

ASR_JSON="$(curl -fsS "http://${NODE_HOST}:${NODE_PORT}/v1/audio/transcriptions" \
  -F "file=@${TEST_WAV};type=audio/wav" \
  -F "model=${WHISPER_MODEL_NAME}" \
  "${ASR_LANG_ARGS[@]}")"
echo "${ASR_JSON}"

echo "==> [TTS output] POST /v1/audio/speech -> out.wav"
TTS_OUT="${MODEL_DIR}/tts_out.wav"
curl -fsS "http://${NODE_HOST}:${NODE_PORT}/v1/audio/speech" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"toy_tts.onnx\",\"input\":\"hello audio i/o poc\",\"voice\":\"default\",\"response_format\":\"wav\",\"speed\":1.0}" \
  --output "${TTS_OUT}"

if [[ ! -s "${TTS_OUT}" ]]; then
  echo "Error: TTS output is empty" >&2
  exit 1
fi

python3 - <<'PY' "${TTS_OUT}"
import sys
path = sys.argv[1]
with open(path, "rb") as f:
    head = f.read(4)
if head != b"RIFF":
    raise SystemExit(f"Not a WAV file: {path}")
print(path)
PY

ls -lh "${TTS_OUT}"
echo "OK: ASR(JSON) + TTS(WAV) round-trip succeeded."
