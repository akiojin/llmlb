# タスク: ダッシュボードメトリクスの永続化と復元

**入力**: `/specs/SPEC-ba72f693/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1-US5)

## Phase 1: ユーザーストーリー2 - オフラインLatency保持 (優先度: P1)

**目標**: ヘルスチェック失敗でオフラインに遷移したエンドポイントの
レイテンシ値が消失せず、最後に計測された値が保持される

**独立テスト**: オンラインエンドポイントをオフラインに遷移させ、
ダッシュボードAPIでレイテンシ値が保持されていることを確認

### テスト (RED)

- [x] T001 [US2] `llmlb/src/db/endpoints.rs` の `#[cfg(test)] mod tests` に
  `update_endpoint_status` で `latency_ms=None` を渡した場合に
  既存のレイテンシ値が保持されることを検証するユニットテストを追加。
  テストシナリオ: (1) エンドポイント登録 → (2) latency_ms=Some(120.0) で
  ステータス更新 → (3) latency_ms=None でステータス更新 →
  (4) DBから読み取りlatency_msが120.0のまま保持されていることを確認
- [x] T002 [P] [US2] `llmlb/src/registry/endpoints.rs` の `#[cfg(test)] mod tests` に
  `update_status` で `latency_ms=None` を渡した場合に
  キャッシュ内のレイテンシ値が保持されることを検証するユニットテストを追加。
  テストシナリオ: (1) キャッシュにlatency_ms=Some(120.0)のエンドポイント登録 →
  (2) update_status(latency_ms=None) 呼び出し →
  (3) キャッシュからlatency_msがSome(120.0)のまま保持されていることを確認

### 実装 (GREEN)

- [x] T003 [US2] `llmlb/src/db/endpoints.rs` の `update_endpoint_status` 関数内の
  SQL文で `latency_ms = ?` を `latency_ms = COALESCE(?, latency_ms)` に変更。
  None渡し時にDB上の既存レイテンシ値が保持されるようにする
- [x] T004 [US2] `llmlb/src/registry/endpoints.rs` の `update_status` メソッド内で
  `endpoint.latency_ms = latency_ms` を
  `if let Some(v) = latency_ms { endpoint.latency_ms = Some(v); }` に変更。
  None渡し時にキャッシュの既存レイテンシ値が保持されるようにする

**チェックポイント**: T001, T002のテストが通過すること

---

## Phase 2: ユーザーストーリー1 - リクエストカウンタのリアルタイム反映 (優先度: P1)

**目標**: リクエスト処理完了時にDBとキャッシュのカウンタが同時に更新され、
ダッシュボードAPIで即座に反映される

**独立テスト**: エンドポイントにリクエスト送信後、
ダッシュボードAPIのリクエスト数が即座に増加していることを確認

### テスト (RED)

- [x] T005 [US1] `llmlb/src/registry/endpoints.rs` の `#[cfg(test)] mod tests` に
  `increment_request_counters` メソッドのユニットテストを追加。
  テストシナリオ: (1) エンドポイント登録(カウンタ=0) →
  (2) increment_request_counters(success=true) 呼び出し →
  (3) キャッシュのtotal_requests=1, successful_requests=1を確認。
  (4) increment_request_counters(success=false) 呼び出し →
  (5) キャッシュのtotal_requests=2, failed_requests=1を確認

### 実装 (GREEN)

- [x] T006 [US1] `llmlb/src/registry/endpoints.rs` に
  `increment_request_counters(&self, endpoint_id: Uuid, success: bool)`
  メソッドを追加。DB (`db::increment_request_counters`) を呼び出した後、
  キャッシュ内の `Endpoint` の `total_requests` / `successful_requests` /
  `failed_requests` をインクリメントする
- [x] T007 [US1] `llmlb/src/api/proxy.rs` の `record_endpoint_request_stats` 関数と
  `TpsTrackingState` 構造体の `pool: SqlitePool` フィールドを
  `endpoint_registry: EndpointRegistry` に変更。内部で
  `endpoint_registry.increment_request_counters()` を呼び出す。
  TPS/daily_stats用のpoolは `endpoint_registry.pool()` から取得
