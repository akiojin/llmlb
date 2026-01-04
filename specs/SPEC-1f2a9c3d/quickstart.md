# クイックスタート: Node/Router Log Retrieval API

## 概要

ノードのログをHTTP経由で取得する方法を説明する。

## ノードから直接ログ取得

### 末尾200行を取得（デフォルト）

```bash
curl http://localhost:11435/v0/logs | jq '.entries[-5:]'
```

### 末尾N行を指定して取得

```bash
curl "http://localhost:11435/v0/logs?tail=50" | jq '.entries | length'
```

### レスポンス例

```json
{
  "entries": [
    {
      "timestamp": "2025-01-02T10:30:00.123Z",
      "level": "INFO",
      "target": "llm_node::api",
      "message": "Server started on port 11435"
    },
    {
      "timestamp": "2025-01-02T10:30:01.456Z",
      "level": "INFO",
      "target": "llm_node::api::router_client",
      "message": "Registered with router"
    }
  ],
  "path": "/home/user/.llm-node/logs/current.jsonl"
}
```

## ルーター経由でノードログ取得

### 特定ノードのログを取得

```bash
# まずノードIDを確認
curl http://localhost:8080/v0/nodes | jq '.[].id'

# ノードIDを指定してログ取得
curl "http://localhost:8080/v0/nodes/550e8400-e29b-41d4-a716-446655440000/logs?tail=100"
```

### 全ノードのログを一括取得

```bash
for node_id in $(curl -s http://localhost:8080/v0/nodes | jq -r '.[].id'); do
  echo "=== Node: $node_id ==="
  curl -s "http://localhost:8080/v0/nodes/$node_id/logs?tail=10" | jq '.entries'
done
```

## ログのフィルタリング（jq活用）

### ERRORレベルのみ抽出

```bash
curl http://localhost:11435/v0/logs?tail=500 | \
  jq '.entries | map(select(.level == "ERROR"))'
```

### 特定のターゲットモジュールのログ

```bash
curl http://localhost:11435/v0/logs?tail=500 | \
  jq '.entries | map(select(.target | contains("inference")))'
```

### 時間範囲でフィルタ

```bash
curl http://localhost:11435/v0/logs?tail=1000 | \
  jq '.entries | map(select(.timestamp > "2025-01-02T10:00:00Z"))'
```

## エラーハンドリング

### ノードが見つからない場合

```bash
curl "http://localhost:8080/v0/nodes/invalid-node-id/logs"
```

レスポンス（404 Not Found）:

```json
{
  "error": "Node 'invalid-node-id' not found"
}
```

### ノードに接続できない場合

```bash
curl "http://localhost:8080/v0/nodes/offline-node-id/logs"
```

レスポンス（502 Bad Gateway）:

```json
{
  "error": "Failed to connect to node: connection refused"
}
```

### ログファイルが存在しない場合

```bash
curl http://localhost:11435/v0/logs
```

レスポンス（200 OK、空の配列）:

```json
{
  "entries": [],
  "path": "/home/user/.llm-node/logs/current.jsonl"
}
```

## ダッシュボード連携

ダッシュボードのログパネルは内部的に以下のAPIを呼び出す:

```javascript
// ノード選択時
const response = await fetch(
  `/v0/nodes/${nodeId}/logs?tail=${tailCount}`
);
const { entries } = await response.json();
```

## tailパラメータの範囲

| 値 | 動作 |
|----|------|
| 未指定 | 200（デフォルト） |
| 0以下 | 1にクランプ |
| 1-1000 | そのまま使用 |
| 1001以上 | 1000にクランプ |

## トラブルシューティング

### ログが空で返ってくる

1. ログファイルパスを確認: `~/.llm-node/logs/`
2. ノードのログ設定を確認: `LLM_NODE_LOG_DIR`
3. ログレベルを確認: `LLM_NODE_LOG_LEVEL`

### タイムアウトが発生する

- ノードの負荷状況を確認
- ネットワーク接続を確認
- tail件数を減らして再試行
