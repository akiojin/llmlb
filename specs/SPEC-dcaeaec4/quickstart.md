# クイックスタート: LLM-Load Balancer独自モデルストレージ

## 前提条件

| 項目 | 要件 |
|------|------|
| ロードバランサー | 起動済み（`http://localhost:8080`） |
| ノード | 1台以上のオンラインノード |
| ストレージ | `~/.llmlb/models/` が存在 |

## 基本的な使用例

### モデルディレクトリの確認

```bash
# デフォルトのモデルディレクトリ
ls ~/.llmlb/models/

# 環境変数でカスタマイズ
export XLLM_MODELS_DIR=/path/to/custom/models
ls $XLLM_MODELS_DIR
```

### 手動でモデルを配置

```bash
# GGUFモデルの場合
mkdir -p ~/.llmlb/models/llama-3.2-1b
cp Llama-3.2-1B-Instruct-Q4_K_M.gguf ~/.llmlb/models/llama-3.2-1b/model.gguf

# SafeTensorsモデルの場合
mkdir -p ~/.llmlb/models/qwen2.5-coder-7b
cp -r qwen2.5-coder-7b/* ~/.llmlb/models/qwen2.5-coder-7b/
```

### モデル一覧の確認

```bash
# ロードバランサー経由で登録済みモデルを確認
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk-your-api-key"
```

### レスポンス例

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3.2-1b",
      "object": "model",
      "created": 1701388800,
      "owned_by": "local"
    },
    {
      "id": "qwen2.5-coder-7b",
      "object": "model",
      "created": 1701388800,
      "owned_by": "local"
    }
  ]
}
```

### モデルマニフェストの取得（Node同期用）

```bash
# 特定モデルのマニフェストを取得
curl http://localhost:8080/v0/models/registry/llama-3.2-1b/manifest.json \
  -H "Authorization: Bearer sk-your-api-key"
```

### マニフェストレスポンス例

```json
{
  "model_id": "llama-3.2-1b",
  "repo": "bartowski/Llama-3.2-1B-Instruct-GGUF",
  "files": [
    {
      "filename": "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
      "format": "gguf",
      "size_bytes": 800000000
    }
  ],
  "created_at": "2024-12-01T00:00:00Z"
}
```

### Python での使用例

```python
import httpx
from pathlib import Path

BASE_URL = "http://localhost:8080"
HEADERS = {"Authorization": "Bearer sk-your-api-key"}
MODELS_DIR = Path.home() / ".llmlb" / "models"

# モデル一覧を取得
response = httpx.get(f"{BASE_URL}/v1/models", headers=HEADERS)
models = response.json()["data"]

print("登録済みモデル:")
for model in models:
    model_id = model["id"]
    local_path = MODELS_DIR / model_id
    exists = local_path.exists()
    print(f"  {model_id}: {'✓ ローカル' if exists else '✗ 未ダウンロード'}")

# マニフェストを取得
model_id = "llama-3.2-1b"
manifest = httpx.get(
    f"{BASE_URL}/v0/models/registry/{model_id}/manifest.json",
    headers=HEADERS
).json()

print(f"\n{model_id} のファイル:")
for f in manifest["files"]:
    size_mb = f["size_bytes"] / 1e6
    print(f"  {f['filename']} ({size_mb:.1f}MB, {f['format']})")
```

## エラーハンドリング

### モデルが見つからない場合

```bash
# HTTP 404 Not Found
{
  "error": {
    "message": "Model 'unknown-model' not found",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

### 不正なモデルID

```bash
# HTTP 400 Bad Request（パストラバーサル試行）
{
  "error": {
    "message": "Invalid model ID: contains path traversal",
    "type": "invalid_request_error",
    "code": "invalid_model_id"
  }
}
```

### ダウンロード失敗

```bash
# HTTP 502 Bad Gateway（HuggingFaceからの取得失敗）
{
  "error": {
    "message": "Failed to download model from HuggingFace",
    "type": "upstream_error",
    "code": "download_failed"
  }
}
```

## 制限事項

| 項目 | 制限 |
|------|------|
| モデルID長 | 最大256文字 |
| モデルID形式 | 英数字、`-`、`_`、`/`のみ |
| パストラバーサル | `..` 禁止 |
| 同時ダウンロード | ノードあたり1件 |
| 対応形式 | GGUF, SafeTensors, Metal |

## 設定変更

### 環境変数

```bash
# モデルディレクトリ（推奨）
export XLLM_MODELS_DIR=/custom/path/models

# 互換モード
export LLM_MODELS_DIR=/custom/path/models

# HuggingFaceトークン（非公開モデル用）
export HF_TOKEN=hf_xxxxxxxxxxxxx
```

### ノード起動時のオプション

```bash
# カスタムモデルディレクトリを指定
xllm --models-dir /custom/path/models

# ヘルプを表示
xllm --help
```
