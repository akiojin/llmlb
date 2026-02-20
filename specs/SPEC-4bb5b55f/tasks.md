# タスク: エンドポイント×モデル単位TPS可視化

**入力**: `specs/SPEC-4bb5b55f/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1, US2, US3)

## Phase 1: 基盤（全ストーリー共通のインフラ）

**目的**: TPS計測に必要なデータ基盤（型定義・DB・インメモリ状態）を構築

### Phase 1 テスト (RED)

- [x] T001 [P] [共通] `llmlb/src/types/endpoint.rs` に `EndpointType::is_tps_trackable()` のunit testを追加。xLLM/Ollama/vLLM/LmStudio/OpenaiCompatibleはtrueを返すことを検証
- [x] T002 [P] [共通] `llmlb/src/balancer/mod.rs` に `ModelTpsState` のunit testを追加。EMA計算（α=0.2）が正しく動作すること、初期値None→初回計測でSome値になること、複数回更新で平滑化されることを検証
- [x] T003 [P] [共通] `llmlb/src/db/endpoint_daily_stats.rs` に `upsert_daily_stats` 拡張版のunit testを追加。output_tokensとduration_msが累積加算されること、既存のリクエストカウント動作に影響しないことを検証

### Phase 1 実装 (GREEN)

- [x] T004 [P] [共通] `llmlb/migrations/016_add_tps_columns.sql` を新規作成。`endpoint_daily_stats` テーブルに `total_output_tokens INTEGER NOT NULL DEFAULT 0` と `total_duration_ms INTEGER NOT NULL DEFAULT 0` カラムを追加する ALTER TABLE 文を記述
- [x] T005 [P] [共通] `llmlb/src/types/endpoint.rs` に `EndpointType::is_tps_trackable()` メソッドを実装。全エンドポイントタイプで `true` を返す
- [x] T006 [共通] `llmlb/src/balancer/mod.rs` に `ModelTpsState` 構造体と `TpsTracker` を追加。`ModelTpsState` は `tps_ema: Option<f64>`, `request_count: u64`, `total_output_tokens: u64`, `total_duration_ms: u64` を保持。`LoadManager` に `tps_tracker: Arc<RwLock<HashMap<(Uuid, String), ModelTpsState>>>` フィールドを追加し、`update_tps()` と `get_model_tps(endpoint_id)` メソッドを実装。EMA計算は `α=0.2`、TPS = `output_tokens / duration_seconds`
- [x] T007 [共通] `llmlb/src/db/endpoint_daily_stats.rs` の `upsert_daily_stats()` シグネチャを拡張。引数に `output_tokens: u64` と `duration_ms: u64` を追加し、UPSERT文で `total_output_tokens = total_output_tokens + excluded.total_output_tokens`、`total_duration_ms = total_duration_ms + excluded.total_duration_ms` として累積加算する。既存の呼び出し箇所も更新（デフォルト値0で呼び出し）

**チェックポイント**: T001-T003のテストがT004-T007の実装で全てパスすること

---

## Phase 2: US-2 TPS履歴トレンド分析（DB永続化）

**目標**: リクエスト完了時にトークン数・処理時間をDBに永続化し、日次TPSを算出可能にする

**独立テスト**: `cargo test` でDB操作のunit testがパスし、`upsert_daily_stats` に渡されたトークン・時間が正しく累積されることを確認

### US-2 テスト (RED)

- [x] T008 [US2] `llmlb/src/db/endpoint_daily_stats.rs` に日次TPS算出クエリのunit testを追加。`get_model_stats` 拡張版が `total_output_tokens` と `total_duration_ms` を含むレスポンスを返し、日次平均TPS（`total_output_tokens / (total_duration_ms / 1000)`）が計算可能であることを検証

### US-2 実装 (GREEN)

- [x] T009 [US2] `llmlb/src/db/endpoint_daily_stats.rs` の `ModelStatEntry` 構造体に `total_output_tokens: i64` と `total_duration_ms: i64` フィールドを追加。`get_model_stats()` と `get_all_model_stats()` のSELECT文に `SUM(total_output_tokens)` と `SUM(total_duration_ms)` を追加。`ModelStatRow` も同様に拡張
- [x] T010 [US2] `llmlb/src/api/proxy.rs` の `record_endpoint_request_stats()` 関数シグネチャに `output_tokens: u64`、`duration_ms: u64`、`endpoint_type: crate::types::endpoint::EndpointType` を追加。`endpoint_type.is_tps_trackable()` が true の場合のみ `upsert_daily_stats` にトークン・時間を渡す。false の場合はトークン・時間に0を渡す
- [x] T011 [US2] `llmlb/src/api/openai.rs` の全 `record_endpoint_request_stats()` 呼び出し箇所を更新。非ストリーミング成功時は `token_usage` から `output_tokens` を抽出し `duration.as_millis()` と共に渡す。ストリーミング・エラー時は `0, 0` を渡す。`endpoint_type` はエンドポイント情報から取得して渡す

**チェックポイント**: T008のテストがパスし、DBにトークン・時間データが永続化されること

---

## Phase 3: US-3 REST API経由のTPS取得

**目標**: REST APIからエンドポイント×モデルのTPS情報を取得可能にする

**独立テスト**: `curl` でAPIエンドポイントを叩き、JSON レスポンスに `model_id`, `tps`, `request_count`, `total_output_tokens`, `average_duration_ms` が含まれることを確認

### US-3 テスト (RED)

- [x] T012 [P] [US3] `llmlb/src/api/dashboard.rs` に `ModelTpsEntry` 構造体と `get_endpoint_model_tps` ハンドラーのunit testを追加。LoadManagerの `get_model_tps()` からTPS情報を取得し、正しいJSON構造で返却されることを検証
- [x] T013 [P] [US3] `llmlb/src/balancer/mod.rs` に `get_model_tps()` メソッドのunit testを追加。TPS未計測のエンドポイントでは空Vecを返し、計測済みの場合は `ModelTpsEntry` のリストを返すことを検証

### US-3 実装 (GREEN)

- [x] T014 [US3] `llmlb/src/api/dashboard.rs` に `ModelTpsEntry` 構造体（`model_id: String`, `tps: Option<f64>`, `request_count: u64`, `total_output_tokens: u64`, `average_duration_ms: Option<f64>`）と `get_endpoint_model_tps` ハンドラー（`GET /api/endpoints/{id}/model-tps`）を実装。`LoadManager::get_model_tps(endpoint_id)` からインメモリTPS値を取得して返却
- [x] T015 [US3] `llmlb/src/api/mod.rs` のルーティングに `GET /api/endpoints/{id}/model-tps` を `endpoint_read_routes` に追加（`EndpointsRead` パーミッション）
- [x] T016 [US3] `llmlb/src/api/proxy.rs` の `record_endpoint_request_stats()` 内で `LoadManager::update_tps()` を呼び出し、リクエスト完了時にインメモリTPS EMAを更新する。`AppState` の `load_manager` を引数に追加するか、`tokio::spawn` 内で利用可能にする

**チェックポイント**: `GET /api/endpoints/{id}/model-tps` が正しいJSONレスポンスを返すこと

---

## Phase 4: US-1 リアルタイムTPS確認（WebSocket + ダッシュボード）

**目標**: ダッシュボードでモデル別TPSテーブルをリアルタイム表示

**独立テスト**: ダッシュボードのエンドポイント詳細パネルにTPSテーブルが表示され、推論リクエスト完了後にリアルタイム更新されることを確認

### US-1 テスト (RED)

- [x] T017 [P] [US1] `llmlb/src/events/mod.rs` に `DashboardEvent::TpsUpdated` バリアントのシリアライゼーションtestを追加。JSON出力に `type: "TpsUpdated"` と `endpoint_id`, `model_id`, `tps`, `output_tokens`, `duration_ms` が含まれることを検証

### US-1 実装 (GREEN)

- [x] T018 [US1] `llmlb/src/events/mod.rs` の `DashboardEvent` enumに `TpsUpdated` バリアントを追加。フィールド: `endpoint_id: Uuid`, `model_id: String`, `tps: f64`, `output_tokens: u32`, `duration_ms: u64`
- [x] T019 [US1] `llmlb/src/api/proxy.rs` の `record_endpoint_request_stats()` 内で、TPS更新後に `event_bus.publish(DashboardEvent::TpsUpdated { ... })` を呼び出す。`event_bus: SharedEventBus` を引数に追加
- [x] T020 [US1] `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` のエンドポイント詳細パネルにモデルTPSテーブルを追加。テーブル列: モデル名 | TPS | リクエスト数 | 累計出力トークン | 平均処理時間。TPS値は小数点1位 + "tok/s" で表示（例: "42.5 tok/s"）、未計測は "—" 表示。初期データは `/api/endpoints/{id}/model-tps` から取得、WebSocket `TpsUpdated` イベントでリアルタイム更新
- [x] T021 [US1] ダッシュボードビルド実行: `pnpm --filter @llm/dashboard build` を実行し、`llmlb/src/web/static/` の生成物をコミット対象に含める

**チェックポイント**: ダッシュボードでTPSテーブルが表示され、リアルタイム更新されること

---

## Phase 5: 仕上げ＆横断的関心事

**目的**: 品質保証、統合テスト、ドキュメント

- [x] T022 [共通] `llmlb/src/api/openai.rs` の全ストリーミングハンドラー（`chat_completions`, `completions`, `embeddings`）で `record_endpoint_request_stats()` 呼び出しが拡張後のシグネチャに対応していることを確認・修正
- [x] T023 [共通] `GET /api/dashboard/overview` レスポンスの `DashboardOverview` にTPS概要情報を含める。エンドポイントごとの集約TPSまたはモデル別TPS上位をstats内に追加
- [x] T024 [共通] `cargo fmt --check && cargo clippy -- -D warnings && cargo test` を実行し全テストパス・lint警告ゼロを確認
- [x] T025 [共通] `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"` を実行しmarkdownlint警告ゼロを確認
- [x] T026 [共通] `make quality-checks` を実行し全品質チェックパスを確認
- [x] T027 [共通] quickstart.md検証: `curl` で `/api/endpoints/{id}/model-tps` を叩き、期待通りのJSONが返ることを確認

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (基盤)**: 依存関係なし。T001-T003は並列でRED、T004-T007でGREEN
- **Phase 2 (US-2)**: Phase 1完了に依存。DB永続化フロー確立
- **Phase 3 (US-3)**: Phase 1完了に依存。Phase 2と並列可能だが、T016がT006に依存
- **Phase 4 (US-1)**: Phase 3完了に依存（REST APIが必要）。T018はPhase 1のみに依存し先行可能
- **Phase 5 (仕上げ)**: Phase 1-4完了に依存

### 並列実行可能なタスク

```text
Phase 1: T001 || T002 || T003 （テスト）、T004 || T005 （実装の一部）
Phase 3: T012 || T013 （テスト）
Phase 4: T017 （テスト、Phase 1のみに依存）
```

### クリティカルパス

```text
T001-T003 → T004-T007 → T010-T011 → T016 → T019 → T020 → T021 → T024-T026
```

## 実装戦略

### TDDサイクル

各Phaseで以下を厳守:

1. テストタスク（RED）を先にコミット
2. 実装タスク（GREEN）でテストをパスさせる
3. リファクタリング後に品質チェック

### MVPファースト

Phase 1 + Phase 2 でDB永続化が完了すれば、日次TPS算出は可能。
Phase 3 で REST API が使えるようになり、外部ツール連携が可能。
Phase 4 でダッシュボードUIが完成し、運用者向けの可視化が完了。
