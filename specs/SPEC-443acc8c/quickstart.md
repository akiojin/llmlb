# クイックスタート: ヘルスチェックシステム

## 前提条件

| 項目 | 要件 |
|------|------|
| ルーター | ビルド済み（Rust） |
| ノード | 1台以上のノード |
| ノードトークン | 登録時に発行されたトークン |

## 基本設定

### ルーター側環境変数

```bash
# 監視設定
export LLMLB_HEALTH_CHECK_INTERVAL=10  # 監視間隔（秒）
export LLMLB_NODE_TIMEOUT=60           # タイムアウト（秒）
```

### ノード側環境変数

```bash
# ハートビート設定
export XLLM_HEARTBEAT_SECS=30    # 送信間隔（秒）
export XLLM_TOKEN=<node-token>   # ノードトークン
export LLMLB_URL=http://localhost:8080  # ルーターURL
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
      "last_seen": "2025-01-01T12:00:00Z"
    },
    {
      "id": "node-2",
      "state": "offline",
      "last_seen": "2025-01-01T11:58:00Z"
    }
  ]
}
```

### ハートビート送信（手動テスト）

```bash
# ノードからルーターへハートビート送信
curl -X POST http://localhost:8080/v0/health \
  -H "X-Node-Token: <node-token>" \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "node-1",
    "gpu_metrics": {
      "usage": 45.5,
      "vram_usage": 78.2,
      "temperature": 65.0
    },
    "system_metrics": {
      "cpu_usage": 30.0,
      "memory_usage": 50.0
    },
    "active_requests": 2,
    "loaded_models": ["llama-3.2-1b"]
  }'
```

**レスポンス例**:

```json
{
  "status": "ok",
  "server_time": 1704067200
}
```

### メトリクス確認

```bash
# Prometheusメトリクス
curl http://localhost:8080/metrics | grep -E "(heartbeat|node_state|timeout)"
```

**出力例**:

```text
llm_router_heartbeats_received_total{node_id="node-1"} 100
llm_router_heartbeats_received_total{node_id="node-2"} 95
llm_router_node_state{node_id="node-1",state="online"} 1
llm_router_node_state{node_id="node-2",state="offline"} 1
llm_router_node_timeouts_total{node_id="node-2"} 1
```

## Python での利用

### ハートビート送信

```python
import httpx
import time
import threading

class HeartbeatSender:
    def __init__(self, router_url: str, node_token: str, node_id: str):
        self.router_url = router_url
        self.node_token = node_token
        self.node_id = node_id
        self.running = False

    def send_heartbeat(self, metrics: dict):
        """単一のハートビートを送信"""
        response = httpx.post(
            f"{self.router_url}/v0/health",
            headers={"X-Node-Token": self.node_token},
            json={
                "node_id": self.node_id,
                "gpu_metrics": metrics.get("gpu"),
                "active_requests": metrics.get("active_requests", 0),
                "loaded_models": metrics.get("models", [])
            },
            timeout=10.0
        )
        return response.json()

    def start(self, interval: int = 30):
        """定期的なハートビート送信を開始"""
        self.running = True
        def loop():
            while self.running:
                try:
                    metrics = self._collect_metrics()
                    self.send_heartbeat(metrics)
                except Exception as e:
                    print(f"Heartbeat failed: {e}")
                time.sleep(interval)
        threading.Thread(target=loop, daemon=True).start()

    def stop(self):
        """ハートビート送信を停止"""
        self.running = False

    def _collect_metrics(self):
        """メトリクスを収集（実装依存）"""
        return {
            "gpu": {"usage": 50.0, "vram_usage": 60.0},
            "active_requests": 0,
            "models": ["llama-3.2-1b"]
        }

# 使用例
sender = HeartbeatSender(
    router_url="http://localhost:8080",
    node_token="your-node-token",
    node_id="node-1"
)
sender.start(interval=30)
```

### ノード状態の監視

```python
import httpx
import time

def monitor_health(jwt_token: str, interval: int = 10):
    """ノードの健康状態を監視"""
    while True:
        response = httpx.get(
            "http://localhost:8080/v0/nodes",
            headers={"Authorization": f"Bearer {jwt_token}"}
        )
        nodes = response.json()["nodes"]

        print("\n--- Health Status ---")
        for node in nodes:
            state_icon = "✅" if node["state"] == "online" else "❌"
            print(f"{state_icon} {node['id']}: {node['state']} "
                  f"(last: {node['last_seen']})")

        time.sleep(interval)

# monitor_health("your-jwt-token")
```

### 障害検知アラート

```python
import httpx
from datetime import datetime, timedelta

def check_node_health(jwt_token: str, timeout_threshold: int = 60):
    """ノードの健康状態をチェックし、問題があればアラート"""
    response = httpx.get(
        "http://localhost:8080/v0/nodes",
        headers={"Authorization": f"Bearer {jwt_token}"}
    )
    nodes = response.json()["nodes"]

    alerts = []
    for node in nodes:
        if node["state"] == "offline":
            alerts.append({
                "node_id": node["id"],
                "type": "offline",
                "message": f"Node {node['id']} is offline"
            })

    return alerts

# アラートをチェック
alerts = check_node_health("your-jwt-token")
for alert in alerts:
    print(f"⚠️ {alert['message']}")
```

## 障害シミュレーション

### ノードのタイムアウト

```bash
# 1. ノードを停止
pkill -f xllm

# 2. 60秒後にオフライン状態を確認
sleep 60
curl -X GET http://localhost:8080/v0/nodes \
  -H "Authorization: Bearer <jwt_token>" | jq '.nodes[].state'
```

### ノードの復旧

```bash
# 1. ノードを再起動
./xllm --router-url http://localhost:8080

# 2. 即座にオンライン状態を確認
curl -X GET http://localhost:8080/v0/nodes \
  -H "Authorization: Bearer <jwt_token>" | jq '.nodes[].state'
```

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Nodes」メニューで状態を確認

### 表示項目

| 項目 | 説明 |
|------|------|
| State | Online / Offline / Pending |
| Last Seen | 最終ハートビート受信時刻 |
| Uptime | オンライン継続時間 |
| Heartbeats | 累計ハートビート受信数 |

## 状態遷移

```text
[登録] → Pending → [承認] → Online ← [ハートビート受信]
                              ↓
                    [タイムアウト] → Offline
                              ↑
                    [ハートビート受信]
```

## トラブルシューティング

### ノードがオフラインになる

```bash
# 原因: ハートビート送信失敗
# 確認:
curl http://localhost:8080/metrics | grep heartbeats

# 対策:
# - ネットワーク接続を確認
# - ノードトークンを確認
# - ファイアウォール設定を確認
```

### 誤検知が多い

```bash
# 原因: タイムアウトが短すぎる
# 対策: タイムアウトを延長
export LLMLB_NODE_TIMEOUT=120  # 2分に延長
```

### 復旧が遅い

```bash
# 原因: ハートビート間隔が長い
# 対策: ノード側で間隔を短縮
export XLLM_HEARTBEAT_SECS=10  # 10秒に短縮
```

## 制限事項

| 項目 | 制限 |
|------|------|
| ハートビート間隔 | 10-60秒 |
| タイムアウト | 30-300秒 |
| 最大ノード数 | 1000台 |
| 検出遅延 | 最大40秒 |

## 次のステップ

- アラート通知の設定
- Grafanaダッシュボードの構築
- 自動スケーリング連携
