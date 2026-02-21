# タスク: 監査ログ（Audit Log）

**入力**: `specs/SPEC-8301d106/` の設計ドキュメント
**前提条件**: spec.md, plan.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- **[Story]**: このタスクが属するユーザーストーリー（US1〜US6）

## Phase 1: 基盤（共有インフラ）

**目的**: 監査ログの全ユーザーストーリーが依存するコアインフラの構築

### テスト（RED）

- [x] T001 [P] [基盤] `llmlb/src/audit/types.rs` のユニットテスト作成:
  `AuditLogEntry`構造体の生成・`ActorType` enumのシリアライズ/デシリアライズ・
  `AuditBatchHash`構造体の生成をテスト。
  テストファイル: `llmlb/src/audit/types.rs` 内 `#[cfg(test)] mod tests`
- [x] T002 [P] [基盤] `llmlb/src/db/audit_log.rs` のユニットテスト作成:
  `AuditLogStorage::insert_batch`で複数レコードの一括挿入、
  `AuditLogStorage::query`でフィルタ・ページネーション・ソート、
  `AuditLogStorage::count`でレコード数取得をテスト。
  インメモリSQLiteプール（`sqlite::memory:`）を使用。
  テストファイル: `llmlb/src/db/audit_log.rs` 内 `#[cfg(test)] mod tests`
- [x] T003 [P] [基盤] `llmlb/src/audit/writer.rs` のユニットテスト作成:
  `AuditLogWriter::send`でエントリをバッファに送信、
  フラッシュ間隔（30秒）経過後にDBへ書き込み、
  バッファ上限（10,000件）超過時に最古エントリ破棄をテスト。
  テストファイル: `llmlb/src/audit/writer.rs` 内 `#[cfg(test)] mod tests`

### 実装（GREEN）

- [x] T004 [基盤] `llmlb/migrations/017_audit_log.sql` を作成:
  `audit_log_entries`テーブル（data-model.mdの全カラム定義）、
  `audit_batch_hashes`テーブル、
  `audit_log_fts` FTS5仮想テーブル、
  FTS同期トリガー（INSERT/DELETE）、
  全インデックス（timestamp, actor, path, status, batch, model, tokens）を定義。
  既存マイグレーション（001〜016）との整合性を維持
- [x] T005 [P] [基盤] `llmlb/src/audit/types.rs` を作成:
  `AuditLogEntry`構造体（data-model.mdの全フィールド）、
  `ActorType` enum（User/ApiKey/Anonymous、serde Serialize/Deserialize実装）、
  `AuditBatchHash`構造体、
  `AuditLogFilter`構造体（actor\_type, actor\_id, http\_method, request\_path,
  status\_code, time\_from, time\_to, search\_text, page, per\_page フィールド）を定義
- [x] T006 [P] [基盤] `llmlb/src/audit/mod.rs` を作成:
  `pub mod types;` `pub mod writer;` `pub mod middleware;` `pub mod hash_chain;`
  のモジュール宣言。`llmlb/src/lib.rs` に `pub mod audit;` を追加
- [x] T007 [基盤] `llmlb/src/db/audit_log.rs` を作成:
  `AuditLogStorage`構造体（`SqlitePool`を保持）。
  メソッド: `new(pool)`, `insert_batch(entries)` (一括INSERT、トランザクション使用),
  `query(filter) -> Vec<AuditLogEntry>` (フィルタ・ページネーション・ソート),
  `count(filter) -> i64`,
  `get_by_id(id) -> Option<AuditLogEntry>`。
  既存`ModelStorage`（`db/models.rs`）のパターンに準拠。
  `llmlb/src/db/mod.rs` に `pub mod audit_log;` を追加。
  T004のマイグレーション完了後にテスト実行可能
- [x] T008 [基盤] `llmlb/src/audit/writer.rs` を作成:
  `AuditLogWriter`構造体（`tokio::sync::mpsc::Sender<AuditLogEntry>`ラッパー）。
  `AuditLogWriter::new(storage, config)` で`mpsc::channel`を生成し、
  バックグラウンドタスク（`tokio::spawn`）を起動。
  バックグラウンドタスク: `tokio::time::interval(30s)`でフラッシュ、
  `VecDeque`バッファに蓄積、上限10,000件超過時は`pop_front`で最古破棄しwarn!ログ出力。
  `AuditLogWriter::send(entry)`は`try_send`で非同期送信（チャネルfull時もwarn!で続行）。
  `AuditLogWriter::shutdown()`でチャネルclose＋残存バッファフラッシュ。
  環境変数: `LLMLB_AUDIT_FLUSH_INTERVAL_SECS`（デフォルト30）、
  `LLMLB_AUDIT_BUFFER_CAPACITY`（デフォルト10000）
