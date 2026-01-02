# データモデル: 構造化ロギング強化

## エンティティ定義

### LogEntry

1つのログイベントを表現する共通構造。

```rust
pub struct LogEntry {
    /// タイムスタンプ（ISO 8601形式）
    pub timestamp: DateTime<Utc>,
    /// ログレベル
    pub level: LogLevel,
    /// ログ出力元モジュール
    pub target: String,
    /// 人間可読なメッセージ
    pub message: String,
    /// リクエスト追跡用ID
    pub request_id: Option<Uuid>,
    /// コンポーネント識別子
    pub component: Component,
    /// イベント固有の追加フィールド
    pub fields: HashMap<String, Value>,
}
```

### LogLevel

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
```

### Component

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Router,
    Node,
}
```

## ログイベント定義

### リクエスト受信 (Router)

```json
{
  "timestamp": "2025-01-02T10:30:00.123Z",
  "level": "INFO",
  "target": "llm_router::api::openai",
  "message": "Request received",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "component": "router",
  "endpoint": "/v1/chat/completions",
  "model": "llama-3.1-8b",
  "client_ip": "192.168.1.100"
}
```

### ノード選択成功 (Router)

```json
{
  "timestamp": "2025-01-02T10:30:00.125Z",
  "level": "INFO",
  "target": "llm_router::routing",
  "message": "Node selected",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "component": "router",
  "node_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "node_ip": "192.168.1.10"
}
```

### ノード選択失敗 (Router)

```json
{
  "timestamp": "2025-01-02T10:30:00.125Z",
  "level": "ERROR",
  "target": "llm_router::routing",
  "message": "No available nodes",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "component": "router",
  "error": "NoNodesAvailable",
  "model": "llama-3.1-8b"
}
```

### 推論完了 (Node)

```json
{
  "timestamp": "2025-01-02T10:30:01.500Z",
  "level": "INFO",
  "target": "node::inference",
  "message": "Inference completed",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "component": "node",
  "model": "llama-3.1-8b",
  "prompt_tokens": 128,
  "generated_tokens": 256,
  "duration_ms": 1375
}
```

## フィールド定義

### 共通フィールド

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| timestamp | string (ISO 8601) | Yes | イベント発生時刻 |
| level | string | Yes | ログレベル |
| target | string | Yes | 出力元モジュール |
| message | string | Yes | 人間可読メッセージ |
| request_id | string (UUID) | No | リクエスト追跡ID |
| component | string | Yes | router / node |

### Router固有フィールド

| フィールド | 型 | イベント | 説明 |
|-----------|-----|---------|------|
| endpoint | string | リクエスト受信 | APIエンドポイント |
| model | string | リクエスト受信 | モデル名 |
| client_ip | string | リクエスト受信 | クライアントIP |
| node_id | string (UUID) | ノード選択 | 選択されたノードID |
| node_ip | string | ノード選択 | ノードIPアドレス |
| error | string | エラー | エラー種別 |

### Node固有フィールド

| フィールド | 型 | イベント | 説明 |
|-----------|-----|---------|------|
| model | string | 推論 | モデル名 |
| prompt_tokens | integer | 推論完了 | 入力トークン数 |
| generated_tokens | integer | 推論完了 | 生成トークン数 |
| duration_ms | integer | 推論完了 | 処理時間（ミリ秒） |

## ログ出力先

### ファイルパス

```text
~/.llm-router/logs/
├── router.log           # ルーターログ（現在）
├── router.log.2025-01-01 # ローテーション済み
├── node.log             # ノードログ（現在）
└── node.log.2025-01-01  # ローテーション済み
```

### 保持期間

- ローテーション: 日次
- 保持期間: 7日間
- 最大サイズ: 10MB/ファイル
