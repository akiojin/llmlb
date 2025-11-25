#!/usr/bin/env bash
set -euo pipefail

REPO="akiojin/ollama-router"
ASSET_OS=""
ASSET_ARCH=""

case "$(uname -s)" in
  Linux)  ASSET_OS="linux" ;;
  Darwin) ASSET_OS="macos" ;;
  *) echo "Unsupported OS: $(uname -s)"; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ASSET_ARCH="amd64" ;;
  arm64|aarch64) ASSET_ARCH="arm64" ;;
  *) echo "Unsupported arch: $(uname -m)"; exit 1 ;;
esac

ASSET_NAME="ollama-node-${ASSET_OS}-${ASSET_ARCH}"
EXT="tar.gz"
if [ "$ASSET_OS" = "windows" ]; then EXT="zip"; fi

URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}.${EXT}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading ${URL}..."
curl -fsSL "$URL" -o "${TMPDIR}/asset.${EXT}"

echo "Extracting..."
if [ "$EXT" = "tar.gz" ]; then
  tar -xzf "${TMPDIR}/asset.${EXT}" -C "$TMPDIR"
else
  unzip -q "${TMPDIR}/asset.${EXT}" -d "$TMPDIR"
fi

if [ ! -f "${TMPDIR}/ollama-node" ] && [ -f "${TMPDIR}/win/ollama-node.exe" ]; then
  # windows zip structure
  mv "${TMPDIR}/win/ollama-node.exe" "${TMPDIR}/ollama-node.exe"
fi

INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
  echo "Installing requires sudo for ${INSTALL_DIR}"
  sudo mv "${TMPDIR}/ollama-node"* "$INSTALL_DIR/"
  sudo chmod +x "${INSTALL_DIR}/ollama-node"*
else
  mv "${TMPDIR}/ollama-node"* "$INSTALL_DIR/"
  chmod +x "${INSTALL_DIR}/ollama-node"*
fi

echo "Installed to ${INSTALL_DIR}"
