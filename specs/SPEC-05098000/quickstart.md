# クイックスタート: 推論中ノードへの多重リクエストキューイング

## 前提条件

| 項目 | 要件 |
|------|------|
| ルーター | SPEC-589f2df1（ロードバランシング）実装済み |
| ノード | 1台以上のオンラインノード |
| API | SPEC-63acef08（統一APIプロキシ）経由 |

## 基本的な使用例

### シンプルなリクエスト送信

```bash
# 通常のリクエスト（キューが空の場合、即座に処理）
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### キュー状態の確認（レスポンスヘッダー）

```bash
# レスポンスヘッダーを表示
curl -i -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Hello"}]
  }'

# キュー待機時のヘッダー例:
# X-Queue-Position: 3
# X-Estimated-Wait: 30
```

### Python での並行リクエスト

```python
import asyncio
import aiohttp

async def send_request(session, prompt):
    async with session.post(
        "http://localhost:8080/v1/chat/completions",
        headers={"Authorization": "Bearer sk-your-api-key"},
        json={
            "model": "llama-3.2-1b",
            "messages": [{"role": "user", "content": prompt}]
        }
    ) as response:
        # キュー状態をチェック
        if "X-Queue-Position" in response.headers:
            print(f"キュー位置: {response.headers['X-Queue-Position']}")
            print(f"推定待ち時間: {response.headers.get('X-Estimated-Wait', 'N/A')}秒")

        return await response.json()

async def main():
    async with aiohttp.ClientSession() as session:
        # 5件の並行リクエスト
        tasks = [
            send_request(session, f"質問{i}: Pythonについて教えて")
            for i in range(5)
        ]
        results = await asyncio.gather(*tasks)
        for i, result in enumerate(results):
            print(f"応答{i}: {result['choices'][0]['message']['content'][:50]}...")

asyncio.run(main())
```

## エラーハンドリング

### キュー満杯時 (HTTP 429)

```bash
# キューが満杯の場合
curl -i -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3.2-1b", "messages": [{"role": "user", "content": "Hello"}]}'

# レスポンス例:
# HTTP/1.1 429 Too Many Requests
# Retry-After: 5
# Content-Type: application/json
#
# {
#   "error": {
#     "message": "キューが満杯です。5秒後に再試行してください。",
#     "type": "queue_full",
#     "code": "queue_full"
#   }
# }
```

### タイムアウト時 (HTTP 504)

```bash
# キュー待機タイムアウトの場合
# HTTP/1.1 504 Gateway Timeout
# Content-Type: application/json
#
# {
#   "error": {
#     "message": "キュー待機タイムアウト（60秒）。再試行してください。",
#     "type": "queue_timeout",
#     "code": "queue_timeout"
#   }
# }
```

### Python でのリトライ処理

```python
import time
import requests

def send_with_retry(prompt, max_retries=3):
    for attempt in range(max_retries):
        response = requests.post(
            "http://localhost:8080/v1/chat/completions",
            headers={"Authorization": "Bearer sk-your-api-key"},
            json={
                "model": "llama-3.2-1b",
                "messages": [{"role": "user", "content": prompt}]
            }
        )

        if response.status_code == 200:
            return response.json()
        elif response.status_code == 429:
            retry_after = int(response.headers.get("Retry-After", 5))
            print(f"キュー満杯。{retry_after}秒後にリトライ...")
            time.sleep(retry_after)
        elif response.status_code == 504:
            print(f"タイムアウト。リトライ {attempt + 1}/{max_retries}")
        else:
            response.raise_for_status()

    raise Exception("最大リトライ回数を超えました")
```

## ダッシュボードでの確認

ダッシュボード（`http://localhost:8080`）の「キュー状態」パネルで以下を確認可能:

| 項目 | 説明 |
|------|------|
| 待機中 | キュー内で待機しているリクエスト数 |
| 処理中 | 各ノードで処理中のリクエスト数 |
| 平均待機時間 | 過去1時間の平均キュー待機時間 |

## 制限事項

| 項目 | 制限 |
|------|------|
| 最大キューサイズ | デフォルト100件（設定変更可能） |
| キュー待機タイムアウト | デフォルト60秒（設定変更可能） |
| ノード同時処理 | 1件のみ（単発リクエスト制限） |
| 優先度制御 | 未対応（ラウンドロビンのみ） |
| バッチ処理 | 未対応 |

## 設定変更

環境変数またはconfig.tomlで設定可能:

```toml
[queue]
# 最大キューサイズ
max_size = 100
# キュー待機タイムアウト（秒）
timeout_secs = 60
# フェアネス制御の有効化
enable_fairness = true
```

```bash
# 環境変数での設定
export LLMLB_QUEUE_MAX_SIZE=200
export LLMLB_QUEUE_TIMEOUT_SECS=120
```
