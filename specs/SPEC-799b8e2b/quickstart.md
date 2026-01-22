# クイックスタート: 共通ログシステム

## 前提条件

| 項目 | 要件 |
|------|------|
| ルーター | ビルド済み（Rust） |
| ノード | ビルド済み（C++） |
| ログディレクトリ | 書き込み権限あり |

## 基本的な使用例

### 環境変数によるログ設定

```bash
# ログディレクトリの指定（デフォルト: ~/.llmlb/logs）
export LLM_LOG_DIR=/var/log/llmlb

# ログレベルの設定（trace, debug, info, warn, error）
export LLM_LOG_LEVEL=debug

# ログ保持日数（デフォルト: 7日）
export LLM_LOG_RETENTION_DAYS=14
```

### ルーター起動

```bash
# デフォルト設定で起動
llmlb

# デバッグログを有効にして起動
LLM_LOG_LEVEL=debug llmlb
```

### ノード起動

```bash
# デフォルト設定で起動
xllm

# トレースログを有効にして起動
LLM_LOG_LEVEL=trace xllm
```

## ログファイルの確認

### ファイル構造

```text
~/.llmlb/logs/
├── llmlb.jsonl.2025-12-01    # ルーターログ（日付別）
├── llmlb.jsonl.2025-12-02
├── xllm.jsonl.2025-12-01      # ノードログ（日付別）
└── xllm.jsonl.2025-12-02
```

### ログの閲覧

```bash
# 最新のルーターログを表示
tail -f ~/.llmlb/logs/llmlb.jsonl.$(date +%Y-%m-%d)

# エラーログのみをフィルタ
cat ~/.llmlb/logs/llmlb.jsonl.* | jq 'select(.level == "error")'

# 特定カテゴリのログを表示
cat ~/.llmlb/logs/llmlb.jsonl.* | jq 'select(.category == "api")'

# リクエストIDでトレース
cat ~/.llmlb/logs/*.jsonl.* | jq 'select(.request_id == "abc123")'
```

### Python でのログ解析

```python
import json
from pathlib import Path
from datetime import datetime

LOG_DIR = Path.home() / ".llmlb" / "logs"

def read_logs(date: str = None, level: str = None, category: str = None):
    """ログを読み込んでフィルタリング"""
    if date is None:
        date = datetime.now().strftime("%Y-%m-%d")

    logs = []
    for log_file in LOG_DIR.glob(f"*.jsonl.{date}"):
        with open(log_file) as f:
            for line in f:
                entry = json.loads(line)
                if level and entry["level"] != level:
                    continue
                if category and entry["category"] != category:
                    continue
                logs.append(entry)

    return sorted(logs, key=lambda x: x["ts"])

# エラーログを取得
errors = read_logs(level="error")
for e in errors:
    print(f"[{e['ts']}] {e['msg']}")

# API関連ログを取得
api_logs = read_logs(category="api")
for log in api_logs[-10:]:  # 最新10件
    print(f"{log['ts']}: {log['msg']}")
```

## API経由でのログ取得

### GET /v0/logs

```bash
# 最新ログを取得
curl http://localhost:8080/v0/logs \
  -H "Authorization: Bearer sk-your-api-key"

# フィルタ付きで取得
curl "http://localhost:8080/v0/logs?level=error&category=api&limit=50" \
  -H "Authorization: Bearer sk-your-api-key"
```

### レスポンス例

```json
{
  "logs": [
    {
      "ts": "2025-12-01T12:00:00.000Z",
      "level": "info",
      "category": "api",
      "msg": "Request received",
      "request_id": "abc123"
    },
    {
      "ts": "2025-12-01T12:00:01.000Z",
      "level": "info",
      "category": "inference",
      "msg": "Generation complete",
      "model": "llama-3.2-1b",
      "tokens": 50
    }
  ],
  "total": 2,
  "has_more": false
}
```

### クエリパラメータ

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| `level` | string | - | ログレベルでフィルタ |
| `category` | string | - | カテゴリでフィルタ |
| `since` | string | - | ISO 8601形式の開始時刻 |
| `until` | string | - | ISO 8601形式の終了時刻 |
| `request_id` | string | - | リクエストIDでフィルタ |
| `model` | string | - | モデル名でフィルタ |
| `limit` | integer | 100 | 取得件数（最大1000） |
| `offset` | integer | 0 | オフセット |

## ダッシュボードでの確認

1. `http://localhost:8080` にアクセス
2. admin / test でログイン
3. 「Logs」メニューを選択
4. リアルタイムでログストリームを確認

### ダッシュボード機能

| 機能 | 説明 |
|------|------|
| リアルタイム表示 | WebSocketで新規ログを自動更新 |
| レベルフィルタ | error/warn/info/debug切り替え |
| カテゴリフィルタ | api/inference/model等で絞り込み |
| 検索 | テキスト検索 |
| エクスポート | JSON/CSVでダウンロード |

## ログカテゴリ一覧

| カテゴリ | 説明 | 主な出力内容 |
|---------|------|-------------|
| `system` | システム | 起動/終了/設定変更 |
| `api` | API | HTTPリクエスト/レスポンス |
| `model` | モデル | ロード/アンロード |
| `inference` | 推論 | 生成処理/トークン数 |
| `sync` | 同期 | モデル同期 |
| `repair` | 修復 | 自動修復 |
| `health` | ヘルス | ヘルスチェック/ハートビート |

## エラーハンドリング

### ログディレクトリが存在しない場合

```bash
# 自動作成される（権限がある場合）
# 権限エラーの場合は起動時に警告
[WARN] Failed to create log directory: Permission denied
[WARN] Falling back to console-only logging
```

### ディスク容量不足

```bash
# ログ書き込み失敗時の動作
[WARN] Log write failed: No space left on device
[WARN] Switching to console-only mode
```

## 制限事項

| 項目 | 制限 |
|------|------|
| ログフォーマット | JSONLのみ |
| ローテーション | 日次（時間指定不可） |
| 圧縮 | 非対応 |
| リモート転送 | 非対応（外部ツール利用） |
| 構造化クエリ | 非対応（jq等で処理） |
| ログ集約 | 非対応（ノード毎に個別ファイル） |

## トラブルシューティング

### ログが出力されない

```bash
# ログレベルを確認
echo $LLM_LOG_LEVEL

# ディレクトリの権限を確認
ls -la ~/.llmlb/logs/

# ディスク容量を確認
df -h ~/.llmlb/
```

### ログファイルが大きすぎる

```bash
# 保持日数を短くする
export LLM_LOG_RETENTION_DAYS=3

# 手動で古いログを削除
find ~/.llmlb/logs/ -name "*.jsonl.*" -mtime +7 -delete
```

## 次のステップ

- ログ集約ツール（Loki, Elasticsearch）との連携
- アラート設定（エラーログ監視）
- カスタムログフィールドの追加