- [x] T009 [基盤] `llmlb/src/lib.rs` の `AppState` を拡張:
  `audit_log_writer: audit::writer::AuditLogWriter` フィールドを追加。
  `AppState`のClone実装が維持されることを確認（`AuditLogWriter`にClone実装）。
  `llmlb/src/cli/serve.rs`（または`main.rs`）のAppState初期化箇所で
  `AuditLogWriter::new`を呼び出してフィールドを設定

**チェックポイント**: `cargo test audit` で基盤テストが全て合格

---

## Phase 2: US1 - 全操作の自動記録（優先度: P1）

**目標**: 全HTTP操作のメタデータを自動的に監査ログに記録する

**独立テスト**: エンドポイント操作後、DBクエリで監査ログレコードの存在を確認

### テスト（RED）

- [x] T010 [P] [US1] `llmlb/src/audit/middleware.rs` のユニットテスト作成:
  ミドルウェアがGET/POST/PUT/DELETEリクエストのメタデータ
  （method, path, status\_code, duration\_ms）をキャプチャすることをテスト。
  WebSocketパス（`/ws/*`）、静的アセット（`/dashboard/*`のうちHTMLやJS等）が
  除外されることをテスト。
  テストファイル: `llmlb/src/audit/middleware.rs` 内 `#[cfg(test)] mod tests`
- [x] T011 [P] [US1] 統合テスト作成:
  `create_app`で作成したRouterにリクエストを送信し、
  `audit_log_entries`テーブルにレコードが挿入されることを検証。
  JWT認証済みリクエストで`actor_type='user'`、`actor_id`がClaims.subと一致することを検証。
  テストファイル: `llmlb/src/audit/middleware.rs` 内 `#[cfg(test)] mod tests`
  またはインテグレーションテストとして適切な場所

### 実装（GREEN）

- [x] T012 [US1] `llmlb/src/audit/middleware.rs` を作成:
  `pub async fn audit_middleware(State(writer): State<AuditLogWriter>, request: Request, next: Next) -> Response`。
  before: `Instant::now()`で開始時刻記録、`request.method()`, `request.uri().path()`,
  `request.headers()`からクライアントIP（`x-forwarded-for`または`x-real-ip`）を取得。
  除外判定: `/ws/`プレフィックス、`/dashboard/`の静的アセット（拡張子判定: .js, .css, .png等）、
  `/health`パスを除外。
  next.run(request)実行後: `response.status()`, `elapsed`を取得。
  `request.extensions()`から`Claims`（JWT認証時）または`ApiKeyAuthContext`（APIキー認証時）を取得し、
  `ActorType`と`actor_id`を決定。
  `AuditLogEntry`を構築して`writer.send(entry)`で送信
- [x] T013 [US1] `llmlb/src/api/mod.rs` の `create_app` を修正:
  `Router::new()`の最外層（`.with_state(state)`の直前）に
  `.layer(middleware::from_fn_with_state(state.audit_log_writer.clone(), audit::middleware::audit_middleware))`
  を追加。認証ミドルウェアより外側に配置し、認証失敗リクエストもキャプチャ
- [x] T014 [US1] 推論ハンドラーからのトークン数補足:
  `llmlb/src/api/openai.rs`の`chat_completions`ハンドラーで、
  レスポンスのトークン使用量を`request.extensions_mut().insert(TokenUsage{...})`で注入。
  `llmlb/src/audit/types.rs`に`TokenUsage`構造体（input\_tokens, output\_tokens, total\_tokens, model\_name, endpoint\_id）を追加。
  `audit_middleware`でレスポンス後に`extensions().get::<TokenUsage>()`で取得し`AuditLogEntry`に設定
