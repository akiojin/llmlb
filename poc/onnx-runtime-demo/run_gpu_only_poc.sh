#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
POC_DIR="${ROOT_DIR}/poc/onnx-runtime-demo"

ORT_CMAKE_DIR="${ORT_CMAKE_DIR:-/tmp/onnxruntime-coreml/install/lib/cmake/onnxruntime}"
BUILD_DIR="${BUILD_DIR:-${POC_DIR}/build}"
MODEL_DIR="${MODEL_DIR:-/tmp/onnx_poc_models}"
PYTHON_BIN="${PYTHON_BIN:-python3}"
VENV_DIR="${VENV_DIR:-/tmp/llm_router_onnx_poc_venv}"

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

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Error: this PoC currently targets macOS + CoreML EP only." >&2
  exit 1
fi

if [[ ! -d "${ORT_CMAKE_DIR}" ]]; then
  echo "==> CoreML-enabled onnxruntime not found at:" >&2
  echo "    ${ORT_CMAKE_DIR}" >&2
  echo "==> Building onnxruntime with CoreML EP enabled (this can take a while)..." >&2
  "${ROOT_DIR}/scripts/build-onnxruntime-coreml.sh"
fi

ensure_venv_deps

echo "==> Building C++ PoC"
cmake -S "${POC_DIR}" -B "${BUILD_DIR}" -DCMAKE_BUILD_TYPE=Release -Donnxruntime_DIR="${ORT_CMAKE_DIR}"
cmake --build "${BUILD_DIR}" -j

echo "==> Generating ONNX models for GPU-only validation"
"${PYTHON_RUNTIME}" "${POC_DIR}/generate_gpu_only_models.py" --out-dir "${MODEL_DIR}"

echo "==> [expected: success] numeric model (conv.onnx)"
"${BUILD_DIR}/onnx_poc" "${MODEL_DIR}/conv.onnx"

echo "==> [expected: failure] unsupported model (string_identity.onnx)"
set +e
"${BUILD_DIR}/onnx_poc" "${MODEL_DIR}/string_identity.onnx"
STATUS=$?
set -e

if [[ "${STATUS}" -eq 0 ]]; then
  echo "Error: expected session creation failure, but it succeeded." >&2
  exit 1
fi

echo "OK: session creation failed as expected (exit=${STATUS})."
echo "All checks passed."