- [x] T008 [US1] `llmlb/src/api/openai.rs` の `record_endpoint_request_stats`
  呼び出し元5箇所で、引数を `state.db_pool.clone()` から
  `state.endpoint_registry.clone()` に変更
- [x] T009 [US1] `llmlb/src/api/responses.rs` の `record_endpoint_request_stats`
  呼び出し元4箇所で、引数を `state.db_pool.clone()` から
  `state.endpoint_registry.clone()` に変更

**チェックポイント**: T005のテストが通過し、
`cargo test` で全テストが通過すること

---

## Phase 3: ユーザーストーリー3 - 平均レスポンス時間の再起動後復元 (優先度: P1)

**目標**: サーバー再起動後もダッシュボード概要の平均レスポンス時間に
有意な値が表示される

**独立テスト**: オンラインエンドポイントのレイテンシが計測済みの状態で
collect_stats を呼び出し、average_response_time_ms が None ではないことを確認

### テスト (RED)

- [x] T010 [US3] `llmlb/src/api/dashboard.rs` の `#[cfg(test)] mod tests` に
  `collect_stats` のフォールバック計算テストを追加。
  テストシナリオ: (1) LoadManagerのインメモリavg_response_time_msがNone →
  (2) オンラインエンドポイント2つ (latency_ms=100.0, 200.0) を渡す →
  (3) 結果のaverage_response_time_msが150.0（平均値）であることを確認。
  追加シナリオ: 全エンドポイントがオフラインの場合はNoneのまま

### 実装 (GREEN)

- [x] T011 [US3] `llmlb/src/api/dashboard.rs` の `collect_stats` 関数内で
  `summary.average_response_time_ms` が `None` の場合に、
  引数として受け取った endpoints リストからオンライン（status="online"）の
  エンドポイントの `latency_ms` を収集し、平均値を計算して設定する
  フォールバックロジックを追加。全エンドポイントがオフラインまたは
  latency_ms が None の場合は None のまま維持

**チェックポイント**: T010のテストが通過すること

---

## Phase 4: ユーザーストーリー5 - リクエスト履歴タイムラインの再起動後復元 (優先度: P2)

**目標**: サーバー再起動後もダッシュボードのリクエスト履歴タイムラインに
再起動前のデータが表示される

**独立テスト**: request_historyテーブルにデータを挿入した状態で
seed_history_from_db を呼び出し、VecDeque内に復元されることを確認

### テスト (RED)

- [x] T012 [US5] `llmlb/src/db/request_history.rs` の `#[cfg(test)] mod tests` に
  `get_recent_history_by_minute` クエリのユニットテストを追加。
  テストシナリオ: (1) request_historyテーブルに直近10分のレコードを挿入
  (成功3件+失敗1件) → (2) get_recent_history_by_minute呼び出し →
  (3) 返却された分単位データに正しいsuccess/failure件数が含まれることを確認
- [x] T013 [P] [US5] `llmlb/src/balancer/mod.rs` の `#[cfg(test)] mod tests` に
  `seed_history_from_db` メソッドのユニットテストを追加。
  テストシナリオ: (1) HistoryMinuteEntryのリストを作成 →
  (2) seed_history_from_db 呼び出し →
  (3) VecDeque内の対応するスロットに値が設定されていることを確認

### 実装 (GREEN)

- [x] T014 [US5] `llmlb/src/db/request_history.rs` に
  `get_recent_history_by_minute(pool, since)` クエリ関数を追加。
  request_historyテーブルから指定日時以降のレコードを分単位で集計し、
  minute_bucket / success / failure のリストを返却
- [x] T015 [US5] `llmlb/src/balancer/mod.rs` に
  `LoadManager::seed_history_from_db(&self, entries)` メソッドを追加。
  取得した分単位データをVecDeque内の対応スロットに投入
- [x] T016 [US5] `llmlb/src/bootstrap.rs` の LoadManager 初期化後に
  seed_history_from_db 呼び出しを追加。
  `get_recent_history_by_minute` で直近60分のデータを取得し、
  `load_manager.seed_history_from_db()` に渡す。
  失敗時は `warn!` ログのみで正常起動を継続（FR-006準拠）

**チェックポイント**: T012, T013のテストが通過し、
`cargo test` で全テストが通過すること

