# データモデル: エンドポイント×モデル単位TPS可視化

**機能ID**: `SPEC-4bb5b55f` | **日付**: 2026-02-19

## DBスキーマ変更

### endpoint_daily_stats テーブル（ALTER TABLE）

既存テーブルに2カラムを追加:

| カラム名 | 型 | デフォルト | 説明 |
|----------|------|-----------|------|
| `total_output_tokens` | INTEGER | 0 | 日次出力トークン累計 |
| `total_duration_ms` | INTEGER | 0 | 日次処理時間累計（ミリ秒） |

マイグレーションSQL:

```sql
ALTER TABLE endpoint_daily_stats
  ADD COLUMN total_output_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoint_daily_stats
  ADD COLUMN total_duration_ms INTEGER NOT NULL DEFAULT 0;
```

## インメモリデータ構造

### ModelTpsState（新規）

エンドポイント×モデル単位のTPS EMA状態:

| フィールド | 型 | 説明 |
|-----------|------|------|
| `tps_ema` | `Option<f64>` | EMA平滑化されたTPS値（None=未計測） |
| `request_count` | `u64` | リクエスト完了数 |
| `total_output_tokens` | `u64` | 出力トークン累計 |
| `total_duration_ms` | `u64` | 処理時間累計（ミリ秒） |

### TpsTracker（新規、LoadManagerに保持）

| フィールド | 型 | 説明 |
|-----------|------|------|
| `states` | `HashMap<(Uuid, String), ModelTpsState>` | (endpoint_id, model_id) → TPS状態 |

## APIレスポンス型

### ModelTpsEntry（REST APIレスポンス）

GET `/api/endpoints/{id}/model-tps` のレスポンス要素:

| フィールド | 型 | JSON名 | 説明 |
|-----------|------|--------|------|
| `model_id` | `String` | `model_id` | モデルID |
| `tps` | `Option<f64>` | `tps` | TPS（EMA, 小数点1位） |
| `request_count` | `u64` | `request_count` | リクエスト完了数 |
| `total_output_tokens` | `u64` | `total_output_tokens` | 出力トークン累計 |
| `average_duration_ms` | `Option<f64>` | `average_duration_ms` | 平均処理時間（ミリ秒） |

### DashboardEvent::TpsUpdated（WebSocket通知）

| フィールド | 型 | 説明 |
|-----------|------|------|
| `endpoint_id` | `Uuid` | エンドポイントID |
| `model_id` | `String` | モデルID |
| `tps` | `f64` | 更新後のTPS値 |
| `output_tokens` | `u32` | このリクエストの出力トークン数 |
| `duration_ms` | `u64` | このリクエストの処理時間 |

## 関連既存型

- `TokenUsage` (`token/mod.rs`): トークン使用量（input/output/total）
- `EndpointLoadState` (`balancer/mod.rs`): エンドポイント単位の負荷状態
- `DailyStatEntry` (`db/endpoint_daily_stats.rs`): 日次統計エントリ
- `ModelStatEntry` (`db/endpoint_daily_stats.rs`): モデル別統計エントリ