- [x] T015 [US1] 認証失敗の記録:
  `llmlb/src/auth/middleware.rs`の認証失敗パスで、
  `request.extensions_mut().insert(AuthFailureInfo{...})`を注入。
  `llmlb/src/audit/types.rs`に`AuthFailureInfo`構造体（attempted\_username: Option\<String\>, reason: String）を追加。
  `audit_middleware`で`extensions().get::<AuthFailureInfo>()`を取得し、
  `actor_type=Anonymous`, `actor_username=attempted_username`として記録

**チェックポイント**: サーバー起動→各種操作→`audit_log_entries`テーブルにレコードが蓄積

---

## Phase 3: US2 - 監査ログの検索・閲覧（優先度: P1）

**目標**: ダッシュボードで監査ログを検索・フィルタ・閲覧可能にする

**独立テスト**: REST APIで監査ログの一覧取得・フィルタ検索が動作

### テスト（RED）

- [x] T016 [P] [US2] `llmlb/src/api/audit_log.rs` のユニットテスト作成:
  `GET /api/dashboard/audit-logs` でページネーション付きJSON一覧が返ること、
  `?actor_type=user&actor_id=xxx`フィルタが正しく動作すること、
  `?search=keyword`でFTS5検索が動作すること、
  admin以外（viewer）がアクセスすると403が返ることをテスト。
  テストファイル: `llmlb/src/api/audit_log.rs` 内 `#[cfg(test)] mod tests`
- [x] T017 [P] [US2] `llmlb/src/db/audit_log.rs` にFTS5検索テスト追加:
  `AuditLogStorage::search_fts(query)`でフリーテキスト検索が動作し、
  マッチするレコードのみ返ることをテスト

### 実装（GREEN）

- [x] T018 [US2] `llmlb/src/db/audit_log.rs` にFTS5検索メソッド追加:
  `search_fts(query, filter) -> Vec<AuditLogEntry>`:
  `SELECT ale.* FROM audit_log_entries ale JOIN audit_log_fts fts ON ale.id = fts.rowid WHERE audit_log_fts MATCH ?`
  にフィルタ条件を追加。`rank`でソート。ページネーション対応
- [x] T019 [US2] `llmlb/src/api/audit_log.rs` を作成:
  `pub async fn list_audit_logs(State(app_state): State<AppState>, Query(params): Query<AuditLogQueryParams>) -> Result<Json<AuditLogListResponse>, Response>`。
  `AuditLogQueryParams`: actor\_type, actor\_id, http\_method, request\_path,
  status\_code, time\_from, time\_to, search, page(default 1), per\_page(default 50)。
  `AuditLogListResponse`: items: Vec\<AuditLogEntry\>, total: i64, page: i64, per\_page: i64。
  searchパラメータ存在時はFTS5検索、それ以外は構造化フィルタクエリを使用
- [x] T020 [US2] `llmlb/src/api/audit_log.rs` に統計APIハンドラー追加:
  `pub async fn get_audit_log_stats(...)`:
  `GET /api/dashboard/audit-logs/stats` で期間別の操作数サマリ、
  アクター別操作数、HTTPメソッド別操作数を返す
- [x] T021 [US2] `llmlb/src/api/mod.rs` にルート追加:
  `dashboard_api_routes`に以下を追加:
  `.route("/dashboard/audit-logs", get(audit_log::list_audit_logs))`
  `.route("/dashboard/audit-logs/stats", get(audit_log::get_audit_log_stats))`
  JWT認証（admin限定）ミドルウェアを適用。
  `llmlb/src/api/mod.rs`の先頭に`pub mod audit_log;`を追加

**チェックポイント**: `curl -H "Authorization: Bearer <jwt>" /api/dashboard/audit-logs` で一覧取得可能

---

## Phase 4: US4 - request\_history統合とトークン統計の継続（優先度: P1）

**目標**: request\_historyを監査ログに統合し、トークン統計を維持する

**独立テスト**: 推論リクエスト後にダッシュボードのトークン統計が従来通り表示される

### テスト（RED）

- [x] T022 [P] [US4] `llmlb/src/db/audit_log.rs` にトークン統計テスト追加:
  `get_token_statistics` で全体のトークン累計が正しく返ること、
  `get_token_statistics_by_model`でモデル別集計が正しいこと、
  `get_daily_token_statistics`で日次集計が正しいこと、
  `get_monthly_token_statistics`で月次集計が正しいことをテスト。
  `RequestHistoryStorage`の同名メソッドと同じ結果形式を期待