---

## Phase 5: ユーザーストーリー4 - TPSの再起動後復元 (優先度: P2)

**目標**: サーバー再起動後も当日処理実績のあるエンドポイントのTPS値が
復元される

**独立テスト**: endpoint_daily_statsに当日データがある状態で
seed_tps_from_db を呼び出し、TpsTrackerMap内に復元されることを確認

### テスト (RED)

- [x] T017 [US4] `llmlb/src/db/endpoint_daily_stats.rs` の `#[cfg(test)] mod tests` に
  `get_today_stats_all` クエリのユニットテストを追加。
  テストシナリオ: (1) endpoint_daily_statsに当日のレコードを挿入
  (total_output_tokens=100, total_duration_ms=2000) →
  (2) get_today_stats_all呼び出し →
  (3) 返却されたTpsSeedEntryの値が挿入データと一致することを確認
- [x] T018 [P] [US4] `llmlb/src/balancer/mod.rs` の `#[cfg(test)] mod tests` に
  `seed_tps_from_db` メソッドのユニットテストを追加。
  テストシナリオ: (1) TpsSeedEntry (tokens=100, duration_ms=2000) を作成 →
  (2) seed_tps_from_db 呼び出し →
  (3) TpsTrackerMap内のtps_emaが50.0 (100/2.0) であることを確認。
  追加: duration_ms=0またはtokens=0のエントリはスキップされること

### 実装 (GREEN)

- [x] T019 [US4] `llmlb/src/db/endpoint_daily_stats.rs` に
  `get_today_stats_all(pool, date)` クエリ関数と
  `TpsSeedEntry` / `TpsSeedRow` 構造体を追加。
  endpoint_daily_statsテーブルから指定日付のレコードを全取得
- [x] T020 [US4] `llmlb/src/balancer/mod.rs` に
  `LoadManager::seed_tps_from_db(&self, entries)` メソッドを追加。
  各TpsSeedEntryからTPS = total_output_tokens / (total_duration_ms / 1000.0) を計算し、
  TpsTrackerMap に `TpsApiKind::ChatCompletions` キーで設定。
  duration_ms <= 0 または tokens <= 0 のエントリはスキップ
- [x] T021 [US4] `llmlb/src/bootstrap.rs` の seed_history 呼び出し後に
  seed_tps_from_db 呼び出しを追加。
  `get_today_stats_all` で当日データを取得し、
  `load_manager.seed_tps_from_db()` に渡す。
  失敗時は `warn!` ログのみで正常起動を継続（FR-006準拠）

**チェックポイント**: T017, T018のテストが通過し、
`cargo test` で全テストが通過すること

---

## Phase 6: 仕上げ

**目的**: 品質チェックと最終検証

- [x] T022 全テスト通過確認: `cargo test -- --test-threads=1` で
  全テストが通過することを確認
- [x] T023 [P] Clippy通過確認: `cargo clippy -- -D warnings` で
  警告がないことを確認
- [x] T024 [P] フォーマット確認: `cargo fmt --check` で
  フォーマットが正しいことを確認
- [x] T025 quickstart.md の手動検証シナリオ (SC-001〜SC-005) の確認

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (US2 Latency)**: 依存なし、最小変更で即時開始可能
- **Phase 2 (US1 Request)**: 依存なし、Phase 1と並列可能だが
  registry/endpoints.rs の同時編集を避けるため順次推奨
- **Phase 3 (US3 Avg Response)**: 依存なし、Phase 1/2と並列可能
- **Phase 4 (US5 History)**: 依存なし、Phase 1-3と並列可能
- **Phase 5 (US4 TPS)**: 依存なし、Phase 1-4と並列可能
  ただしbootstrap.rsの編集がPhase 4と競合するため順次推奨
- **Phase 6 (仕上げ)**: 全フェーズ完了後

### 各Phase内

- テスト (RED) → 実装 (GREEN) の順序厳守
- [P]マークのテストタスクは並列実行可能
- 実装タスクは依存順に順次実行

### 並列機会

- Phase 1とPhase 3は完全に並列実行可能（異なるファイル）
- Phase 4とPhase 5のテストタスクは並列実行可能
- Phase 6の T023, T024 は並列実行可能
