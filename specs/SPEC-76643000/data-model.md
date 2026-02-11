# データモデル: エンドポイント単位リクエスト統計

**機能ID**: `SPEC-76643000` | **日付**: 2026-02-10

## エンティティ関連図

```text
endpoints (既存テーブル拡張)
├── total_requests        (新規カラム)
├── successful_requests   (新規カラム)
└── failed_requests       (新規カラム)
         │
         │ endpoint_id (FK制約なし)
         ▼
endpoint_daily_stats (新規テーブル)
├── endpoint_id
├── model_id
├── date
├── total_requests
├── successful_requests
└── failed_requests
```

## テーブル定義

### endpoints テーブル (既存 - カラム追加)

| カラム | 型 | 制約 | 説明 |
|--------|------|------|------|
| total_requests | INTEGER | NOT NULL DEFAULT 0 | 累計リクエスト数 |
| successful_requests | INTEGER | NOT NULL DEFAULT 0 | 累計成功リクエスト数 |
| failed_requests | INTEGER | NOT NULL DEFAULT 0 | 累計失敗リクエスト数 |

### endpoint_daily_stats テーブル (新規)

| カラム | 型 | 制約 | 説明 |
|--------|------|------|------|
| endpoint_id | TEXT | NOT NULL | エンドポイントID (UUID) |
| model_id | TEXT | NOT NULL | モデルID |
| date | TEXT | NOT NULL | 日付 (YYYY-MM-DD, サーバーローカル時間) |
| total_requests | INTEGER | NOT NULL DEFAULT 0 | 当日のリクエスト合計数 |
| successful_requests | INTEGER | NOT NULL DEFAULT 0 | 当日の成功リクエスト数 |
| failed_requests | INTEGER | NOT NULL DEFAULT 0 | 当日の失敗リクエスト数 |

**主キー**: (endpoint_id, model_id, date)

**インデックス**:

- `idx_daily_stats_endpoint_date` ON (endpoint_id, date) - 日次チャート用
- `idx_daily_stats_date` ON (date) - 日次バッチ用

**制約**:

- FOREIGN KEY制約なし（エンドポイント削除時に孤児データを許容）
- 保持期限なし（永続保存）

## Rust構造体

### Endpoint (既存 - フィールド追加)

```text
Endpoint {
    ...既存フィールド...
    total_requests: i64,
    successful_requests: i64,
    failed_requests: i64,
}
```

### EndpointDailyStats (新規)

```text
EndpointDailyStats {
    endpoint_id: Uuid,
    model_id: String,
    date: String,            // YYYY-MM-DD形式
    total_requests: i64,
    successful_requests: i64,
    failed_requests: i64,
}
```

### DashboardEndpoint (既存 - フィールド追加)

```text
DashboardEndpoint {
    ...既存フィールド...
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}
```

## API レスポンス型

### GET /api/dashboard/endpoints/:id/stats/daily

```text
EndpointDailyStatsResponse {
    endpoint_id: String,
    days: Vec<DailyStatEntry>,
}

DailyStatEntry {
    date: String,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}
```

### GET /api/dashboard/endpoints/:id/stats/models

```text
EndpointModelStatsResponse {
    endpoint_id: String,
    models: Vec<ModelStatEntry>,
}

ModelStatEntry {
    model_id: String,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
}
```

## マイグレーションSQL

```sql
-- 014_add_endpoint_request_stats.sql

-- endpointsテーブルにカウンタカラム追加
ALTER TABLE endpoints ADD COLUMN total_requests INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN successful_requests INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN failed_requests INTEGER NOT NULL DEFAULT 0;

-- 日次集計テーブル作成
CREATE TABLE IF NOT EXISTS endpoint_daily_stats (
    endpoint_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    date TEXT NOT NULL,
    total_requests INTEGER NOT NULL DEFAULT 0,
    successful_requests INTEGER NOT NULL DEFAULT 0,
    failed_requests INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (endpoint_id, model_id, date)
);

CREATE INDEX IF NOT EXISTS idx_daily_stats_endpoint_date
    ON endpoint_daily_stats (endpoint_id, date);

CREATE INDEX IF NOT EXISTS idx_daily_stats_date
    ON endpoint_daily_stats (date);
```