- [x] T023 [P] [US4] マイグレーションテスト作成:
  インメモリDBに`request_history`テストデータを挿入し、
  移行SQLを実行後、`audit_log_entries`に`is_migrated=1`でデータが
  正しく移行されることを検証

### 実装（GREEN）

- [x] T024 [US4] `llmlb/src/db/audit_log.rs` にトークン統計メソッド追加:
  `get_token_statistics() -> TokenStatistics`:
  `SELECT SUM(input_tokens), SUM(output_tokens), SUM(total_tokens) FROM audit_log_entries WHERE total_tokens IS NOT NULL`。
  `get_token_statistics_by_model() -> Vec<ModelTokenStatistics>`:
  `GROUP BY model_name`。
  `get_daily_token_statistics(days) -> Vec<DailyTokenStatistics>`:
  `GROUP BY DATE(timestamp)`。
  `get_monthly_token_statistics(months) -> Vec<MonthlyTokenStatistics>`:
  `GROUP BY strftime('%Y-%m', timestamp)`。
  既存の`RequestHistoryStorage`のメソッドシグネチャと互換のレスポンス型を使用
- [x] T025 [US4] `llmlb/migrations/017_audit_log.sql` にrequest\_history移行SQLを追加:
  data-model.mdの移行SQLをマイグレーションに含める。
  `request_history`テーブルが存在する場合のみ実行（`IF EXISTS`相当のSQLite構文）。
  移行後も`request_history`テーブルは残す（Phase完了後に別マイグレーションで削除）
- [x] T026 [US4] ダッシュボード統計APIの切り替え:
  `llmlb/src/api/dashboard.rs`の`get_token_stats`, `get_daily_token_stats`,
  `get_monthly_token_stats`ハンドラーを修正し、
  `app_state.request_history`の代わりに`AuditLogStorage`を使用。
  `AppState`に`audit_log_storage: Arc<AuditLogStorage>`フィールドを追加し、
  `llmlb/src/lib.rs`と初期化コードを更新
- [x] T027 [US4] `llmlb/src/api/dashboard.rs`の`get_request_history`ハンドラーを修正:
  `audit_log_entries`テーブルから推論リクエスト
  （`request_path LIKE '/v1/%' AND total_tokens IS NOT NULL`）をフィルタして返す。
  既存のレスポンス形式との互換性を維持

**チェックポイント**: トークン統計ページが従来通り表示、request\_historyデータが移行済み

---

## Phase 5: US3 - 改ざん検知（ハッシュチェーン検証）（優先度: P2）

**目標**: SHA-256バッチハッシュチェーンによる監査ログの改ざん検知

**独立テスト**: 検証APIで正常/改ざんの検出結果が正しく返される

### テスト（RED）

- [x] T028 [P] [US3] `llmlb/src/audit/hash_chain.rs` のユニットテスト作成:
  `compute_batch_hash`で正しいSHA-256ハッシュが生成されること、
  `verify_chain`で正常チェーンがtrue、改ざんチェーンがfalseを返すこと、
  genesis batch（seq=1）で`previous_hash`がゼロハッシュであることをテスト
- [x] T029 [P] [US3] `llmlb/src/api/audit_log.rs` に検証APIテスト追加:
  `POST /api/dashboard/audit-logs/verify` で正常時に`{valid: true, batches_checked: N}`、
  改ざん検出時に`{valid: false, tampered_batch: N, ...}`が返ることをテスト

### 実装（GREEN）

- [x] T030 [US3] `llmlb/src/audit/hash_chain.rs` を作成:
  `compute_record_hash(entry: &AuditLogEntry) -> String`:
  エントリの主要フィールド（timestamp, http\_method, request\_path, status\_code,
  actor\_type, actor\_id）を連結してSHA-256。
  `compute_batch_hash(previous_hash, seq, start, end, count, records) -> String`:
  `SHA-256(previous_hash || seq || start || end || count || records_hash)`。
  `verify_chain(storage: &AuditLogStorage) -> ChainVerificationResult`:
  全バッチを先頭から順に検証、最初の不整合で`TamperedAt(batch_id)`を返す。
  `GENESIS_HASH`: `"0".repeat(64)`定数
