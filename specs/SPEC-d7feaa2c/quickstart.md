# クイックスタート: マネージャ方式（Text/Audio/Image）

## 前提条件

| 項目 | 要件 |
|------|------|
| OS | macOS (Metal) / Windows (CUDA) |
| GPU | Metal 対応 / NVIDIA CUDA 対応 |
| モデル配置 | `~/.llmlb/models` 配下に配置（GGUF / safetensors） |

> DirectML は凍結。Windows は CUDA 主経路。

## 1. ディレクトリ構成

デフォルトは `~/.llmlb/models`（Windows は `%USERPROFILE%\.llmlb\models`）。

```
~/.llmlb/models/
  llama-3.2-1b/
    model.gguf
  nemotron-3-8b/
    config.json
    tokenizer.json
    model.safetensors
```

- GGUF: `model.gguf` を配置
- safetensors: `config.json` + `tokenizer.json` + `*.safetensors` が必須

## 2. Node 起動

```bash
XLLM_MODELS_DIR=~/.llmlb/models \
  ./llmlb-node
```

## 3. 確認

- `/v1/models` に対象モデルが `ready` で表示されること。
- `/v1/responses` または `/v1/chat/completions` が成功すること。
- 音声/画像が有効な場合は `/v1/audio/transcriptions` / `/v1/images/generations` を確認。

## 4. 失敗時のチェック

- safetensors: `config.json` / `tokenizer.json` の不足
- GGUF: `model.gguf` が見つからない
- GPU バックエンド不一致（CUDA/Metal の未対応）
- build flags: `BUILD_WITH_WHISPER`, `BUILD_WITH_SD` の有効化
