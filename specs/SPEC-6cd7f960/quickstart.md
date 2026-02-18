# クイックスタート: 対応モデルリスト型管理

## 前提条件

| 項目 | 要件 |
|------|------|
| ロードバランサー | 起動済み（`http://localhost:8080`） |
| ノード | 1台以上のオンラインノード |
| 認証 | admin / test でログイン済み |

## 基本的な使用例

### 対応モデル一覧の取得

```bash
# 対応モデル一覧を取得
curl http://localhost:8080/api/models \
  -H "Authorization: Bearer sk-your-api-key"
```

### レスポンス例

```json
{
  "models": [
    {
      "model": {
        "id": "llama-3.2-1b",
        "name": "LLaMA 3.2 1B",
        "description": "Lightweight model for quick inference",
        "repo": "bartowski/Llama-3.2-1B-Instruct-GGUF",
        "size_bytes": 800000000,
        "required_memory_bytes": 1600000000,
        "tags": ["chat", "instruct", "lightweight"],
        "capabilities": ["TextGeneration"],
        "quantization": "Q4_K_M"
      },
      "status": "Available",
      "hf_stats": {
        "downloads": 50000,
        "stars": 120
      },
      "available_nodes": 0
    },
    {
      "model": {
        "id": "qwen2.5-coder-7b",
        "name": "Qwen2.5 Coder 7B"
      },
      "status": "Ready",
      "available_nodes": 2
    }
  ]
}
```

### モデルの登録（Pull開始）

```bash
# モデルを登録（ノードが自動的にダウンロード開始）
curl -X POST http://localhost:8080/api/models/register \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"model_id": "llama-3.2-1b"}'
```

### Python での使用例

```python
import httpx

BASE_URL = "http://localhost:8080"
HEADERS = {"Authorization": "Bearer sk-your-api-key"}

# 対応モデル一覧を取得
response = httpx.get(f"{BASE_URL}/api/models", headers=HEADERS)
models = response.json()["models"]

# 利用可能なモデルを表示
for m in models:
    status = m["status"]
    name = m["model"]["name"]
    size_gb = m["model"]["size_bytes"] / 1e9
    print(f"{name}: {status} ({size_gb:.1f}GB)")

# 未ダウンロードのモデルを登録
available_models = [m for m in models if m["status"] == "Available"]
if available_models:
    model_id = available_models[0]["model"]["id"]
    httpx.post(
        f"{BASE_URL}/api/models/register",
        headers=HEADERS,
        json={"model_id": model_id}
    )
    print(f"Registered: {model_id}")
```

### WebSocketでリアルタイム更新を受信

```python
import asyncio
import websockets
import json

async def listen_model_updates():
    uri = "ws://localhost:8080/ws/models"
    async with websockets.connect(uri) as ws:
        async for message in ws:
            event = json.loads(message)
            if event["type"] == "ModelStatusChanged":
                print(f"Model {event['model_id']}: {event['status']}")
                if event.get("progress"):
                    print(f"  Progress: {event['progress']*100:.1f}%")

asyncio.run(listen_model_updates())
```

## ダッシュボードでの操作

### Model Hubタブ

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Models」メニュー → 「Model Hub」タブを選択
4. 利用可能なモデルがカード形式で表示される

### モデルのPull

1. Model Hubタブでモデルカードを確認
2. 「Pull」ボタンをクリック
3. プログレスバーでダウンロード進捗を確認
4. 完了後、「Local」タブにモデルが表示される

### Localタブ

1. 「Local」タブを選択
2. ダウンロード済みモデル一覧が表示される
3. 各モデルの「Delete」ボタンで削除可能

## エラーハンドリング

### 未対応モデルの登録試行

```bash
# HTTP 400 Bad Request
{
  "error": {
    "message": "Model 'unknown-model' is not in supported models list",
    "type": "invalid_request_error",
    "code": "model_not_supported"
  }
}
```

### VRAMが不足するモデル

```bash
# HTTP 400 Bad Request（ノード登録後に発生）
{
  "error": {
    "message": "Insufficient VRAM. Required: 16GB, Available: 8GB",
    "type": "resource_error",
    "code": "insufficient_memory"
  }
}
```

### ダウンロード中モデルへの推論リクエスト

```bash
# HTTP 503 Service Unavailable
{
  "error": {
    "message": "Model 'llama-3.2-1b' is still downloading (45% complete)",
    "type": "service_unavailable",
    "code": "model_downloading"
  }
}
```

## 制限事項

| 項目 | 制限 |
|------|------|
| 対応モデル | supported_models.jsonに定義されたもののみ |
| カスタムモデル登録 | 非対応（対応モデル以外は登録不可） |
| モデル自動更新 | 非対応 |
| バージョン管理 | 非対応 |
| エンジン選択 | 自動（ユーザー変更不可） |
| 同時ダウンロード | ノード依存 |

## モデル状態一覧

| 状態 | 説明 | UIアクション |
|------|------|-------------|
| Available | Hub上で利用可能 | Pullボタン表示 |
| Registered | 登録済み（ダウンロード待ち） | 進捗待ち |
| Downloading | ダウンロード中 | プログレスバー表示 |
| Ready | 利用可能 | チェックマーク表示 |
| Error | エラー状態 | リトライボタン表示 |

## 次のステップ

- Playgroundでモデルをテスト
- 複数ノードへのモデル配布
- クラウドプロバイダー連携（SPEC-996e37bf）
