# タスク: エンドポイント単位リクエスト統計

**入力**: `specs/SPEC-8c32349f/` の設計ドキュメント
**前提条件**: plan.md, spec.md, data-model.md, research.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1〜US5)

## Phase 1: 基盤 (データ永続化基盤)

**目的**: 全ユーザーストーリーが依存するDBスキーマと型定義を構築

- [ ] T001 [US2] `llmlb/migrations/014_add_endpoint_request_stats.sql` を作成。
  endpointsテーブルに `total_requests`, `successful_requests`, `failed_requests`
  (INTEGER NOT NULL DEFAULT 0) を追加。
  `endpoint_daily_stats` テーブルを新規作成
  (PK: endpoint_id + model_id + date, FK制約なし)。
  インデックス `idx_daily_stats_endpoint_date`, `idx_daily_stats_date` を作成。
  data-model.md のSQL定義に従う。

- [ ] T002 [US2] `llmlb/src/types/endpoint.rs` の `Endpoint` 構造体に
  `total_requests: i64`, `successful_requests: i64`, `failed_requests: i64`
  フィールドを追加。Serialize/Deserialize対応。デフォルト値0。

- [ ] T003 [P] [US2] `llmlb/src/types/endpoint.rs` に `EndpointDailyStats`
  構造体を新規定義。フィールド: `endpoint_id: Uuid`, `model_id: String`,
  `date: String` (YYYY-MM-DD), `total_requests: i64`,
  `successful_requests: i64`, `failed_requests: i64`。
  Serialize/Deserialize derive付与。

---

## Phase 2: DB操作レイヤー

**目的**: カウンタ更新と日次集計のCRUD操作を実装

### テスト (RED)

- [ ] T004 [US2] `llmlb/src/db/endpoints.rs` のテストモジュールに
  `increment_request_counters` のユニットテストを作成。
  テスト内容: (1) 成功リクエストでtotal_requests+1, successful_requests+1,
  (2) 失敗リクエストでtotal_requests+1, failed_requests+1,
  (3) 複数回呼び出しでカウンタが正しく累積。
  テストが失敗することを確認。

- [ ] T005 [P] [US2] `llmlb/src/db/endpoint_daily_stats.rs` のテストモジュールに
  以下のユニットテストを作成:
  (1) `upsert_daily_stats` - 新規レコード挿入と既存レコードのカウンタ加算,
  (2) `get_daily_stats` - 期間指定での取得と空結果,
  (3) `get_model_stats` - エンドポイント別モデル集計。
  テストが失敗することを確認。

### 実装 (GREEN)

- [ ] T006 [US2] `llmlb/src/db/endpoints.rs` に `increment_request_counters`
  関数を実装。引数: `pool: &SqlitePool, id: Uuid, success: bool`。
  SQLは `UPDATE endpoints SET total_requests = total_requests + 1,
  successful_requests = successful_requests + CASE WHEN ? THEN 1 ELSE 0 END,
  failed_requests = failed_requests + CASE WHEN ? THEN 0 ELSE 1 END
  WHERE id = ?`。T004のテストがパスすることを確認。

- [ ] T007 [US2] `llmlb/src/db/endpoint_daily_stats.rs` を新規作成。
  `llmlb/src/db/mod.rs` にモジュール登録。以下の関数を実装:
  (1) `upsert_daily_stats(pool, endpoint_id, model_id, date, success)` -
  INSERT OR UPDATE でカウンタをインクリメント,
  (2) `get_daily_stats(pool, endpoint_id, days)` -
  指定日数分の日次データを取得,
  (3) `get_model_stats(pool, endpoint_id)` -
  モデル別の累計集計を取得。
  T005のテストがパスすることを確認。

- [ ] T008 [US2] `llmlb/src/db/endpoints.rs` の `create_endpoint` 関数を更新。
  INSERT文に `total_requests`, `successful_requests`, `failed_requests` カラムを
  追加（デフォルト0）。`list_endpoints` と `get_endpoint` のSELECT文にも
  3カラムを追加。既存テストが引き続きパスすることを確認。

