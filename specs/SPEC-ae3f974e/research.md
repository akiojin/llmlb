# リサーチ: 画像生成モデル対応

## 調査目的

画像生成モデルを safetensors 正本で実行するための技術調査。

## エンジン候補

### stable-diffusion.cpp

| 項目 | 状態 |
|------|------|
| safetensors対応 | ○ 直接ロード可能 |
| Metal | ○ 対応 |
| DirectML | △ 実験的 |
| CUDA | ○ 対応 |
| ライセンス | MIT |

**採用理由**: safetensors 直接読み込み、C++実装、Python依存なし

### 代替候補（不採用）

| エンジン | 不採用理由 |
|---------|-----------|
| diffusers (Python) | Python依存 |
| ONNX Runtime | 変換が必要 |
| TensorRT | NVIDIA専用 |

## 対応モデル

### Stable Diffusion 系

| モデル | サイズ | VRAM要件 |
|--------|--------|---------|
| SD 1.5 | ~4GB | 6GB+ |
| SD 2.1 | ~5GB | 8GB+ |
| SDXL | ~6GB | 12GB+ |

### 形式優先度

1. safetensors（正本）
2. GGUF（Nodeが選択、利用可能な場合）

## API仕様

### OpenAI Images API互換

| エンドポイント | 用途 |
|---------------|------|
| POST /v1/images/generations | テキストから画像生成 |
| POST /v1/images/edits | 画像編集（Inpainting） |
| POST /v1/images/variations | バリエーション生成 |

### サポートパラメータ

| パラメータ | 値 |
|-----------|-----|
| size | 256x256, 512x512, 1024x1024, 1792x1024, 1024x1792 |
| quality | standard, hd |
| style | vivid, natural |
| response_format | url, b64_json |
| n | 1-10 |

## 制約

- 入力画像: 最大4MB
- 生成時間: 1024x1024で30秒以内（GPU使用時）
- GPUメモリ: SDXL使用時12GB+推奨

## 参考資料

- [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp)
- [OpenAI Images API](https://platform.openai.com/docs/api-reference/images)
