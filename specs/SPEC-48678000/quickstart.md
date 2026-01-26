# クイックスタート: モデル自動解決機能

## 前提条件

- LLM Load Balancer Node がインストール済み
- `supported_models.json` が設定済み
- ネットワーク接続（HuggingFaceへのアクセス）

## 基本的な使用例

### 1. supported_models.json の設定

```json
{
  "models": [
    {
      "id": "llama-3.2-1b",
      "name": "Llama 3.2 1B",
      "source": "hf_gguf",
      "repo_id": "bartowski/Llama-3.2-1B-Instruct-GGUF",
      "artifacts": [
        {
          "filename": "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
          "size_bytes": 876000000
        }
      ],
      "required_vram_mb": 2048,
      "tags": ["chat", "instruct"]
    }
  ]
}
```

### 2. 推論リクエスト送信

モデルがローカルにない場合、自動的にダウンロードされます。

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

### 3. Python での使用

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk_debug"
)

# モデルがローカルにない場合、自動ダウンロード後に推論実行
response = client.chat.completions.create(
    model="llama-3.2-1b",
    messages=[
        {"role": "user", "content": "Hello!"}
    ]
)

print(response.choices[0].message.content)
```

## ダウンロード進捗の確認

### ログ出力

```text
[INFO] Model resolution started: llama-3.2-1b
[INFO] Model not found locally, downloading from HuggingFace...
[INFO] Download progress: 10% (87.6 MB / 876 MB)
[INFO] Download progress: 20% (175.2 MB / 876 MB)
...
[INFO] Download completed: llama-3.2-1b
[INFO] Model loaded successfully
```

### API経由での確認

```bash
# ダウンロード状態確認
curl http://localhost:8080/v0/models/llama-3.2-1b/status \
  -H "Authorization: Bearer sk_debug"
```

レスポンス:

```json
{
  "model_id": "llama-3.2-1b",
  "status": "downloading",
  "progress": 0.45,
  "downloaded_bytes": 394200000,
  "total_bytes": 876000000
}
```

## エラーハンドリング

### 未定義モデルの場合

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{
    "model": "unknown-model",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

レスポンス:

```json
{
  "error": {
    "message": "Model 'unknown-model' is not supported. Check supported_models.json.",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

### ダウンロード失敗時

```json
{
  "error": {
    "message": "Failed to download model: Network timeout",
    "type": "server_error",
    "code": "download_failed"
  }
}
```

## HuggingFace認証（プライベートモデル）

プライベートリポジトリのモデルを使用する場合:

```bash
# 環境変数で設定
export HF_TOKEN="hf_xxxxxxxxxxxxxxxxxxxx"

# または設定ファイル
echo "HF_TOKEN=hf_xxxxxxxxxxxxxxxxxxxx" >> ~/.llmlb/.env
```

## 制限事項表

| 項目 | 制限値 | 備考 |
|------|--------|------|
| 同時ダウンロード数 | 1件/ノード | 重複ダウンロード防止 |
| ダウンロードタイムアウト | 5分 | 設定可能 |
| 進捗通知粒度 | 10%単位 | ログ出力 |
| サポートソース | HF GGUF, HF safetensors, HF ONNX | 拡張可能 |

## トラブルシューティング

### モデルがダウンロードされない

1. `supported_models.json`にモデルが定義されているか確認
2. ネットワーク接続を確認
3. HF_TOKENが必要なプライベートリポジトリでないか確認

### ダウンロードが途中で止まる

1. ディスク容量を確認（`df -h ~/.llmlb/models/`）
2. ロックファイルを確認（`ls ~/.llmlb/models/.locks/`）
3. タイムアウト設定を確認