- [x] T031 [US3] `llmlb/src/audit/writer.rs` のフラッシュ処理にハッシュチェーン統合:
  フラッシュ時にバッチ間隔（5分、`LLMLB_AUDIT_BATCH_INTERVAL_SECS`）をチェック。
  バッチ間隔経過時: `compute_batch_hash`でハッシュ計算、
  `AuditLogStorage::insert_batch_hash`でDB保存、
  エントリの`batch_id`を設定してから`insert_batch`で一括保存。
  バッチ間隔未経過時: エントリは`batch_id=NULL`で保存し、次バッチに含める
- [x] T032 [US3] `llmlb/src/db/audit_log.rs` にバッチハッシュCRUDメソッド追加:
  `insert_batch_hash(batch: &AuditBatchHash) -> i64`:
  バッチハッシュをINSERTしてIDを返す。
  `get_all_batch_hashes() -> Vec<AuditBatchHash>`: 全バッチを連番順で取得。
  `get_latest_batch_hash() -> Option<AuditBatchHash>`: 最新バッチを取得。
  `get_entries_for_batch(batch_id) -> Vec<AuditLogEntry>`: バッチ内エントリ取得
- [x] T033 [US3] 起動時検証と定期検証の実装:
  `llmlb/src/cli/serve.rs`（またはサーバー起動処理）に起動時検証を追加:
  `hash_chain::verify_chain`を呼び出し、結果をinfo!/warn!でログ出力。
  `tokio::spawn`で24時間間隔の定期検証タスクを起動。
  改ざん検出時: warn!ログ出力、新しいチェーン開始（次バッチのprevious\_hashをゼロハッシュに）
- [x] T034 [US3] `llmlb/src/api/audit_log.rs` に検証APIハンドラー追加:
  `pub async fn verify_hash_chain(...)`:
  `POST /api/dashboard/audit-logs/verify` で`hash_chain::verify_chain`を実行し結果を返す。
  `llmlb/src/api/mod.rs`にルート追加:
  `.route("/dashboard/audit-logs/verify", post(audit_log::verify_hash_chain))`

**チェックポイント**: 検証APIで`{valid: true}`が返り、DB改ざん後に`{valid: false}`が返る

---

## Phase 6: US5 - 監査ログのアーカイブとライフサイクル管理（優先度: P2）

**目標**: 90日以上のデータを自動アーカイブし、アーカイブデータも検索可能にする

**独立テスト**: 古いテストデータがアーカイブDBに移動し、API検索で返される

### テスト（RED）

- [x] T035 [P] [US5] アーカイブテスト作成:
  90日以上前のテストデータを`audit_log_entries`に挿入し、
  アーカイブ処理後にメインDBから削除されアーカイブDBに存在すること、
  FTSインデックスが更新されること、
  バッチハッシュもアーカイブDBにコピーされることをテスト。
  テストファイル: `llmlb/src/db/audit_log.rs` 内テストモジュール

### 実装（GREEN）

- [x] T036 [US5] `llmlb/src/db/audit_log.rs` にアーカイブ機能追加:
  `AuditLogStorage::archive_old_entries(retention_days, archive_pool)`:
  `timestamp < datetime('now', '-{retention_days} days')`のエントリを
  アーカイブDBにINSERT→メインDBからDELETE（トランザクション）。
  関連するバッチハッシュもアーカイブDBにコピー。
  環境変数: `LLMLB_AUDIT_RETENTION_DAYS`（デフォルト90）、
  `LLMLB_AUDIT_ARCHIVE_PATH`（デフォルト: データディレクトリ/audit\_archive.db）
- [x] T037 [US5] アーカイブDB管理:
  `llmlb/src/db/audit_log.rs`に`create_archive_pool(path) -> SqlitePool`:
  アーカイブDBファイルが存在しない場合は自動作成、
  メインDBと同じスキーマ（`audit_log_entries` + `audit_batch_hashes`テーブル）を
  マイグレーション実行。WALモード設定
- [x] T038 [US5] 統合検索（メインDB + アーカイブDB）:
  `llmlb/src/db/audit_log.rs`の`query`メソッドを拡張し、
  `include_archive: bool`パラメータを追加。
  trueの場合、メインDBとアーカイブDBの両方にクエリを発行し、
  結果をマージしてtimestampでソート。ページネーションはマージ後に適用。
  `llmlb/src/api/audit_log.rs`のクエリパラメータに`include_archive`を追加