---

## Phase 3: ユーザーストーリー2 - リクエスト数の永続的な記録 (優先度: P1)

**目標**: リクエスト完了時にカウンタと日次集計をリアルタイム更新。日次バッチで確定。

**独立テスト**: リクエスト送信後にカウンタが増加し、クリーンアップ後も維持されることを確認

### テスト (RED)

- [ ] T009 [US2] `llmlb/src/balancer/` のテストモジュールに
  `finish_request` がカウンタ更新を呼び出すことを検証するテストを作成。
  モック/テスト用DBで `finish_request` 呼び出し後に
  endpointsテーブルのカウンタが増加していることを確認。
  テストが失敗することを確認。

### 実装 (GREEN)

- [ ] T010 [US2] `llmlb/src/balancer/mod.rs` の `finish_request()` (L821-879) と
  `finish_request_with_tokens()` (L882-965) を変更。
  処理末尾で `tokio::spawn` を使い、非同期で
  `db::endpoints::increment_request_counters()` と
  `db::endpoint_daily_stats::upsert_daily_stats()` を呼び出す。
  AppState経由でdb_poolを取得。model_idはリクエストコンテキストから取得。
  日付はサーバーローカル時間の `Local::now().format("%Y-%m-%d")` を使用。
  T009のテストがパスすることを確認。

- [ ] T011 [US2] `llmlb/src/main.rs` に日次バッチタスクの起動を追加。
  `start_cleanup_task` (request_history.rs:886-922) と同じパターンで
  `tokio::spawn` を使用。サーバーローカル時間0:00まで待機
  (`chrono::Local::now()` で次の0:00までのDurationを計算)後、
  `tokio::time::interval(Duration::from_secs(86400))` で24時間周期実行。
  バッチ処理: request_historyテーブルから前日分をendpoint_id×model_id×dateで
  集計し、endpoint_daily_statsの値と比較・補正。

**チェックポイント**: リクエスト送信でカウンタが増加し、
リクエスト履歴クリーンアップ後もカウンタが維持される

---

## Phase 4: ユーザーストーリー1 - エンドポイント一覧でリクエスト数を確認する (優先度: P1)

**目標**: エンドポイント一覧テーブルにRequestsカラムを追加し、リクエスト数と成功率を表示

**独立テスト**: 一覧テーブルにRequestsカラムが存在し正しい値が表示される

### テスト (RED)

- [ ] T012 [US1] `llmlb/src/api/dashboard.rs` のテストモジュールに
  `DashboardEndpoint` に `total_requests`, `successful_requests`, `failed_requests`
  フィールドが含まれることを検証するAPIテストを作成。
  `/api/dashboard/endpoints` レスポンスにカウンタが含まれることを確認。
  テストが失敗することを確認。

### 実装 (GREEN)

- [ ] T013 [US1] `llmlb/src/api/dashboard.rs` の `DashboardEndpoint` 構造体
  (L36-71) に `total_requests: u64`, `successful_requests: u64`,
  `failed_requests: u64` フィールドを追加。
  `collect_endpoints()` (L279-315) でendpointsテーブルからカウンタ値を
  取得してDashboardEndpointに設定。T012のテストがパスすることを確認。

- [ ] T014 [P] [US1] `llmlb/src/web/dashboard/src/lib/api.ts` の
  `DashboardEndpoint` interface (L187-205) に
  `total_requests: number`, `successful_requests: number`,
  `failed_requests: number` を追加。

- [ ] T015 [US1] `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx`
  にRequestsカラムを追加。表示形式: リクエスト数0件の場合は「0 (-)」、
  1件以上の場合は「N (XX.X%)」（Nはtotal_requestsのカンマ区切り、
  XX.X%はsuccessful_requests/total_requests×100）。
  エラー率（1-成功率）≥5%で `text-yellow-600`、≥20%で `text-red-600` の
  Tailwindクラス適用。カラムはtotal_requestsでソート可能
  （既存のソートロジック `handleSort` に追加）。

**チェックポイント**: エンドポイント一覧テーブルにRequestsカラムが表示され、
エラー率に応じた色分けが機能する

