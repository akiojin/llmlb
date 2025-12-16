#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
POC_DIR="${ROOT_DIR}/poc/image-io-demo"

PYTHON_BIN="${PYTHON_BIN:-python3}"
VENV_DIR="${VENV_DIR:-/tmp/llm_router_image_poc_venv}"
MODEL_DIR="${MODEL_DIR:-/tmp/llm_router_image_poc_models}"

Z_IMAGE_MODEL_ID="${Z_IMAGE_MODEL_ID:-Tongyi-MAI/Z-Image-Turbo}"
Z_IMAGE_PROMPT="${Z_IMAGE_PROMPT:-A cute shiba inu wearing sunglasses, high quality, 4k}"
Z_IMAGE_DEVICE="${Z_IMAGE_DEVICE:-auto}"
Z_IMAGE_HEIGHT="${Z_IMAGE_HEIGHT:-512}"
Z_IMAGE_WIDTH="${Z_IMAGE_WIDTH:-512}"
Z_IMAGE_STEPS="${Z_IMAGE_STEPS:-9}"
Z_IMAGE_GUIDANCE="${Z_IMAGE_GUIDANCE:-0.0}"
Z_IMAGE_SEED="${Z_IMAGE_SEED:-42}"

GLM_MODEL_ID="${GLM_MODEL_ID:-zai-org/GLM-4.6V-Flash}"
GLM_PROMPT="${GLM_PROMPT:-describe this image in Japanese}"
GLM_DEVICE="${GLM_DEVICE:-auto}"
GLM_MAX_NEW_TOKENS="${GLM_MAX_NEW_TOKENS:-256}"

ensure_python() {
  if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1; then
    echo "Error: python is required (PYTHON_BIN=${PYTHON_BIN} not found)" >&2
    exit 1
  fi
}

ensure_venv() {
  ensure_python

  if [[ ! -x "${VENV_DIR}/bin/python" ]]; then
    echo "==> Creating python venv: ${VENV_DIR}"
    "${PYTHON_BIN}" -m venv "${VENV_DIR}"
  fi
}

install_deps_if_needed() {
  local py="${VENV_DIR}/bin/python"

  if "${py}" -c "import torch; from diffusers import ZImagePipeline; from transformers import AutoProcessor, Glm4vForConditionalGeneration; import PIL" >/dev/null 2>&1; then
    return 0
  fi

  echo "==> Installing python deps into venv (this can take a while)"
  "${py}" -m pip install --quiet --upgrade pip >/dev/null 2>&1 || true

  # Core deps
  "${py}" -m pip install --quiet torch pillow accelerate safetensors numpy scipy tqdm importlib-metadata

  # Z-Image requires recent diffusers features (install from source).
  "${py}" -m pip install --quiet --upgrade --force-reinstall git+https://github.com/huggingface/diffusers

  # GLM-4.6V-Flash uses transformers APIs that may require a newer (pre-release) version.
  "${py}" -m pip install --quiet --pre "transformers>=5.0.0rc0"
}

ensure_venv
install_deps_if_needed

mkdir -p "${MODEL_DIR}"

# Keep HF caches under MODEL_DIR so users can control disk usage by changing MODEL_DIR.
export HF_HOME="${HF_HOME:-${MODEL_DIR}/hf_home}"
export HF_XET_HIGH_PERFORMANCE="${HF_XET_HIGH_PERFORMANCE:-1}"

OUT_PNG="${MODEL_DIR}/z_image_out.png"
CAPTION_TXT="${MODEL_DIR}/glm_caption.txt"

echo "==> [T2I] ${Z_IMAGE_MODEL_ID}"
echo "prompt: ${Z_IMAGE_PROMPT}"
"${VENV_DIR}/bin/python" "${POC_DIR}/generate_z_image_turbo.py" \
  --require-gpu \
  --device "${Z_IMAGE_DEVICE}" \
  --model "${Z_IMAGE_MODEL_ID}" \
  --prompt "${Z_IMAGE_PROMPT}" \
  --height "${Z_IMAGE_HEIGHT}" \
  --width "${Z_IMAGE_WIDTH}" \
  --steps "${Z_IMAGE_STEPS}" \
  --guidance "${Z_IMAGE_GUIDANCE}" \
  --seed "${Z_IMAGE_SEED}" \
  --out "${OUT_PNG}"

ls -lh "${OUT_PNG}"
echo "To open (macOS):"
echo "  open \"${OUT_PNG}\""

echo "==> [I2T] ${GLM_MODEL_ID}"
echo "prompt: ${GLM_PROMPT}"
"${VENV_DIR}/bin/python" "${POC_DIR}/glm4v_flash_caption.py" \
  --require-gpu \
  --device "${GLM_DEVICE}" \
  --model "${GLM_MODEL_ID}" \
  --image "${OUT_PNG}" \
  --prompt "${GLM_PROMPT}" \
  --max-new-tokens "${GLM_MAX_NEW_TOKENS}" \
  --out "${CAPTION_TXT}"

echo "==> Caption:"
cat "${CAPTION_TXT}"

echo "OK: T2I(PNG) + I2T(TEXT) round-trip succeeded."
