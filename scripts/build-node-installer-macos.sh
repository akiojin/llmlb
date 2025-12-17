#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Error: macOS (Darwin) only." >&2
  exit 1
fi

usage() {
  cat <<'EOF' >&2
Usage:
  scripts/build-node-installer-macos.sh <version> <output_pkg>

Environment variables:
  ORT_INSTALL_PREFIX   Where to install onnxruntime (default: /tmp/onnxruntime-coreml/install)
  ORT_DIR              Where to clone onnxruntime sources (default: /tmp/onnxruntime-coreml)
  NODE_BUILD_DIR       CMake build dir for llm-node (default: node/build)
  PKG_ID               pkgbuild identifier (default: io.llm.node)
  LIB_INSTALL_DIR      Install location for dylibs inside the pkgroot (default: /usr/local/lib/llm-router)
EOF
}

version="${1:-}"
output_pkg="${2:-}"
if [[ -z "$version" || -z "$output_pkg" ]]; then
  usage
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

ORT_DIR="${ORT_DIR:-/tmp/onnxruntime-coreml}"
ORT_INSTALL_PREFIX="${ORT_INSTALL_PREFIX:-${ORT_DIR}/install}"
export ORT_DIR ORT_INSTALL_PREFIX

NODE_BUILD_DIR="${NODE_BUILD_DIR:-${repo_root}/node/build}"
PKG_ID="${PKG_ID:-io.llm.node}"
LIB_INSTALL_DIR="${LIB_INSTALL_DIR:-/usr/local/lib/llm-router}"

if [[ ! -f "${repo_root}/node/third_party/whisper.cpp/CMakeLists.txt" ]]; then
  echo "==> Initializing submodules (whisper.cpp)"
  git -C "${repo_root}" submodule update --init --recursive node/third_party/whisper.cpp
fi

if ! command -v brew >/dev/null 2>&1; then
  echo "Error: Homebrew is required to locate OpenSSL (brew)." >&2
  exit 1
fi

if ! brew list openssl@3 >/dev/null 2>&1; then
  echo "==> Installing openssl@3 via Homebrew"
  brew install openssl@3
fi

OPENSSL_PREFIX="$(brew --prefix openssl@3)"
OPENSSL_SSL="${OPENSSL_PREFIX}/lib/libssl.3.dylib"
OPENSSL_CRYPTO="${OPENSSL_PREFIX}/lib/libcrypto.3.dylib"

if [[ ! -f "${OPENSSL_SSL}" || ! -f "${OPENSSL_CRYPTO}" ]]; then
  echo "Error: OpenSSL dylibs not found under ${OPENSSL_PREFIX}/lib." >&2
  exit 1
fi

echo "==> Building onnxruntime (CoreML)"
"${repo_root}/scripts/build-onnxruntime-coreml.sh"

echo "==> Building llm-node"
cmake -S "${repo_root}/node" -B "${NODE_BUILD_DIR}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTS=OFF \
  -DBUILD_SHARED_LIBS=OFF \
  -Donnxruntime_DIR="${ORT_INSTALL_PREFIX}/lib/cmake/onnxruntime" \
  -DOPENSSL_ROOT_DIR="${OPENSSL_PREFIX}"
cmake --build "${NODE_BUILD_DIR}" --config Release

node_bin="${NODE_BUILD_DIR}/llm-node"
if [[ ! -x "${node_bin}" ]]; then
  echo "Error: llm-node binary not found at ${node_bin}" >&2
  exit 1
fi

pkg_root="${repo_root}/pkgroot-node"
rm -rf "${pkg_root}"

install_dir_bin="${pkg_root}/usr/local/bin"
install_dir_lib="${pkg_root}${LIB_INSTALL_DIR}"
mkdir -p "${install_dir_bin}" "${install_dir_lib}"

install -m 0755 "${node_bin}" "${install_dir_bin}/llm-node"

ort_lib_dir="${ORT_INSTALL_PREFIX}/lib"
ort_versioned="$(
  ls "${ort_lib_dir}"/libonnxruntime.[0-9]*.dylib 2>/dev/null | head -1 || true
)"
if [[ -z "${ort_versioned}" ]]; then
  echo "Error: versioned ONNX Runtime dylib not found under ${ort_lib_dir}." >&2
  exit 1
fi

install -m 0644 "${ort_versioned}" "${install_dir_lib}/"
install -m 0644 "${ort_lib_dir}/libonnxruntime.dylib" "${install_dir_lib}/"
install -m 0644 "${OPENSSL_SSL}" "${install_dir_lib}/"
install -m 0644 "${OPENSSL_CRYPTO}" "${install_dir_lib}/"

node_installed="${install_dir_bin}/llm-node"
ssl_installed="${install_dir_lib}/libssl.3.dylib"
crypto_installed="${install_dir_lib}/libcrypto.3.dylib"

echo "==> Fixing dylib install names (OpenSSL)"
install_name_tool -id "@rpath/libcrypto.3.dylib" "${crypto_installed}"
install_name_tool -id "@rpath/libssl.3.dylib" "${ssl_installed}"

ssl_crypto_dep="$(
  otool -L "${ssl_installed}" \
    | awk '/libcrypto\\.3\\.dylib/ { print $1; exit }'
)"
if [[ -n "${ssl_crypto_dep}" ]]; then
  install_name_tool -change "${ssl_crypto_dep}" "@rpath/libcrypto.3.dylib" "${ssl_installed}"
fi

echo "==> Fixing llm-node rpaths and dependencies"
existing_ort_rpath="$(
  otool -l "${node_installed}" \
    | awk -v want="${ORT_INSTALL_PREFIX}/lib" '
        $1=="path" && $2==want { print $2; exit }
      '
)"
if [[ -n "${existing_ort_rpath}" ]]; then
  install_name_tool -delete_rpath "${existing_ort_rpath}" "${node_installed}"
fi

existing_lib_rpath="$(
  otool -l "${node_installed}" \
    | awk -v want="${LIB_INSTALL_DIR}" '
        $1=="path" && $2==want { print $2; exit }
      '
)"
if [[ -z "${existing_lib_rpath}" ]]; then
  install_name_tool -add_rpath "${LIB_INSTALL_DIR}" "${node_installed}"
fi

node_ssl_dep="$(
  otool -L "${node_installed}" \
    | awk '/libssl\\.3\\.dylib/ { print $1; exit }'
)"
node_crypto_dep="$(
  otool -L "${node_installed}" \
    | awk '/libcrypto\\.3\\.dylib/ { print $1; exit }'
)"

if [[ -n "${node_ssl_dep}" ]]; then
  install_name_tool -change "${node_ssl_dep}" "@rpath/libssl.3.dylib" "${node_installed}"
fi
if [[ -n "${node_crypto_dep}" ]]; then
  install_name_tool -change "${node_crypto_dep}" "@rpath/libcrypto.3.dylib" "${node_installed}"
fi

echo "==> Building pkg: ${output_pkg}"
pkgbuild \
  --root "${pkg_root}" \
  --identifier "${PKG_ID}" \
  --version "${version}" \
  --install-location "/" \
  "${output_pkg}"

rm -rf "${pkg_root}"

echo "==> Done: ${output_pkg}"