---

## Phase 5: ユーザーストーリー3 - エンドポイント詳細で統計カードを確認する (優先度: P2)

**目標**: 詳細モーダルに4枚の数値カード（累計・今日・成功率・平均レスポンス）を表示

**独立テスト**: 詳細モーダルを開き4枚のカードに正しい値が表示される

### テスト (RED)

- [ ] T016 [US3] `llmlb/src/api/dashboard.rs` のテストモジュールに
  `GET /api/dashboard/endpoints/:id/stats/today` のAPIテストを作成。
  当日のリクエスト数を返すことを確認。テストが失敗することを確認。

### 実装 (GREEN)

- [ ] T017 [US3] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/endpoints/:id/stats/today` ハンドラを追加。
  endpoint_daily_statsテーブルから当日分の集計を取得し、
  `{total_requests, successful_requests, failed_requests}` を返す。
  ルーターにルート追加。T016のテストがパスすることを確認。

- [ ] T018 [US3] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `getEndpointTodayStats(endpointId: string)` 関数を追加。
  `EndpointTodayStats` 型定義も追加
  (`total_requests`, `successful_requests`, `failed_requests`)。

- [ ] T019 [US3]
  `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx`
  の既存Info Gridの上にリクエスト統計カードセクションを追加。
  4枚のカード: (1) 累計リクエスト（DashboardEndpoint.total_requests）、
  (2) 今日のリクエスト（/stats/today APIから取得、React Query使用）、
  (3) 成功率（successful/total×100、≥5%エラーで黄、≥20%エラーで赤）、
  (4) 平均レスポンス時間（既存のlatency_msを使用）。
  リクエスト0件時は「-」を表示。

**チェックポイント**: 詳細モーダルに4枚の統計カードが正しい値で表示される

---

## Phase 6: ユーザーストーリー4 - 日次トレンドチャート (優先度: P2)

**目標**: 詳細モーダルに成功/失敗の日次積み上げ棒グラフを表示

**独立テスト**: チャートが正しいデータで描画され、7/30/90日の切替が機能する

### テスト (RED)

- [x] T020 [US4] `llmlb/tests/contract/endpoint_daily_stats_api_test.rs` に
  `GET /api/endpoints/:id/daily-stats?days=7` のAPIテストを作成。
  期間指定で日次集計データの配列を返すことを確認。4テストケース全パス。

### 実装 (GREEN)

- [x] T021 [US4] `llmlb/src/api/dashboard.rs` に
  `GET /api/endpoints/:id/daily-stats` ハンドラを追加。
  クエリパラメータ `days` (デフォルト7、最大365) を受け取り、
  `db::endpoint_daily_stats::get_daily_stats()` を呼び出して結果を返す。
  レスポンス型: `Vec<DailyStatEntry>`。
  DailyStatEntry: `{ date, total_requests, successful_requests, failed_requests }`。
  ルーターにルート追加済み。T020のテスト全パス。

- [x] T022 [P] [US4] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `getDailyStats(id: string, days?: number)` 関数を追加。
  `EndpointDailyStatEntry` 型定義も追加済み。

- [x] T023 [US4]
  `llmlb/src/web/dashboard/src/components/dashboard/EndpointRequestChart.tsx`
  を新規作成。Recharts の `BarChart` + `Bar` (stacked) を使用した
  積み上げ棒グラフコンポーネント。
  Props: `endpointId: string`。
  内部でReact Query (`useQuery`) を使い `/daily-stats` APIからデータ取得。
  7/30/90日のタブ切替（Radix UI Tabs使用、デフォルト7日）。
  成功バー: `fill="#22c55e"` (green-500)、
  失敗バー: `fill="#ef4444"` (red-500)。
  X軸: 日付（MM/DD形式）、Y軸: リクエスト数。
  データなし時: "No request data available" のエンプティステート表示。

- [x] T024 [US4]
  `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx`
  に `EndpointRequestChart` コンポーネントを統合。
  統計カードセクションの下、Info Sectionの上に配置。

**チェックポイント**: 日次チャートが正しく描画され、期間切替が1秒以内に完了

---

## Phase 7: ユーザーストーリー5 - モデル別リクエスト内訳 (優先度: P3)

**目標**: 詳細モーダルにモデル別リクエスト数の内訳テーブルを表示

**独立テスト**: モデル別テーブルに各モデルのリクエスト数と成功/失敗内訳が表示される

### テスト (RED)

- [ ] T025 [US5] `llmlb/src/api/dashboard.rs` のテストモジュールに
  `GET /api/dashboard/endpoints/:id/stats/models` のAPIテストを作成。
  モデル別集計データを返すことを確認。テストが失敗することを確認。

### 実装 (GREEN)

- [ ] T026 [US5] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/endpoints/:id/stats/models` ハンドラを追加。
  `db::endpoint_daily_stats::get_model_stats()` を呼び出して結果を返す。
  レスポンス型: `EndpointModelStatsResponse { endpoint_id, models: Vec<ModelStatEntry> }`。
  ModelStatEntry: `{ model_id, total_requests, successful_requests, failed_requests }`。
  ルーターにルート追加。T025のテストがパスすることを確認。

