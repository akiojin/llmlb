#!/usr/bin/env bash
set -euo pipefail

# Minimal HF→ONNX変換 + C++ PoC 実行スクリプト
#  - 変換対象: デフォルトは超小型の hf-internal-testing/tiny-random-bert
#  - 依存: python3, pip, torch, transformers, onnx
#  - 出力: /tmp/onnx_poc_model/model.onnx
#
# 使い方:
#   ./convert_and_run.sh                # デフォルトモデルで変換＋PoC実行
#   MODEL=bert-base-uncased ./convert_and_run.sh   # 別モデルを指定（CPUで数分かかる場合あり）

MODEL="${MODEL:-hf-internal-testing/tiny-random-bert}"
OUT_DIR="${OUT_DIR:-/tmp/onnx_poc_model}"
OUT_MODEL="${OUT_DIR}/model.onnx"
PYTHON_BIN="${PYTHON_BIN:-python3}"
VENV_DIR="${VENV_DIR:-/tmp/llm_router_onnx_convert_venv}"

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
}

ensure_venv_deps

echo "==> Installing python deps (torch/transformers/onnx) if missing"
"${PYTHON_RUNTIME}" -m pip install --quiet --upgrade pip >/dev/null 2>&1 || true
"${PYTHON_RUNTIME}" -m pip install --quiet --upgrade "torch>=2.3,<2.5" "transformers>=4.44,<5.0" onnx >/dev/null

echo "==> Exporting ${MODEL} to ONNX ..."
"${PYTHON_RUNTIME}" -m transformers.onnx --opset 17 --model "${MODEL}" --feature=sequence-classification "${OUT_DIR}" >/dev/null

if [[ ! -f "${OUT_MODEL}" ]]; then
  echo "ONNX export failed: ${OUT_MODEL} not found" >&2
  exit 1
fi

echo "==> Built model: ${OUT_MODEL}"
ls -lh "${OUT_MODEL}"

echo "==> Running C++ ONNX Runtime PoC"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
${SCRIPT_DIR}/build/onnx_poc "${OUT_MODEL}"
