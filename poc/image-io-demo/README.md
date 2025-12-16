# Image I/O PoC (Text-to-Image + Image-to-Text)

目的: Hugging Face 配布モデルを **変換せずそのまま**使い、最低限の形で
画像の **入力（Image-to-Text）** と **出力（Text-to-Image）** をエンドツーエンドに確認します。

- Text-to-Image: `Tongyi-MAI/Z-Image-Turbo`（diffusers）
- Image-to-Text: `zai-org/GLM-4.6V-Flash`（transformers）

注意:
- 初回は venv 作成＋依存導入＋モデルDLで時間がかかります（モデルが大きいです）。
- GPU前提: `--require-gpu` 相当で動かし、GPUが無い場合は失敗します。
- このPoCは「HF配布モデルをそのまま使う」ため、GGUF変換は不要です。

## 実行

```bash
./poc/image-io-demo/run_image_io_poc.sh
```

出力物:
- 生成画像: `MODEL_DIR/z_image_out.png`
- キャプション: `MODEL_DIR/glm_caption.txt`

macOSでの確認:

```bash
open /tmp/llm_router_image_poc_models/z_image_out.png
cat /tmp/llm_router_image_poc_models/glm_caption.txt
```

## 代表的な環境変数

```bash
# 出力先
MODEL_DIR=/tmp/llm_router_image_poc_models

# Z-Image-Turbo
Z_IMAGE_PROMPT="A cute cat, studio photo, 4k"
Z_IMAGE_HEIGHT=512
Z_IMAGE_WIDTH=512
Z_IMAGE_STEPS=9
Z_IMAGE_GUIDANCE=0.0
Z_IMAGE_DEVICE=auto   # auto|cuda|mps

# GLM-4.6V-Flash
GLM_PROMPT="この画像を日本語で説明して"
GLM_MAX_NEW_TOKENS=256
GLM_DEVICE=auto       # auto|cuda|mps
```