- [ ] T027 [P] [US5] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `getEndpointModelStats(endpointId: string)` 関数を追加。
  `EndpointModelStatsResponse` と `ModelStatEntry` 型定義も追加。

- [ ] T028 [US5]
  `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx`
  にモデル別リクエスト統計テーブルセクションを追加。
  チャートセクションの下に配置。
  React Query で `/stats/models` APIからデータ取得。
  テーブルカラム: モデル名、合計、成功、失敗、成功率。
  成功率の2段階カラーリング（≥5%エラーで黄、≥20%エラーで赤）。
  データなし時: 「リクエスト履歴がありません」表示。

**チェックポイント**: モデル別テーブルにリクエスト内訳が正しく表示される

---

## Phase 8: 仕上げ＆横断的関心事

**目的**: 品質確認とドキュメント整備

- [ ] T029 [P] `cargo fmt --check` でフォーマット確認、
  `cargo clippy -- -D warnings` でリントパス確認
- [ ] T030 [P] `cargo test` で全テスト（既存＋新規）がパスすることを確認
- [ ] T031 [P] `pnpm dlx markdownlint-cli2` でマークダウンlintパス確認
- [ ] T032 quickstart.md の検証シナリオを手動実行:
  エンドポイント登録→リクエスト送信→一覧確認→詳細確認
- [ ] T033 `make quality-checks` で全品質チェックをパス

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (基盤)**: 依存なし - 即開始可能
- **Phase 2 (DB操作)**: Phase 1 完了に依存
- **Phase 3 (US2: 永続化)**: Phase 2 完了に依存
- **Phase 4 (US1: 一覧表示)**: Phase 3 完了に依存（カウンタが更新される前提）
- **Phase 5 (US3: 統計カード)**: Phase 3 完了に依存
- **Phase 6 (US4: チャート)**: Phase 3 完了に依存
- **Phase 7 (US5: モデル別)**: Phase 3 完了に依存
- **Phase 8 (仕上げ)**: 全フェーズ完了に依存

### 並列実行可能なフェーズ

Phase 3完了後、以下は並列実行可能:

- Phase 4 (US1: 一覧表示)
- Phase 5 (US3: 統計カード)
- Phase 6 (US4: チャート)
- Phase 7 (US5: モデル別)

### 各Phase内の依存関係

- テスト(RED) → 実装(GREEN) の順序は厳守
- [P]マークのタスクは並列実行可能
- 実装タスクはテストタスクの後に実行

### 推奨実行順序（直列の場合）

```text
T001 → T002 → T003 → T004/T005(並列) → T006 → T007 → T008
→ T009 → T010 → T011
→ T012 → T013 → T014 → T015
→ T016 → T017 → T018 → T019
→ T020 → T021 → T022 → T023 → T024
→ T025 → T026 → T027 → T028
→ T029/T030/T031(並列) → T032 → T033
```
