# SPEC-5045f436: データモデル

## 概要

トークン累積統計機能で使用するデータモデルを定義する。
既存のrequest_historyテーブルを拡張し、Rust構造体に対応するフィールドを追加する。

## SQLiteスキーマ

### マイグレーション: 004_add_token_statistics.sql

```sql
-- request_history テーブルにトークンカラム追加
ALTER TABLE request_history ADD COLUMN input_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN output_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN total_tokens INTEGER;

-- 集計用インデックス
CREATE INDEX idx_request_history_tokens ON request_history(timestamp DESC, model);
CREATE INDEX idx_request_history_runtime_tokens ON request_history(runtime_id, timestamp DESC);
```

### 拡張後のrequest_historyテーブル

| カラム名 | 型 | NULL許可 | 説明 |
|----------|-----|----------|------|
| id | TEXT | NOT NULL | UUID (PRIMARY KEY) |
| timestamp | TEXT | NOT NULL | ISO8601形式 |
| request_type | TEXT | NOT NULL | Chat / Generate など |
| model | TEXT | NOT NULL | 使用モデル名 |
| runtime_id | TEXT | NOT NULL | 処理ノードID |
| node_machine_name | TEXT | NOT NULL | ノード名 |
| node_ip | TEXT | NOT NULL | ノードIP |
| client_ip | TEXT | NULL | クライアントIP |
| request_body | TEXT | NOT NULL | リクエスト本文（JSON） |
| response_body | TEXT | NULL | レスポンス本文（JSON） |
| duration_ms | INTEGER | NOT NULL | 処理時間(ms) |
| status | TEXT | NOT NULL | success / error |
| error_message | TEXT | NULL | エラーメッセージ |
| completed_at | TEXT | NOT NULL | 完了時刻 |
| **input_tokens** | INTEGER | NULL | 入力トークン数（新規） |
| **output_tokens** | INTEGER | NULL | 出力トークン数（新規） |
| **total_tokens** | INTEGER | NULL | 総トークン数（新規） |

## Rust構造体

### RequestResponseRecord（拡張）

```rust
// common/src/protocol.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestResponseRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub request_type: RequestType,
    pub model: String,
    pub runtime_id: Uuid,
    pub node_machine_name: String,
    pub node_ip: IpAddr,
    pub client_ip: Option<IpAddr>,
    pub request_body: serde_json::Value,
    pub response_body: Option<serde_json::Value>,
    pub duration_ms: u64,
    pub status: RecordStatus,
    pub completed_at: DateTime<Utc>,
    // 新規フィールド
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}
```

### NodeLoadState（拡張）

```rust
// router/src/balancer/mod.rs

#[derive(Debug, Clone, Default)]
struct NodeLoadState {
    last_metrics: Option<HealthMetrics>,
    assigned_active: u32,
    total_assigned: u64,
    success_count: u64,
    error_count: u64,
    total_latency_ms: u128,
    metrics_history: VecDeque<HealthMetrics>,
    initializing: bool,
    ready_models: Option<(u8, u8)>,
    // 新規フィールド
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_tokens: u64,
}
```

### DashboardNode（拡張）

```rust
// router/src/api/dashboard.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardNode {
    pub id: Uuid,
    pub name: String,
    pub ip: IpAddr,
    pub status: NodeStatus,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: Option<f32>,
    // 新規フィールド
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
    pub average_tokens_per_request: Option<f32>,
}
```

### DashboardStats（拡張）

```rust
// router/src/api/dashboard.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time_ms: Option<f32>,
    // 新規フィールド
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
}
```

### TokenStatistics（新規）

```rust
// router/src/api/dashboard.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStatistics {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub by_node: Vec<NodeTokenStats>,
    pub by_model: Vec<ModelTokenStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTokenStats {
    pub runtime_id: Uuid,
    pub node_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTokenStats {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
```

### DailyTokenStats（新規）

```rust
// router/src/api/dashboard.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTokenStats {
    pub date: String,  // YYYY-MM-DD
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyTokenStats {
    pub month: String,  // YYYY-MM
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
```

## OpenAI互換usageフィールド

トークン情報の取得元となるOpenAI互換APIレスポンスのusageフィールド形式：

```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gpt-3.5-turbo",
  "choices": [...],
  "usage": {
    "prompt_tokens": 123,
    "completion_tokens": 456,
    "total_tokens": 579
  }
}
```

### マッピング

| OpenAI usage | Rust構造体 | SQLiteカラム |
|--------------|------------|--------------|
| prompt_tokens | input_tokens | input_tokens |
| completion_tokens | output_tokens | output_tokens |
| total_tokens | total_tokens | total_tokens |

## 集計クエリ例

### ノード別累計

```sql
SELECT
    runtime_id,
    SUM(input_tokens) as total_input,
    SUM(output_tokens) as total_output,
    SUM(total_tokens) as total
FROM request_history
WHERE input_tokens IS NOT NULL
GROUP BY runtime_id;
```

### モデル別累計

```sql
SELECT
    model,
    SUM(input_tokens) as total_input,
    SUM(output_tokens) as total_output,
    SUM(total_tokens) as total
FROM request_history
WHERE input_tokens IS NOT NULL
GROUP BY model;
```

### 日次集計

```sql
SELECT
    DATE(timestamp) as date,
    SUM(input_tokens) as total_input,
    SUM(output_tokens) as total_output,
    SUM(total_tokens) as total
FROM request_history
WHERE input_tokens IS NOT NULL
  AND timestamp >= ?
  AND timestamp < ?
GROUP BY DATE(timestamp)
ORDER BY date DESC;
```

### 月次集計

```sql
SELECT
    STRFTIME('%Y-%m', timestamp) as month,
    SUM(input_tokens) as total_input,
    SUM(output_tokens) as total_output,
    SUM(total_tokens) as total
FROM request_history
WHERE input_tokens IS NOT NULL
  AND timestamp >= ?
  AND timestamp < ?
GROUP BY STRFTIME('%Y-%m', timestamp)
ORDER BY month DESC;
```

## 変更履歴

| 日付 | 変更内容 |
|------|----------|
| 2026-01-04 | 初版作成 |
