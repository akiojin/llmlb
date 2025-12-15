#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
POC_DIR="${ROOT_DIR}/poc/onnx-runtime-demo"

ORT_CMAKE_DIR="${ORT_CMAKE_DIR:-/tmp/onnxruntime-coreml/install/lib/cmake/onnxruntime}"
BUILD_DIR="${BUILD_DIR:-${POC_DIR}/build}"
MODEL_DIR="${MODEL_DIR:-/tmp/onnx_poc_models}"

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

echo "==> Building C++ PoC"
cmake -S "${POC_DIR}" -B "${BUILD_DIR}" -DCMAKE_BUILD_TYPE=Release -Donnxruntime_DIR="${ORT_CMAKE_DIR}"
cmake --build "${BUILD_DIR}" -j

echo "==> Generating ONNX models for GPU-only validation"
python3 "${POC_DIR}/generate_gpu_only_models.py" --out-dir "${MODEL_DIR}"

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
