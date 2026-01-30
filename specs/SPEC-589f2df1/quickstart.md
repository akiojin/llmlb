# クイックスタート: ロードバランシングシステム

## 前提条件

| 項目 | 要件 |
|------|------|
| ロードバランサー | ビルド済み（Rust） |
| ノード | 2台以上のオンラインノード |
| GPU | 各ノードにGPU搭載 |

## 基本設定

### 環境変数

```bash
# ロードバランサーモード
export LOAD_BALANCER_MODE=metrics  # metrics | round_robin

# GPU負荷閾値
export LLMLB_GPU_THRESHOLD=80      # GPU使用率閾値（%）
export LLMLB_VRAM_THRESHOLD=90     # VRAM使用率閾値（%）
export LLMLB_ACTIVE_REQ_THRESHOLD=10  # アクティブリクエスト閾値
```

### config.toml

```toml
[load_balancer]
mode = "metrics"  # "metrics" or "round_robin"
gpu_threshold = 80
vram_threshold = 90
active_request_threshold = 10
```

## 動作確認

### ノード状態の確認

```bash
# 登録済みノード一覧
curl -X GET http://localhost:8080/v0/nodes \
  -H "Authorization: Bearer <jwt_token>"
```

**レスポンス例**:

```json
{
  "nodes": [
    {
      "id": "node-1",
      "state": "online",
      "gpu_usage": 45.5,
      "vram_usage": 78.2,
      "active_requests": 2
    },
    {
      "id": "node-2",
      "state": "online",
      "gpu_usage": 20.0,
      "vram_usage": 50.0,
      "active_requests": 0
    }
  ]
}
```

### リクエスト分散テスト

```bash
# 複数リクエストを送信
for i in {1..10}; do
  curl -X POST http://localhost:8080/v1/chat/completions \
    -H "Authorization: Bearer sk-your-api-key" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "llama-3.2-1b",
      "messages": [{"role": "user", "content": "Hello"}]
    }' &
done
wait
```

### メトリクス確認

```bash
# Prometheusメトリクス
curl http://localhost:8080/metrics | grep llmlb_node
```

**出力例**:

```text
llmlb_node_gpu_usage{node_id="node-1"} 45.5
llmlb_node_gpu_usage{node_id="node-2"} 20.0
llmlb_node_selections_total{node_id="node-1",reason="lowest_gpu"} 3
llmlb_node_selections_total{node_id="node-2",reason="lowest_gpu"} 7
```

## Python での利用

### シンプルなリクエスト

```python
import httpx

def chat(prompt: str):
    response = httpx.post(
        "http://localhost:8080/v1/chat/completions",
        headers={"Authorization": "Bearer sk-your-api-key"},
        json={
            "model": "llama-3.2-1b",
            "messages": [{"role": "user", "content": prompt}]
        },
        timeout=60.0
    )
    return response.json()

# 自動的に最適なノードに振り分けられる
result = chat("What is load balancing?")
print(result["choices"][0]["message"]["content"])
```

### 負荷テスト

```python
import asyncio
import httpx

async def send_request(client, i):
    response = await client.post(
        "http://localhost:8080/v1/chat/completions",
        headers={"Authorization": "Bearer sk-your-api-key"},
        json={
            "model": "llama-3.2-1b",
            "messages": [{"role": "user", "content": f"Request {i}"}]
        }
    )
    return response.status_code

async def load_test(n_requests=30):
    async with httpx.AsyncClient(timeout=60.0) as client:
        tasks = [send_request(client, i) for i in range(n_requests)]
        results = await asyncio.gather(*tasks)
        print(f"Success: {results.count(200)}/{n_requests}")

asyncio.run(load_test())
```

### ノード負荷の監視

```python
import httpx
import time

def monitor_nodes(jwt_token: str, interval: int = 5):
    while True:
        response = httpx.get(
            "http://localhost:8080/v0/nodes",
            headers={"Authorization": f"Bearer {jwt_token}"}
        )
        nodes = response.json()["nodes"]

        print("\n--- Node Status ---")
        for node in nodes:
            print(f"{node['id']}: GPU={node['gpu_usage']:.1f}%, "
                  f"VRAM={node['vram_usage']:.1f}%, "
                  f"Active={node['active_requests']}")

        time.sleep(interval)

# monitor_nodes("your-jwt-token")
```

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Nodes」メニューでリアルタイム負荷を確認

### 表示項目

| 項目 | 説明 |
|------|------|
| GPU Usage | GPU使用率（%） |
| VRAM Usage | VRAM使用率（%） |
| Active Requests | 処理中リクエスト数 |
| Total Requests | 累計処理リクエスト数 |
| Avg Response Time | 平均レスポンスタイム |

## アルゴリズム動作

### メトリクスモード（デフォルト）

```text
1. オンラインノードを抽出
2. GPU使用率 <= 80% のノードをフィルタ
3. VRAM使用率 <= 90% のノードをフィルタ
4. GPU能力スコア順にソート（高い順）
5. アクティブリクエスト数が最少のノードを選択
```

### フォールバック動作

```text
全ノードが高負荷の場合:
→ ラウンドロビンで順番に振り分け

メトリクス取得失敗の場合:
→ GPU能力スコア順 + ラウンドロビン
```

## トラブルシューティング

### 特定ノードに集中する

```bash
# 原因: 他ノードが高負荷
# 確認:
curl http://localhost:8080/metrics | grep gpu_usage

# 対策: 閾値を調整
export LLMLB_GPU_THRESHOLD=70
```

### 選択が遅い（10ms以上）

```bash
# 原因: ノード数が多い
# 確認:
curl http://localhost:8080/metrics | grep selection_duration

# 対策: メトリクス更新間隔を確認
# エンドポイント側で XLLM_HEARTBEAT_SECS を調整
```

### メトリクスが更新されない

```bash
# 原因: ハートビート送信失敗
# 確認:
curl http://localhost:8080/v0/nodes | jq '.nodes[].last_seen'

# 対策: エンドポイント側のログを確認
# XLLM_HEARTBEAT_SECS=30 が設定されているか
```

## 制限事項

| 項目 | 制限 |
|------|------|
| 最大ノード数 | 1000台 |
| 選択時間 | 10ms以内 |
| メトリクス更新間隔 | 30秒 |
| メトリクス履歴保持 | 直近10ポイント |

## 次のステップ

- Grafanaダッシュボードの構築
- アラート設定（高負荷時の通知）
- カスタムアルゴリズムの実装
