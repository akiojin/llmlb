# 技術リサーチ: エンドポイント×モデル単位TPS可視化

**機能ID**: `SPEC-4bb5b55f` | **日付**: 2026-02-19

## 調査対象

### 1. 既存のトークン追跡アーキテクチャ

**現状**: トークン追跡はエンドポイント単位で実装済み。

- `EndpointLoadState` (`balancer/mod.rs`):
  `total_input_tokens`, `total_output_tokens`, `total_tokens` をエンドポイント単位で保持
- `finish_request_with_tokens()`: リクエスト完了時にトークンを累積
- `TokenUsage` (`token/mod.rs`):
  `input_tokens`, `output_tokens`, `total_tokens` を保持する構造体
- `StreamingTokenAccumulator`: SSEストリーミングチャンクからトークンを累積

**ギャップ**: モデル単位のトークン追跡は存在しない。
EndpointLoadStateはモデルを区別せずにトークンを累積している。

### 2. 日次統計の永続化

**現状**: `endpoint_daily_stats` テーブルが存在。

- PK: `(endpoint_id, model_id, date)`
- カラム: `total_requests`, `successful_requests`, `failed_requests`
- `upsert_daily_stats()`: リクエスト完了時にUPSERT
- `record_endpoint_request_stats()` (`proxy.rs`):
  fire-and-forget で日次統計を更新

**ギャップ**: トークン数・処理時間のカラムが存在しない。
TPS計算に必要な `total_output_tokens`、`total_duration_ms` が欠落。

### 3. リクエスト完了フロー（TPS注入ポイント）

**非ストリーミング** (`openai.rs` L1519-1528):

1. `extract_usage_from_response(&body)` でトークン抽出
2. `request_lease.complete_with_tokens()` でLoadManager更新
3. `update_inference_latency()` でEMAレイテンシ更新
4. `record_endpoint_request_stats()` でDB統計更新

**ストリーミング** (`openai.rs` L1423-1460):

1. ストリーミングレスポンスをクライアントに転送
2. `record_endpoint_request_stats()` でDB統計更新
3. トークン累積は `StreamingTokenAccumulator` で実施（ただし現在の
   ストリーミングフローでは `complete_with_tokens` が呼ばれない箇所あり）

**結論**: `record_endpoint_request_stats()` が最適な注入ポイント。
endpoint_id、model_id、durationが全て利用可能な地点。

### 4. EMA実装パターン

**既存**: レイテンシEMA (`types/endpoint.rs`):

```text
update_inference_latency(): α=0.2 の EMA
new_ema = α × current + (1 - α) × previous
```

TPS EMAも同一パターンで実装可能。O(1)計算、追加メモリはキーごとにf64 1つ。

### 5. WebSocket/イベントバスアーキテクチャ

- `DashboardEventBus` (`events/mod.rs`):
  `broadcast::channel` で全WebSocketクライアントにイベント配信
- `DashboardEvent` enum: `NodeRegistered`, `EndpointStatusChanged`,
  `MetricsUpdated`, `NodeRemoved`
- `dashboard_ws_handler`: 認証付きWebSocket接続、イベント購読

**拡張方針**: `DashboardEvent` に `TpsUpdated` バリアントを追加。

### 6. エンドポイントタイプフィルタリング

`EndpointType` enum (`types/endpoint.rs`):
`Xllm`, `Ollama`, `Vllm`, `LmStudio`, `OpenaiCompatible`

FR-2に従い、`OpenaiCompatible` を除外するフィルタが必要。
エンドポイント情報は `EndpointRegistry` から取得可能。

## 技術的決定事項

| 項目 | 決定 | 根拠 |
|------|------|------|
| TPS保持場所 | LoadManager内の新HashMap | 既存パターンと整合 |
| EMAパラメータ | α=0.2 | レイテンシEMAと同一 |
| DB永続化 | ALTER TABLE追加 | 新テーブル不要 |
| WS通知 | 新イベントバリアント | 既存パターンに追従 |
| APIエンドポイント | `/api/endpoints/{id}/model-tps` | 既存パスと整合 |
