# Image I/O PoC (Text-to-Image + Image-to-Text)

目的: Hugging Face 配布モデルを **変換せずそのまま**使い、最低限の形で
画像の **入力（Image-to-Text）** と **出力（Text-to-Image）** をエンドツーエンドに確認します。

- Text-to-Image: `Tongyi-MAI/Z-Image-Turbo`（diffusers）
- Image-to-Text: `zai-org/GLM-4.6V-Flash`（transformers）

注意:
- 初回は venv 作成＋依存導入＋モデルDLで時間がかかります（モデルが非常に大きいです）。
  - `Tongyi-MAI/Z-Image-Turbo`: 約 30.6GiB
  - `zai-org/GLM-4.6V-Flash`: 約 19.2GiB
  - 合計で **50GiB+** のダウンロードになります（HFキャッシュの都合で実使用はさらに増える場合があります）。
- GPU前提: `--require-gpu` 相当で動かし、GPUが無い場合は失敗します。
- このPoCは「HF配布モデルをそのまま使う」ため、GGUF変換は不要です。
- `GLM-4.6V-Flash` の Processor は `torchvision.transforms.v2` を利用するため、PoCは `torchvision` も venv に導入します。

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

## トラブルシューティング

### `Killed: 9` で落ちる（macOS）

モデルDL中に Python が `Killed: 9` で落ちる場合、HFの並列DLやXetバックエンドが原因で
落ちるケースがあります。より保守的な設定で実行してください（遅くなりますが安定します）:

```bash
HF_HUB_DISABLE_XET=1 HF_HUB_MAX_WORKERS=1 ./poc/image-io-demo/run_image_io_poc.sh
```

### `SafetensorError: invalid JSON in header` が出る

過去にDLが中断された場合などに、HFキャッシュ内に **0埋めの壊れた `.safetensors`** が残ることがあります。
PoCは壊れたblob検出に対応しているので、次のように自動修復（壊れたblobだけ削除→再DL）してください:

```bash
FIX_HF_CORRUPT_SAFETENSORS=1 ./poc/image-io-demo/run_image_io_poc.sh
```

### CUDAが無い警告が出る

macOSではCUDAは使えないため、`cuda is not available` という警告が出ても `mps` を使っていれば問題ありません。
ログに `Using device: mps` が出ていることを確認してください。
