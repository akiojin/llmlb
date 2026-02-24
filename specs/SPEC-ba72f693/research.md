# 技術リサーチ: ダッシュボードメトリクスの永続化と復元

**機能ID**: `SPEC-ba72f693` | **日付**: 2026-02-24

## 調査対象

インメモリキャッシュとDB間の同期不足および起動時復元ロジック欠如に関する
既存コードの調査結果。

## Bug 1: リクエストカウンタのキャッシュ未同期

### 現状

- `db::increment_request_counters` (endpoint_daily_stats.rs) はDBのみ更新
- `EndpointRegistry`のキャッシュ (`Arc<RwLock<HashMap<Uuid, Endpoint>>>`) は
  リクエスト処理時に更新されない
- ダッシュボードAPI (`/api/dashboard/endpoints`) はキャッシュから読み取るため、
  DB上のカウンタ増加がリアルタイムに反映されない

### 解決アプローチ

- `EndpointRegistry`に`increment_request_counters`メソッドを追加
- DB更新 → キャッシュ更新の順で同期
- `record_endpoint_request_stats` (proxy.rs) の引数を
  `SqlitePool` → `EndpointRegistry` に変更して呼び出し

### 影響範囲

- `proxy.rs`: `TpsTrackingState`構造体と`record_endpoint_request_stats`関数
- `openai.rs`: 5箇所の呼び出し元
- `responses.rs`: 4箇所の呼び出し元

## Bug 2: Avg Response Timeのインメモリ依存

### 現状

- `collect_stats` (dashboard.rs) は `LoadManager` のインメモリ統計のみ参照
- サーバー再起動後、`LoadManager`は初期状態のため`average_response_time_ms = None`
- エンドポイントのDB永続化済み`latency_ms`は参照されていない

### 解決アプローチ

- `collect_stats`内で`average_response_time_ms`が`None`の場合のフォールバック
- オンラインエンドポイントの`latency_ms`の平均値を計算
- データモデルの変更不要（既存の`Endpoint.latency_ms`を利用）

## Bug 3: オフラインLatency上書き

### 現状

- `update_endpoint_status` (db/endpoints.rs) は`latency_ms`パラメータを
  無条件でSET
- ヘルスチェック失敗時に`latency_ms = None`が渡され、既存値が消失
- キャッシュ側も同様に無条件上書き

### 解決アプローチ

- DB: `COALESCE(?, latency_ms)` で`None`渡し時は既存値保持
- キャッシュ: `if let Some(v)` ガードで`None`時は上書きスキップ

## Bug 4: TPS起動時復元なし

### 現状

- `TpsTrackerMap` (balancer/types.rs) はインメモリのみ
- `HashMap<(Uuid, String, TpsApiKind), ModelTpsState>` で管理
- サーバー再起動で全エントリが消失し、全TPS値が "—" 表示

### 解決アプローチ

- `endpoint_daily_stats`テーブルに当日の`total_output_tokens`/`total_duration_ms`が
  既に永続化されている
- 起動時にこれらからTPS = tokens / (duration_ms / 1000) で近似値を算出
- EMA値の完全な復元は不可能だが、日次集計からの近似は実用上十分

### データソース

```sql
SELECT endpoint_id, model_id, total_output_tokens, total_duration_ms, total_requests
FROM endpoint_daily_stats
WHERE date = ?
```

## Bug 5: リクエスト履歴起動時復元なし

### 現状

- リクエスト履歴は`VecDeque<PerMinuteEntry>` (60分ウィンドウ) でインメモリ管理
- サーバー再起動で全スロットがゼロリセット
- `request_history`テーブルに個別リクエストレコードは永続化済み

### 解決アプローチ

- 起動時に`request_history`から直近60分のデータを分単位で集計
- 集計結果をVecDequeの対応スロットに投入

### データソース

```sql
SELECT
    strftime('%Y-%m-%d %H:%M:00', created_at, 'localtime') AS minute_bucket,
    SUM(CASE WHEN status_code >= 200 AND status_code < 400 THEN 1 ELSE 0 END) AS success,
    SUM(CASE WHEN status_code >= 400 OR status_code IS NULL THEN 1 ELSE 0 END) AS failure
FROM request_history
WHERE created_at >= datetime(?, 'utc')
GROUP BY minute_bucket
ORDER BY minute_bucket ASC
```

## 共通パターン

### グレースフルデグラデーション

全seed処理は以下のパターンで実装:

```rust
match seed_function(&db_pool).await {
    Ok(data) => { /* seed処理 */ }
    Err(e) => { warn!("Failed to seed: {}", e); }
}
```

DB障害時もサーバーは正常起動し、メトリクスはゼロ/未計測状態から開始。

### 既存EndpointRegistryパターン

`EndpointRegistry`は`Arc<RwLock<HashMap>>`でキャッシュを管理し、
DB操作後にキャッシュを同期する既存パターンを持つ。
Bug 1/3の修正はこのパターンに従う。
