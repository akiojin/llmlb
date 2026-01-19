# リサーチ: Node/Router Log Retrieval API

## 調査目的

ノードおよびルーター経由でログを取得するHTTP APIの設計調査。

## ログ取得方式の比較

| 方式 | メリット | デメリット |
|------|---------|-----------|
| ファイル直接読み取り | シンプル | アクセス権限必要 |
| HTTP API | リモート対応 | 実装コスト |
| WebSocket | リアルタイム | 複雑度高い |

**選定**: HTTP API（シンプルかつリモート対応）

## tail実装の選択肢

### 方式1: ファイル末尾からの読み取り

```rust
fn tail_lines(path: &Path, n: usize) -> Vec<String> {
    // ファイル末尾からn行を読み取り
}
```

### 方式2: メモリバッファ

- ログ出力時にリングバッファに保持
- メモリ使用量増加

**選定**: ファイル末尾読み取り（メモリ効率重視）

## レスポンスサイズ制限

### 計算

- 1行最大: 1KB
- tail最大: 1000行
- 最大サイズ: 1KB × 1000 = 1MB

### 10MB超過時の対応

- HTTP 413 Payload Too Large を返却
- tail パラメータの減少を推奨

## ルータープロキシ設計

### プロキシフロー

```text
[クライアント]
     |
     v
[ルーター] GET /v0/nodes/:node_id/logs?tail=N
     |
     v
[ノード検索]
     |
     +-- 見つからない --> 404 Not Found
     |
     v
[ノードへリクエスト] GET /v0/logs?tail=N
     |
     +-- タイムアウト/エラー --> 502 Bad Gateway
     |
     v
[レスポンス転送]
```

### タイムアウト設定

- デフォルト: 5秒
- NFR-001: ノード応答 + 500ms 以内

## ログエントリ形式

### 既存JSONL形式との互換

```json
{
  "timestamp": "2025-01-02T10:30:00.123Z",
  "level": "INFO",
  "target": "allm::api",
  "message": "Request received"
}
```

### APIレスポンス形式

```json
{
  "entries": [
    {"timestamp": "...", "level": "INFO", ...}
  ],
  "path": "/var/log/allm/current.log"
}
```

## セキュリティ考慮

### 現状

- 認証不要（既存APIポリシーに準拠）
- 内部ネットワーク前提

### 将来対応

- APIキー/JWT認証の追加
- ログ内容のフィルタリング（機密情報除去）

## 参考資料

- SPEC-1970e39f: 構造化ロギング強化
- [RFC 7807](https://tools.ietf.org/html/rfc7807): Problem Details for HTTP APIs
