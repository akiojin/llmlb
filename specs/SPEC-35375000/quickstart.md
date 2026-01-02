# クイックスタート: ルーター負荷最適化

## 前提条件

| 項目 | 要件 |
|------|------|
| ルーター | ビルド済み（Rust） |
| ノード | 1台以上のオンラインノード |
| 負荷テストツール | wrk, hey, k6 など（オプション） |

## 基本設定

### 環境変数

```bash
# 待機キュー設定
export LLM_ROUTER_MAX_WAITING=100        # 最大待機数
export LLM_ROUTER_WAIT_TIMEOUT_SECS=30   # タイムアウト（秒）

# バックプレッシャー設定
export LLM_ROUTER_WARNING_THRESHOLD=0.5    # 警告閾値（50%）
export LLM_ROUTER_OVERLOAD_THRESHOLD=0.8   # 過負荷閾値（80%）

# 接続プール設定
export LLM_ROUTER_POOL_MAX_IDLE=32         # 最大アイドル接続数
export LLM_ROUTER_POOL_IDLE_TIMEOUT_SECS=90  # アイドルタイムアウト

# ノードキャッシュ設定
export LLM_ROUTER_CACHE_MAX_CAPACITY=1000  # 最大キャッシュエントリ
export LLM_ROUTER_CACHE_TTL_SECS=5         # キャッシュTTL
```

### config.toml

```toml
[load_manager]
max_waiting = 100
default_timeout_secs = 30
warning_threshold = 0.5
overload_threshold = 0.8

[connection_pool]
max_idle_per_host = 32
idle_timeout_secs = 90

[node_cache]
max_capacity = 1000
ttl_secs = 5
```

## 動作確認

### 通常リクエスト

```bash
# 通常のリクエスト（待機なし）
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### 負荷テスト（wrk）

```bash
# 100接続で30秒間のテスト
wrk -t4 -c100 -d30s \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -s post.lua \
  http://localhost:8080/v1/chat/completions

# post.lua の内容
# wrk.method = "POST"
# wrk.body   = '{"model":"llama-3.2-1b","messages":[{"role":"user","content":"Hi"}]}'
```

### 負荷テスト（hey）

```bash
# 1000リクエストを50並列で実行
hey -n 1000 -c 50 \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -m POST \
  -d '{"model":"llama-3.2-1b","messages":[{"role":"user","content":"Hi"}]}' \
  http://localhost:8080/v1/chat/completions
```

## メトリクス確認

### Prometheus エンドポイント

```bash
curl http://localhost:8080/metrics
```

### 主要メトリクス

```text
# 待機キューサイズ
llm_router_queue_size 45

# リクエスト処理時間（p95）
llm_router_request_duration_seconds{quantile="0.95"} 0.085

# バックプレッシャー拒否数
llm_router_backpressure_rejections_total 12

# キャッシュヒット率
llm_router_cache_hits_total 9500
llm_router_cache_misses_total 500
# ヒット率: 9500 / (9500 + 500) = 95%
```

### Python でのメトリクス取得

```python
import httpx

def get_metrics():
    response = httpx.get("http://localhost:8080/metrics")
    metrics = {}

    for line in response.text.split("\n"):
        if line and not line.startswith("#"):
            parts = line.split(" ")
            if len(parts) >= 2:
                metrics[parts[0]] = float(parts[1])

    return metrics

metrics = get_metrics()
print(f"Queue Size: {metrics.get('llm_router_queue_size', 0)}")
print(f"Rejections: {metrics.get('llm_router_backpressure_rejections_total', 0)}")
```

## エラーハンドリング

### キュー満杯時（503）

```bash
# キューが80%以上で新規リクエスト
# HTTP 503 Service Unavailable
{
  "error": {
    "message": "Service overloaded. Please retry later.",
    "type": "service_unavailable",
    "code": "queue_full"
  }
}
```

### タイムアウト時（504）

```bash
# 待機が30秒を超えた場合
# HTTP 504 Gateway Timeout
{
  "error": {
    "message": "Request timeout after 30 seconds",
    "type": "gateway_timeout",
    "code": "wait_timeout"
  }
}
```

### リトライ処理（Python）

```python
import time
import httpx

def request_with_retry(prompt, max_retries=3):
    for attempt in range(max_retries):
        response = httpx.post(
            "http://localhost:8080/v1/chat/completions",
            headers={"Authorization": "Bearer sk-your-api-key"},
            json={
                "model": "llama-3.2-1b",
                "messages": [{"role": "user", "content": prompt}]
            },
            timeout=60.0
        )

        if response.status_code == 200:
            return response.json()
        elif response.status_code == 503:
            # バックプレッシャー拒否 - 短い待機後リトライ
            wait_time = 2 ** attempt  # 指数バックオフ
            print(f"Server overloaded. Retrying in {wait_time}s...")
            time.sleep(wait_time)
        elif response.status_code == 504:
            # タイムアウト - 長めの待機後リトライ
            print(f"Timeout. Retrying in 5s...")
            time.sleep(5)
        else:
            response.raise_for_status()

    raise Exception("Max retries exceeded")
```

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Metrics」メニューでリアルタイム統計を確認

### 表示項目

| 項目 | 説明 |
|------|------|
| Queue Size | 現在の待機キューサイズ |
| Backpressure State | Normal / Warning / Overloaded |
| Request Latency (p95) | 95パーセンタイルレイテンシ |
| Cache Hit Rate | ノード選択キャッシュのヒット率 |
| Active Connections | 接続プールのアクティブ接続数 |

## パフォーマンスチューニング

### 高スループット向け

```toml
[load_manager]
max_waiting = 200        # 待機数を増加
default_timeout_secs = 60  # タイムアウトを延長

[connection_pool]
max_idle_per_host = 64   # 接続数を増加

[node_cache]
max_capacity = 2000      # キャッシュ容量を増加
ttl_secs = 10            # TTLを延長
```

### 低レイテンシ向け

```toml
[load_manager]
max_waiting = 50         # 待機数を制限
default_timeout_secs = 10  # タイムアウトを短縮
overload_threshold = 0.6   # 早めに拒否

[connection_pool]
max_idle_per_host = 16   # 接続数を制限

[node_cache]
ttl_secs = 2             # TTLを短縮
```

## 制限事項

| 項目 | 制限 |
|------|------|
| 最大待機数 | 1000リクエスト |
| タイムアウト | 1〜300秒 |
| 分散構成 | 非対応（単一ルーター） |
| 外部キュー | 非対応（Redis等） |
| 優先度制御 | 非対応 |

## 次のステップ

- Grafana ダッシュボードの構築
- アラート設定（過負荷時の通知）
- 本番環境向けチューニング
