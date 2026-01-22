# データモデル: Node/Router Log Retrieval API

## エンティティ定義

### LogEntry

個別のログエントリ。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Uuid>,
    #[serde(flatten)]
    pub fields: HashMap<String, serde_json::Value>,
}
```

### LogLevel

ログレベル。

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

### LogsResponse

ログ取得APIのレスポンス。

```rust
pub struct LogsResponse {
    /// ログエントリの配列
    pub entries: Vec<LogEntry>,
    /// ログファイルのパス
    pub path: String,
}
```

### LogsQuery

クエリパラメータ。

```rust
pub struct LogsQuery {
    /// 取得する行数（1-1000、デフォルト200）
    #[serde(default = "default_tail")]
    pub tail: usize,
}

fn default_tail() -> usize { 200 }

impl LogsQuery {
    pub fn clamp_tail(&mut self) {
        self.tail = self.tail.clamp(1, 1000);
    }
}
```

## APIエンドポイント定義

### Node API

| エンドポイント | メソッド | 説明 |
|---------------|---------|------|
| /v0/logs | GET | ノード自身のログを取得 |

### Router Proxy API

| エンドポイント | メソッド | 説明 |
|---------------|---------|------|
| /v0/nodes/:runtime_id/logs | GET | 指定ノードのログをプロキシ |

## エラーモデル

### LogError

```rust
pub enum LogError {
    FileNotFound(PathBuf),
    IoError(std::io::Error),
    PayloadTooLarge { size: usize, limit: usize },
}
```

### ProxyError

```rust
pub enum ProxyError {
    NodeNotFound(Uuid),
    Timeout,
    ConnectionFailed(String),
    NodeError { status: u16, body: String },
}
```

### エラーレスポンスJSON

```json
{
  "error": "Node 'abc123' not found"
}
```

## レスポンス例

### 正常レスポンス (200 OK)

```json
{
  "entries": [
    {
      "timestamp": "2025-01-02T10:30:00.123Z",
      "level": "INFO",
      "target": "xllm::api::router_client",
      "message": "Heartbeat sent",
      "runtime_id": "550e8400-e29b-41d4-a716-446655440000"
    },
    {
      "timestamp": "2025-01-02T10:30:05.456Z",
      "level": "DEBUG",
      "target": "xllm::inference",
      "message": "Model loaded",
      "model": "llama-3.1-8b"
    }
  ],
  "path": "/home/user/.llmlb/logs/current.jsonl"
}
```

### ファイル未存在時 (200 OK)

```json
{
  "entries": [],
  "path": "/home/user/.llmlb/logs/current.jsonl"
}
```

### プロキシエラー (502 Bad Gateway)

```json
{
  "error": "Failed to connect to node: connection refused"
}
```

## サイズ制限

| 項目 | 値 |
|------|-----|
| tail最小値 | 1 |
| tail最大値 | 1000 |
| tailデフォルト | 200 |
| レスポンス最大サイズ | 10MB |
