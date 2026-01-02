# クイックスタート: 画像生成モデル対応

## 前提条件

- GPU搭載ノードが登録済み（8GB+ VRAM推奨）
- stable-diffusion.cpp がビルド済み
- Stable Diffusion モデル（safetensors形式）

## 1. 画像生成モデル登録

```bash
export API_KEY="sk_your_api_key"

# Stable Diffusion XL を登録
curl -X POST http://localhost:3000/v1/models/register \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "sdxl-1.0",
    "source": "stabilityai/stable-diffusion-xl-base-1.0",
    "type": "image_generation"
  }'
```

## 2. テキストから画像生成

```bash
# 基本的な画像生成
curl -X POST http://localhost:3000/v1/images/generations \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "sdxl-1.0",
    "prompt": "A white cat sitting on a windowsill, photorealistic",
    "n": 1,
    "size": "1024x1024"
  }'
```

レスポンス例:

```json
{
  "created": 1700000000,
  "data": [
    {
      "url": "http://localhost:3000/images/abc123.png"
    }
  ]
}
```

## 3. 画像編集（Inpainting）

```bash
# マスク領域を編集
curl -X POST http://localhost:3000/v1/images/edits \
  -H "Authorization: Bearer $API_KEY" \
  -F "image=@original.png" \
  -F "mask=@mask.png" \
  -F "prompt=A red hat" \
  -F "size=1024x1024"
```

## 4. バリエーション生成

```bash
# 元画像のバリエーションを生成
curl -X POST http://localhost:3000/v1/images/variations \
  -H "Authorization: Bearer $API_KEY" \
  -F "image=@original.png" \
  -F "n=3" \
  -F "size=1024x1024"
```

## トラブルシューティング

### GPUメモリ不足

SDXL は 12GB+ のVRAMを必要とします。メモリ不足の場合:

- SD 1.5/2.1 など小さいモデルを使用
- 解像度を 512x512 に下げる

### 生成が遅い

- GPU が正しく認識されているか確認
- `--gpu-layers` オプションでGPU使用を明示

### モデルがロードされない

- safetensors ファイルの存在を確認
- config.json の存在を確認