- [x] T039 [US5] 定期アーカイブタスク:
  `llmlb/src/cli/serve.rs`に`tokio::spawn`で24時間間隔のアーカイブタスクを起動。
  `AuditLogStorage::archive_old_entries`を呼び出し、
  結果をinfo!ログに出力（移動件数）

**チェックポイント**: 90日超データがアーカイブDB移動、API検索でアーカイブ含む結果取得

---

## Phase 7: US6 - 監査ログAPI（プログラマティックアクセス）（優先度: P2）

**目標**: REST APIで監査ログを外部システムから取得可能にする

**独立テスト**: curlで監査ログAPIにリクエストし、フィルタ付きJSON結果が返る

### テスト（RED）

- [x] T040 [P] [US6] REST APIのcontract test作成:
  `GET /api/dashboard/audit-logs`のレスポンスJSON構造が
  `{items: [...], total: N, page: N, per_page: N}`であることを検証。
  各itemが必須フィールド（id, timestamp, http\_method, request\_path,
  status\_code, actor\_type）を含むことを検証。
  `GET /api/dashboard/audit-logs/stats`のレスポンス構造を検証

### 実装（GREEN）

- [x] T041 [US6] Phase 3（US2）のAPI実装で大部分はカバー済み。
  追加として`llmlb/src/api/audit_log.rs`にエクスポート機能追加:
  `GET /api/dashboard/audit-logs?format=csv`でCSVエクスポート
  （将来拡張用のフォーマットパラメータ）。
  初期スコープではJSONのみ。CSVは将来対応としてパラメータだけ受け付け、
  JSON以外が指定された場合は400を返す
- [x] T042 [US6] admin以外のアクセス制御テスト・実装確認:
  `llmlb/src/api/mod.rs`の監査ログルートがadmin限定ミドルウェア配下にあることを確認。
  viewer/APIキーでのアクセス時に403が返ることをテストで検証

**チェックポイント**: curlで全APIエンドポイントにアクセスし、正しいレスポンスを確認

---

## Phase 8: ダッシュボードUI

**目的**: 管理者専用の監査ログダッシュボードページ

### テスト（RED）

- [x] T043 [P] [US2] ダッシュボードの監査ログページが正しくレンダリングされること、
  admin以外でアクセス時にリダイレクトまたは非表示であることを手動テストで確認
  （E2Eテスト対象）

### 実装（GREEN）

- [x] T044 [P] [US2] `llmlb/src/web/dashboard/src/components/audit/AuditLogTable.tsx` を作成:
  shadcn/uiの`Table`コンポーネントを使用。
  カラム: timestamp, http\_method, request\_path, status\_code,
  actor\_type, actor\_id, duration\_ms。
  推論リクエスト行はトークン数も表示。
  行クリックで詳細モーダル表示。
  既存の`RequestHistoryTable.tsx`のパターンに準拠
- [x] T045 [P] [US2] `llmlb/src/web/dashboard/src/components/audit/AuditLogFilters.tsx` を作成:
  shadcn/uiの`Select`, `Input`, `DatePicker`を使用。
  フィルタ: actor\_type（ドロップダウン）、actor\_id（テキスト入力）、
  http\_method（ドロップダウン）、status\_code（ドロップダウン）、
  time\_from/time\_to（日時ピッカー）、search（フリーテキスト入力）。
  フィルタ変更時にAPI再リクエスト（debounce 300ms）
- [x] T046 [P] [US2] `llmlb/src/web/dashboard/src/components/audit/HashChainStatus.tsx` を作成:
  「検証実行」ボタン。クリックで`POST /api/dashboard/audit-logs/verify`を呼び出し、
  結果を表示（成功: 緑バッジ「検証成功」、失敗: 赤バッジ「改ざん検出: バッチN」）。
  実行中はローディングスピナー表示
- [x] T047 [US2] `llmlb/src/web/dashboard/src/pages/AuditLog.tsx` を作成:
  `AuditLogFilters` + `AuditLogTable` + `HashChainStatus`を組み合わせたページ。
  `useEffect`で`/api/dashboard/audit-logs`をフェッチ。
  ページネーションコンポーネント（ページ番号ボタン + 前へ/次へ）。
  admin限定ページ（`useAuth`フックでロールチェック）
