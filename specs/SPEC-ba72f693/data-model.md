# データモデル: ダッシュボードメトリクスの永続化と復元

**機能ID**: `SPEC-ba72f693` | **日付**: 2026-02-24

## 概要

本機能ではDBスキーマの変更は不要。既存テーブルのデータを活用して
インメモリキャッシュとの同期および起動時復元を実現する。

## 既存テーブル（変更なし）

### endpoints

リクエストカウンタとレイテンシの永続化元。

| カラム | 型 | 用途 |
|--------|-----|------|
| id | TEXT (UUID) | エンドポイントID |
| total_requests | INTEGER | 累計リクエスト数 |
| successful_requests | INTEGER | 成功リクエスト数 |
| failed_requests | INTEGER | 失敗リクエスト数 |
| latency_ms | REAL (nullable) | 最新レイテンシ (ms) |
| status | TEXT | オンライン/オフライン状態 |

**Bug 1関連**: `total_requests`/`successful_requests`/`failed_requests`を
DB更新後にキャッシュへ同期。

**Bug 3関連**: `latency_ms`をオフライン遷移時にNULL上書きせず保持。

### endpoint_daily_stats

TPS復元のデータソース。

| カラム | 型 | 用途 |
|--------|-----|------|
| endpoint_id | TEXT (UUID) | エンドポイントID |
| model_id | TEXT | モデルID |
| date | TEXT | 日付 (YYYY-MM-DD) |
| total_output_tokens | INTEGER | 累計出力トークン数 |
| total_duration_ms | INTEGER | 累計処理時間 (ms) |
| total_requests | INTEGER | 累計リクエスト数 |

**Bug 4関連**: 当日データからTPS近似値を算出。

### request_history

リクエスト履歴復元のデータソース。

| カラム | 型 | 用途 |
|--------|-----|------|
| id | INTEGER | レコードID |
| endpoint_id | TEXT (UUID) | エンドポイントID |
| status_code | INTEGER (nullable) | HTTPステータスコード |
| created_at | TEXT (datetime) | 作成日時 (UTC) |

**Bug 5関連**: 直近60分のレコードを分単位で集計しVecDequeにseed。

## インメモリ構造（変更なし）

### EndpointRegistry キャッシュ

```text
Arc<RwLock<HashMap<Uuid, Endpoint>>>
```

`Endpoint`構造体に`total_requests`/`successful_requests`/`failed_requests`/
`latency_ms`を保持。DB更新と同期して更新される。

### TpsTrackerMap

```text
HashMap<(Uuid, String, TpsApiKind), ModelTpsState>
```

`ModelTpsState`に`tps_ema: Option<f64>`を保持。
起動時に`endpoint_daily_stats`からseedされる。

### Request History

```text
VecDeque<PerMinuteEntry> (60要素固定)
```

`PerMinuteEntry`に`success_count`/`failure_count`を保持。
起動時に`request_history`テーブルからseedされる。

## データフロー

### リクエスト処理時 (Bug 1)

```text
リクエスト完了
  → EndpointRegistry.increment_request_counters()
    → DB UPDATE (endpoints テーブル)
    → キャッシュ更新 (HashMap内のEndpoint)
  → ダッシュボードAPI → キャッシュから読み取り → リアルタイム反映
```

### ヘルスチェック失敗時 (Bug 3)

```text
ヘルスチェック失敗
  → update_endpoint_status(latency_ms=None)
    → DB: COALESCE(NULL, latency_ms) → 既存値保持
    → キャッシュ: None渡しはスキップ → 既存値保持
```

### サーバー起動時 (Bug 2, 4, 5)

```text
サーバー起動
  → EndpointRegistry初期化 (DBからendpoints読み込み → latency_ms含む)
  → seed_history_from_db (request_history → VecDeque)
  → seed_tps_from_db (endpoint_daily_stats → TpsTrackerMap)
  → collect_stats時にlatency_msフォールバック計算 (Bug 2)
```
