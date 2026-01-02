# クイックスタート: 画像認識モデル対応（Image Understanding）

## 前提条件

| 項目 | 要件 |
|------|------|
| モデル | Vision対応モデル（LLaVA、Qwen-VL等） |
| ノード | llama.cpp multimodal対応ビルド |
| GPUメモリ | モデルサイズ + 2〜8GB追加 |

## 基本的な使用例

### 画像URLを使用したリクエスト

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llava-1.6-7b",
    "messages": [
      {
        "role": "user",
        "content": [
          {"type": "text", "text": "この画像に何が写っていますか？"},
          {
            "type": "image_url",
            "image_url": {
              "url": "https://example.com/image.jpg"
            }
          }
        ]
      }
    ],
    "max_tokens": 300
  }'
```

### Base64エンコード画像を使用したリクエスト

```bash
# 画像をBase64エンコード
IMAGE_BASE64=$(base64 -i /path/to/image.jpg)

curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d "{
    \"model\": \"llava-1.6-7b\",
    \"messages\": [
      {
        \"role\": \"user\",
        \"content\": [
          {\"type\": \"text\", \"text\": \"この画像を説明してください\"},
          {
            \"type\": \"image_url\",
            \"image_url\": {
              \"url\": \"data:image/jpeg;base64,${IMAGE_BASE64}\"
            }
          }
        ]
      }
    ]
  }"
```

### Python での使用例

```python
import base64
import httpx

def encode_image(image_path):
    with open(image_path, "rb") as f:
        return base64.b64encode(f.read()).decode("utf-8")

# 画像URLを使用
response = httpx.post(
    "http://localhost:8080/v1/chat/completions",
    headers={"Authorization": "Bearer sk-your-api-key"},
    json={
        "model": "llava-1.6-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "この画像に何が写っていますか？"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/cat.jpg"}
                    }
                ]
            }
        ]
    }
)
print(response.json()["choices"][0]["message"]["content"])

# Base64画像を使用
image_data = encode_image("./local_image.jpg")
response = httpx.post(
    "http://localhost:8080/v1/chat/completions",
    headers={"Authorization": "Bearer sk-your-api-key"},
    json={
        "model": "llava-1.6-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "画像を分析してください"},
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": f"data:image/jpeg;base64,{image_data}"
                        }
                    }
                ]
            }
        ]
    }
)
```

### 複数画像の比較

```python
response = httpx.post(
    "http://localhost:8080/v1/chat/completions",
    headers={"Authorization": "Bearer sk-your-api-key"},
    json={
        "model": "llava-1.6-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "これら2つの画像の違いを説明してください"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/image1.jpg"}
                    },
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/image2.jpg"}
                    }
                ]
            }
        ]
    }
)
```

### ストリーミングレスポンス

```python
import httpx

with httpx.stream(
    "POST",
    "http://localhost:8080/v1/chat/completions",
    headers={"Authorization": "Bearer sk-your-api-key"},
    json={
        "model": "llava-1.6-7b",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "この画像を詳しく説明してください"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/image.jpg"}
                    }
                ]
            }
        ],
        "stream": True
    }
) as response:
    for line in response.iter_lines():
        if line.startswith("data: "):
            print(line[6:], end="", flush=True)
```

## エラーハンドリング

### 非対応モデルへのリクエスト

```bash
# HTTP 400 Bad Request
{
  "error": {
    "message": "Model 'llama-3.2-1b' does not support image understanding",
    "type": "invalid_request_error",
    "code": "model_not_supported"
  }
}
```

### 画像サイズ超過

```bash
# HTTP 400 Bad Request
{
  "error": {
    "message": "Image size 15728640 bytes exceeds maximum 10485760 bytes",
    "type": "invalid_request_error",
    "code": "image_too_large"
  }
}
```

### サポートされていない形式

```bash
# HTTP 400 Bad Request
{
  "error": {
    "message": "Image format 'image/tiff' is not supported. Supported formats: jpeg, png, gif, webp",
    "type": "invalid_request_error",
    "code": "unsupported_format"
  }
}
```

## Vision対応モデルの確認

```bash
# /v1/models でcapabilitiesを確認
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk-your-api-key" | jq '.data[] | select(.capabilities.image_understanding == true)'

# 出力例:
# {
#   "id": "llava-1.6-7b",
#   "capabilities": {
#     "text_generation": true,
#     "image_understanding": true
#   }
# }
```

## 制限事項

| 項目 | 制限 |
|------|------|
| 最大画像サイズ | 10MB/画像 |
| 最大画像数 | 10枚/リクエスト |
| 対応形式 | JPEG, PNG, GIF, WebP |
| URL取得タイムアウト | 30秒 |
| リダイレクト | 最大3回 |

## 推奨事項

1. **画像サイズ**: 大きな画像は事前にリサイズすると高速
2. **Base64 vs URL**: 小さな画像はBase64、大きな画像はURL推奨
3. **モデル選択**: タスクに応じて適切なVisionモデルを選択
   - 汎用: LLaVA-1.6
   - 軽量: MiniCPM-V, Phi-3-Vision
   - 高解像度: Phi-3-Vision（1344x1344対応）