- [x] T048 [US2] `llmlb/src/web/dashboard/src/App.tsx` を修正:
  ハッシュルーティングに`#audit-log`を追加。
  `Header.tsx`のナビゲーションに「Audit Log」リンクを追加（admin限定表示）。
  ルート定義で`AuditLog`ページコンポーネントをlazy importで読み込み
- [x] T049 [US2] ダッシュボードビルド:
  `pnpm --filter @llm/dashboard build`を実行し、
  `llmlb/src/web/static/`に成果物を生成。
  生成物をコミット対象に含める

**チェックポイント**: ダッシュボードの「Audit Log」ページで監査ログの閲覧・検索・検証が動作

---

## Phase 9: request\_history廃止・仕上げ

**目的**: 旧機能の完全削除とコード品質向上

- [ ] T050 [US4] **DEFERRED** `llmlb/migrations/018_drop_request_history.sql` を作成:
  `DROP TABLE IF EXISTS request_history;` でテーブル削除。
  Phase 4完了・全テスト合格後に適用
- [ ] T051 [US4] **DEFERRED** `llmlb/src/db/request_history.rs` を削除。
  `llmlb/src/db/mod.rs`から`pub mod request_history;`を削除。
  `llmlb/src/lib.rs`の`AppState`から`request_history`フィールドを削除。
  `request_history`を参照する全コード（`cli/serve.rs`, `api/mod.rs`,
  `api/dashboard.rs`, `auth/middleware.rs`のテスト等）を`audit_log_storage`に置き換え。
  コンパイルエラーを全て解消
- [x] T052 コードクリーンアップ:
  `cargo fmt`、`cargo clippy -- -D warnings`で全警告を解消。
  不要なimport、未使用コードを削除。
  `#[allow(...)]`属性は正当な理由がある場合のみ残す
- [x] T053 品質チェック実行:
  `make quality-checks`を実行し、全チェック（fmt, clippy, test,
  commitlint, markdownlint, check-tasks）が合格することを確認。
  timeout: 900000ms

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1（基盤）**: 依存なし - 最初に着手
- **Phase 2（US1: 記録）**: Phase 1完了に依存 - ミドルウェアはバッファとストレージが必要
- **Phase 3（US2: 検索）**: Phase 2完了に依存 - 検索にはデータが必要
- **Phase 4（US4: 統合）**: Phase 1完了に依存 - Phase 2と並列可能
- **Phase 5（US3: ハッシュチェーン）**: Phase 2完了に依存 - 記録機能が動作後
- **Phase 6（US5: アーカイブ）**: Phase 5完了に依存 - ハッシュチェーンとの整合性
- **Phase 7（US6: API）**: Phase 3完了に依存 - 検索API拡張
- **Phase 8（UI）**: Phase 3, 5完了に依存 - API完成後
- **Phase 9（廃止）**: Phase 4完了 + 全テスト合格後

### 並列実行マップ

```text
Phase 1 (基盤)
  ├── Phase 2 (US1: 記録) ──→ Phase 3 (US2: 検索) ──→ Phase 7 (US6: API)
  │                         └── Phase 5 (US3: ハッシュ) ──→ Phase 6 (US5: アーカイブ)
  │                                                      └── Phase 8 (UI)
  └── Phase 4 (US4: 統合) ──→ Phase 9 (廃止)
```

### 各Phase内の並列機会

- T001/T002/T003: 全テスト並列実行可能
- T005/T006: 型定義とモジュール宣言は並列
- T010/T011: ミドルウェアテスト並列
- T016/T017: 検索テスト並列
- T022/T023: 統計・移行テスト並列
- T028/T029: ハッシュチェーンテスト並列
- T044/T045/T046: UIコンポーネント並列

## 注意事項

- TDD厳守: 各Phaseのテスト（RED）→実装（GREEN）の順序を必ず守る
- 既存テストの破壊禁止: `cargo test`で全テストが常に合格する状態を維持
- AppState変更時: 全テストのtest\_state()ヘルパーも更新が必要
- ダッシュボード変更時: `pnpm --filter @llm/dashboard build`でstatic/を再生成
- request\_history廃止は最終Phase: 全機能が安定してから実施
