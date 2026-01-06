# クイックスタート: 構造化ロギング強化

## 概要

ルーターとノードの構造化ログを確認・分析する方法を説明する。

## ログの確認

### ログファイルの場所

```bash
# ルーターログ
~/.llm-router/logs/router.log

# ノードログ
~/.llm-router/logs/node.log
```

### リアルタイム監視

```bash
# ルーターログをリアルタイムで監視
tail -f ~/.llm-router/logs/router.log

# JSON整形して表示
tail -f ~/.llm-router/logs/router.log | jq '.'
```

## ログレベル設定

### 環境変数

```bash
# ルーター
export RUST_LOG=llm_router=info

# デバッグレベル
export RUST_LOG=llm_router=debug

# 特定モジュールのみ
export RUST_LOG=llm_router::api::openai=debug
```

### ノード側

```bash
# 環境変数で設定
export LOG_LEVEL=info
```

## jqを使った分析

### エラーログの抽出

```bash
# すべてのエラーを表示
cat router.log | jq 'select(.level == "ERROR")'

# 特定のリクエストIDを追跡
cat router.log | jq 'select(.request_id == "550e8400-e29b-41d4-a716-446655440000")'
```

### 統計情報の算出

```bash
# エラー数をカウント
cat router.log | jq 'select(.level == "ERROR")' | wc -l

# モデル別リクエスト数
cat router.log | jq -r 'select(.message == "Request received") | .model' | sort | uniq -c
```

### レスポンス時間分析

```bash
# 平均処理時間（ノードログから）
cat node.log | jq 'select(.duration_ms) | .duration_ms' | awk '{sum+=$1; n++} END {print sum/n}'

# 遅いリクエスト（1秒以上）
cat node.log | jq 'select(.duration_ms > 1000)'
```

## リクエストトレース

### リクエストIDによる追跡

1. クライアントでリクエストを送信
2. レスポンスヘッダーから`X-Request-Id`を取得
3. ログをフィルタリング

```bash
# リクエストIDで全ログを抽出
REQUEST_ID="550e8400-e29b-41d4-a716-446655440000"
cat router.log node.log | jq "select(.request_id == \"$REQUEST_ID\")" | jq -s 'sort_by(.timestamp)'
```

## ログ出力例

### 正常なリクエストフロー

```json
{"timestamp":"2025-01-02T10:30:00.123Z","level":"INFO","message":"Request received","request_id":"...","endpoint":"/v1/chat/completions","model":"llama-3.1-8b"}
{"timestamp":"2025-01-02T10:30:00.125Z","level":"INFO","message":"Node selected","request_id":"...","node_id":"...","node_ip":"192.168.1.10"}
{"timestamp":"2025-01-02T10:30:01.500Z","level":"INFO","message":"Inference completed","request_id":"...","duration_ms":1375}
{"timestamp":"2025-01-02T10:30:01.502Z","level":"INFO","message":"Response sent","request_id":"...","status":200}
```

### エラー発生時

```json
{"timestamp":"2025-01-02T10:30:00.123Z","level":"INFO","message":"Request received","request_id":"...","endpoint":"/v1/chat/completions","model":"llama-3.1-8b"}
{"timestamp":"2025-01-02T10:30:00.125Z","level":"ERROR","message":"No available nodes","request_id":"...","error":"NoNodesAvailable"}
```

## ログローテーション

### 自動ローテーション

- 日次でローテーション
- 7日以上前のログは自動削除

### 手動クリーンアップ

```bash
# 7日以上前のログを削除
find ~/.llm-router/logs -name "*.log.*" -mtime +7 -delete
```

## トラブルシューティング

### ログが出力されない

```bash
# 環境変数を確認
echo $RUST_LOG

# 書き込み権限を確認
ls -la ~/.llm-router/logs/
```

### ログファイルが大きすぎる

```bash
# ファイルサイズを確認
du -h ~/.llm-router/logs/

# 古いログを圧縮
gzip ~/.llm-router/logs/router.log.2025-01-01
```
