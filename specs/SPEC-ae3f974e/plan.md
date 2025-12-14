# 実装計画: 画像生成モデル対応

**機能ID**: `SPEC-ae3f974e`
**作成日**: 2024-12-14

## アーキテクチャ

```text
┌──────────────────────────────────────────────────────────────┐
│  Router (Rust) - OpenAI互換API                               │
│  ├─ /v1/images/generations    → StableDiffusionノード選択    │
│  ├─ /v1/images/edits          → StableDiffusionノード選択    │
│  └─ /v1/images/variations     → StableDiffusionノード選択    │
└──────────────────────┬───────────────────────────────────────┘
                       │
                       ↓
              ┌────────────────┐
              │ Node           │
              │ stable-        │
              │ diffusion.cpp  │
              │ (Image Gen)    │
              └────────────────┘
```

## API設計（OpenAI互換）

### POST /v1/images/generations

テキストから画像を生成（Text-to-Image）

```json
Request (JSON):
{
  "model": "stable-diffusion-xl",
  "prompt": "A white cat sitting on a windowsill",
  "n": 1,
  "size": "1024x1024",
  "quality": "standard",
  "response_format": "url",
  "style": "vivid"
}

Response:
{
  "created": 1699000000,
  "data": [
    { "url": "https://..." }
  ]
}
```

### POST /v1/images/edits

画像編集（Inpainting）

```text
Request (multipart/form-data):
  image: <image_file>           # PNG, max 4MB
  mask: <mask_file>             # optional, PNG with transparency
  prompt: "A sunlit indoor lounge area"
  model: stable-diffusion-xl
  n: 1
  size: 1024x1024
  response_format: url

Response: (same as generations)
```

### POST /v1/images/variations

画像のバリエーション生成

```text
Request (multipart/form-data):
  image: <image_file>           # PNG, max 4MB
  model: stable-diffusion-xl
  n: 1
  size: 1024x1024
  response_format: url

Response: (same as generations)
```

## 実装フェーズ

### Phase 1: 契約テスト作成（RED）- ✅ 完了

| ファイル | 内容 |
|---------|------|
| `router/tests/contract/images_generations_test.rs` | generations API契約テスト |
| `router/tests/contract/images_edits_test.rs` | edits API契約テスト |
| `router/tests/contract/images_variations_test.rs` | variations API契約テスト |

### Phase 2: 型定義・プロトコル拡張 - ✅ 完了

| ファイル | 変更内容 |
|---------|---------|
| `common/src/types.rs` | RuntimeType::StableDiffusion, ModelType::ImageGeneration, ImageSize, ImageQuality, ImageStyle, ImageResponseFormat |
| `common/src/protocol.rs` | ImageGenerationRequest, ImageEditRequest, ImageVariationRequest, ImageResponse, ImageData |

### Phase 3: Router側API実装 - ✅ 完了

| ファイル | 変更内容 |
|---------|---------|
| `router/src/api/images.rs` | generations, edits, variationsエンドポイント |
| `router/src/api/mod.rs` | imagesモジュール追加、ルート登録 |

### Phase 4: 統合テスト作成（RED）- ✅ 完了

| ファイル | 内容 |
|---------|------|
| `router/tests/integration/images_api_test.rs` | 画像APIルーティング統合テスト |

### Phase 5: Node側実装 - stable-diffusion.cpp - ⏳ 未着手

| ファイル | 変更内容 |
|---------|---------|
| `node/CMakeLists.txt` | stable-diffusion.cppサブモジュール追加 |
| `node/include/core/sd_manager.h` | Stable Diffusionマネージャーヘッダー |
| `node/src/core/sd_manager.cpp` | stable-diffusion.cpp統合実装 |
| `node/include/api/image_endpoints.h` | 画像エンドポイントヘッダー |
| `node/src/api/image_endpoints.cpp` | /v1/images/* ハンドラー |
| `node/src/main.cpp` | 画像エンドポイント登録 |

## 依存関係

### Node (C++)

- **stable-diffusion.cpp**: <https://github.com/leejet/stable-diffusion.cpp>
  - SD 1.x, SD 2.x, SDXL対応
  - GGML/GGUF形式モデル
  - GPU/CPU両対応
- **stb_image**: 画像読み込み/書き込み（ヘッダーオンリー）

## 注意事項

1. **GPUメモリ**: 画像生成は大量のVRAMを消費（SDXL: 8GB+）
2. **生成時間**: 1枚あたり数秒〜数十秒
3. **ファイルサイズ**: 入力画像は最大4MB制限
4. **一時ファイル**: 生成画像の保存先管理（URLレスポンス時）
5. **並列処理**: バッチ生成（n > 1）の効率的な処理
