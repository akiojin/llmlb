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

FIX_HF_CORRUPT_SAFETENSORS="${FIX_HF_CORRUPT_SAFETENSORS:-0}"

ensure_python() {
  if ! command -v "${PYTHON_BIN}" >/dev/null 2>&1; then
    echo "Error: python is required (PYTHON_BIN=${PYTHON_BIN} not found)" >&2
    exit 1
  fi
}

check_hf_safetensors_cache() {
  local model_id="${1}"
  local fix="${FIX_HF_CORRUPT_SAFETENSORS}"
  local py="${VENV_DIR}/bin/python"

  if [[ ! -x "${py}" ]]; then
    return 0
  fi

  "${py}" - <<'PY' "${model_id}" "${fix}"
import os
import struct
import sys
from pathlib import Path

model_id = sys.argv[1]
fix = sys.argv[2] == "1"

try:
    from huggingface_hub.constants import HF_HUB_CACHE
    from huggingface_hub.file_download import repo_folder_name
except Exception:
    # If huggingface_hub is not available yet, skip cache validation.
    raise SystemExit(0)

cache_root = Path(HF_HUB_CACHE)
repo_dir = cache_root / repo_folder_name(repo_id=model_id, repo_type="model")
ref_main = repo_dir / "refs" / "main"
if not ref_main.exists():
    print(f"==> HF cache check: {model_id} (not cached yet)")
    raise SystemExit(0)

snapshot = ref_main.read_text(encoding="utf-8").strip()
snap_dir = repo_dir / "snapshots" / snapshot
if not snap_dir.exists():
    print(f"==> HF cache check: {model_id} (snapshot dir missing: {snap_dir})")
    raise SystemExit(0)

bad = []
for p in snap_dir.rglob("*.safetensors"):
    try:
        target = os.readlink(p)
    except OSError:
        continue
    blob = (p.parent / target).resolve()
    if not blob.exists():
        bad.append((p, blob, "missing_blob"))
        continue
    try:
        with open(blob, "rb") as f:
            head = f.read(8)
    except Exception:
        bad.append((p, blob, "read_error"))
        continue
    if len(head) < 8:
        bad.append((p, blob, "too_short"))
        continue
    header_len = struct.unpack("<Q", head)[0]
    # A valid safetensors file always has a non-empty JSON header.
    if header_len == 0:
        bad.append((p, blob, "zero_header"))
        continue
    # Guardrail: unrealistic header length.
    if header_len > 100_000_000:
        bad.append((p, blob, f"suspicious_header_len={header_len}"))
        continue

if not bad:
    print(f"==> HF cache check: {model_id} (OK)")
    raise SystemExit(0)

print(f"==> HF cache check: {model_id} (CORRUPT safetensors blobs detected: {len(bad)})")
for p, blob, reason in bad[:20]:
    rel = p.relative_to(snap_dir)
    print(f"  - {rel} -> {blob.name} ({reason})")
if len(bad) > 20:
    print(f"  ... and {len(bad) - 20} more")

if not fix:
    print("==> To auto-fix: set FIX_HF_CORRUPT_SAFETENSORS=1 and re-run this script.")
    raise SystemExit(1)

deleted = 0
for _, blob, _ in bad:
    try:
        blob.unlink(missing_ok=True)
        deleted += 1
    except Exception as e:
        print(f"Warning: failed to delete blob {blob}: {e}", file=sys.stderr)

print(f"==> Deleted {deleted} corrupt blob(s). Re-run will re-download them.")
raise SystemExit(0)
PY
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

# Hugging Face download settings (large model repos).
#
# NOTE: Z-Image-Turbo + GLM-4.6V-Flash can download ~50GiB+ on first run.
# If you see `Killed: 9` during download on macOS, use more conservative settings.
export HF_HUB_DISABLE_XET="${HF_HUB_DISABLE_XET:-1}"
export HF_HUB_MAX_WORKERS="${HF_HUB_MAX_WORKERS:-1}"
export HF_XET_HIGH_PERFORMANCE="${HF_XET_HIGH_PERFORMANCE:-0}"
echo "==> HF download settings: HF_HUB_DISABLE_XET=${HF_HUB_DISABLE_XET} HF_HUB_MAX_WORKERS=${HF_HUB_MAX_WORKERS} HF_XET_HIGH_PERFORMANCE=${HF_XET_HIGH_PERFORMANCE}"

OUT_PNG="${MODEL_DIR}/z_image_out.png"
CAPTION_TXT="${MODEL_DIR}/glm_caption.txt"

echo "==> [T2I] ${Z_IMAGE_MODEL_ID}"
echo "prompt: ${Z_IMAGE_PROMPT}"
check_hf_safetensors_cache "${Z_IMAGE_MODEL_ID}"
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
check_hf_safetensors_cache "${GLM_MODEL_ID}"
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
