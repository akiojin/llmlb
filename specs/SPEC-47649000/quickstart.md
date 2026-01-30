# クイックスタート: モデルメタデータSQLite統合

## 前提条件

- LLM Load Balancer がインストール済み
- lb.db が初期化済み（自動）

## 基本的な使用例

### 1. 既存データの自動移行

既存の`models.json`がある場合、ロードバランサー起動時に自動移行されます。

```bash
# ロードバランサー起動
./llmlb

# ログ出力
# [INFO] Migrating models.json to SQLite...
# [INFO] Migrated 15 models successfully
# [INFO] Original file renamed to models.json.migrated
```

### 2. モデル一覧の取得

```bash
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug"
```

レスポンス:

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3.2-1b",
      "object": "model",
      "owned_by": "meta-llama",
      "tags": ["chat", "instruct"]
    },
    {
      "id": "mistral-7b",
      "object": "model",
      "owned_by": "mistralai",
      "tags": ["chat", "instruct", "tools"]
    }
  ]
}
```

### 3. タグによるモデル検索

```bash
# 単一タグ検索
curl "http://localhost:8080/api/models?tag=vision" \
  -H "Authorization: Bearer sk_debug"

# 複数タグAND検索
curl "http://localhost:8080/api/models?tag=vision&tag=chat" \
  -H "Authorization: Bearer sk_debug"
```

### 4. ソースによるフィルタリング

```bash
# HuggingFace safetensorsモデルのみ
curl "http://localhost:8080/api/models?source=hf_safetensors" \
  -H "Authorization: Bearer sk_debug"

# HuggingFace GGUFモデルのみ
curl "http://localhost:8080/api/models?source=hf_gguf" \
  -H "Authorization: Bearer sk_debug"
```

### 5. Python での使用

```python
import requests

BASE_URL = "http://localhost:8080"
HEADERS = {"Authorization": "Bearer sk_debug"}

# モデル一覧取得
response = requests.get(f"{BASE_URL}/v1/models", headers=HEADERS)
models = response.json()["data"]

# タグ検索
response = requests.get(
    f"{BASE_URL}/api/models",
    params={"tag": ["vision", "chat"]},
    headers=HEADERS
)
vision_models = response.json()["models"]

# ソースフィルタリング
response = requests.get(
    f"{BASE_URL}/api/models",
    params={"source": "hf_safetensors"},
    headers=HEADERS
)
safetensors_models = response.json()["models"]
```

## ダッシュボードでの操作

### モデル一覧表示

1. ダッシュボード（`http://localhost:8080`）にログイン
2. サイドメニューから「Models」を選択
3. モデル一覧が表示される（100件以上でも1秒以内）

### タグフィルタリング

1. 画面上部のフィルタ領域でタグを選択
2. 選択したタグを持つモデルのみが表示される

### ソースフィルタリング

1. 「Source」ドロップダウンからソースを選択
2. 該当ソースのモデルのみが表示される

## エラーハンドリング

### 移行失敗時

```json
{
  "error": {
    "message": "Failed to migrate models.json: Invalid JSON format",
    "type": "migration_error",
    "code": "invalid_json"
  }
}
```

対処法:

1. `models.json`のJSON形式を確認
2. 必須フィールド（name）が存在するか確認
3. 手動で修正後、ロードバランサーを再起動

### 検索エラー

```json
{
  "error": {
    "message": "Invalid source filter value",
    "type": "validation_error",
    "code": "invalid_source"
  }
}
```

有効なソース値: `hf_safetensors`, `hf_gguf`, `hf_onnx`, `predefined`

## 制限事項表

| 項目 | 制限値 | 備考 |
|------|--------|------|
| モデル一覧取得 | 1秒以内 | 100件以上でも |
| タグ検索 | 0.5秒以内 | インデックス使用 |
| タグ数/モデル | 無制限 | 実用上50以下推奨 |
| モデル名長 | 255文字 | UTF-8 |
| タグ名長 | 50文字 | UTF-8 |

## トラブルシューティング

### 移行が実行されない

1. `models.json`が存在するか確認
2. `models.json.migrated`が既に存在していないか確認
3. ファイル権限を確認（読み取り権限が必要）

### 検索が遅い

1. データベースファイルサイズを確認
2. `VACUUM`コマンドでデータベースを最適化

```bash
sqlite3 lb.db "VACUUM;"
```

### データが表示されない

1. マイグレーションが完了しているか確認
2. lb.dbにmodelsテーブルが存在するか確認

```bash
sqlite3 lb.db ".tables"
# models  model_tags  users  api_keys ...
```
