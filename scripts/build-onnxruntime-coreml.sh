#!/usr/bin/env bash
set -euo pipefail

# Build ONNX Runtime from source with CoreML EP enabled (macOS / Apple Silicon).
#
# Why this exists:
# - Homebrew's onnxruntime bottle is typically CPU-only (no CoreML/XNNPACK EP).
# - Building from source can hit:
#   - abseil/re2/protobuf target conflicts (when system packages are found)
#   - psimd CMake minimum version issues on CMake 4.x
#
# This script pins the build settings we validated on macOS (arm64) and installs
# a CMake package that `find_package(onnxruntime)` can consume.
#
# Usage:
#   ./scripts/build-onnxruntime-coreml.sh
#
# Override paths:
#   ORT_DIR=/path/to/onnxruntime-src \
#   ORT_BUILD_DIR=/path/to/build \
#   ORT_INSTALL_PREFIX=/path/to/prefix \
#   ./scripts/build-onnxruntime-coreml.sh

ORT_VERSION="${ORT_VERSION:-v1.22.2}"
ORT_REPO_URL="${ORT_REPO_URL:-https://github.com/microsoft/onnxruntime.git}"

ORT_DIR="${ORT_DIR:-/tmp/onnxruntime-coreml}"
ORT_BUILD_DIR="${ORT_BUILD_DIR:-${ORT_DIR}/build_coreml}"
ORT_INSTALL_PREFIX="${ORT_INSTALL_PREFIX:-${ORT_DIR}/install}"

PYTHON_BIN="${PYTHON_BIN:-}"
if [[ -z "${PYTHON_BIN}" ]]; then
  if [[ -x "/opt/homebrew/bin/python3" ]]; then
    PYTHON_BIN="/opt/homebrew/bin/python3"
  else
    PYTHON_BIN="python3"
  fi
fi

CMAKE_BIN="${CMAKE_BIN:-cmake}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Error: This script is intended for macOS (Darwin)." >&2
  exit 1
fi

if [[ ! -d "${ORT_DIR}/.git" ]]; then
  echo "==> Cloning onnxruntime ${ORT_VERSION} into ${ORT_DIR}"
  rm -rf "${ORT_DIR}"
  git clone --depth 1 --branch "${ORT_VERSION}" "${ORT_REPO_URL}" "${ORT_DIR}"
else
  echo "==> Using existing onnxruntime checkout at ${ORT_DIR}"
fi

echo "==> Building onnxruntime (CoreML EP enabled)"
"${PYTHON_BIN}" "${ORT_DIR}/tools/ci_build/build.py" \
  --build_dir "${ORT_BUILD_DIR}" \
  --config Release \
  --update \
  --build \
  --parallel \
  --build_shared_lib \
  --use_coreml \
  --skip_tests \
  --cmake_extra_defines \
  "FETCHCONTENT_TRY_FIND_PACKAGE_MODE=NEVER" \
  "CMAKE_POLICY_VERSION_MINIMUM=3.5"

echo "==> Installing CMake package to ${ORT_INSTALL_PREFIX}"
rm -rf "${ORT_INSTALL_PREFIX}"
"${CMAKE_BIN}" --install "${ORT_BUILD_DIR}/Release" --prefix "${ORT_INSTALL_PREFIX}"

cat <<EOF
==> Done.

To build llm-node with this onnxruntime:
  cmake -S node -B build -DCMAKE_BUILD_TYPE=Release \\
    -Donnxruntime_DIR="${ORT_INSTALL_PREFIX}/lib/cmake/onnxruntime"

To build the PoC:
  cmake -S poc/onnx-runtime-demo -B build-poc -DCMAKE_BUILD_TYPE=Release \\
    -Donnxruntime_DIR="${ORT_INSTALL_PREFIX}/lib/cmake/onnxruntime"
EOF

