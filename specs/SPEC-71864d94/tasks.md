# タスク: llmlb全体的リファクタリング（メジャーバージョンアップ）

**入力**: `/specs/SPEC-71864d94/` の設計ドキュメント
**前提条件**: plan.md (必須), spec.md (ユーザーストーリーに必須)

**構成**: タスクはPhase順にグループ化され、Phase間依存関係に従って実行する。

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1〜US8)

---

## Phase 1: 型基盤 (US1 - 型定義の一元管理と命名統一)

**目標**: `common/types.rs`の全型を`types/`に移動し、GPU型統一とレガシー命名を一掃する

**独立テスト**: 全265テスト通過 + `grep -r "node_id\|node_name\|node_ip" llmlb/src/ --include="*.rs"` がゼロ件

### テスト (RED)

- [ ] T001 [US1] `llmlb/src/common/protocol.rs`のテストモジュールに、RequestResponseRecordの`endpoint_id`/`endpoint_name`/`endpoint_ip`フィールドが存在することを検証するテストを追加。現在のnode_フィールドで失敗することを確認。

### 実装

- [ ] T002 [P] [US1] `llmlb/src/types/health.rs`を新規作成。`common/types.rs`のL254-328から`HealthMetrics`/`Request`/`RequestStatus`を移動し、`node_id`フィールドを`endpoint_id`にリネーム。関連するserde属性・derive・impl也全て移動。
- [ ] T003 [P] [US1] `llmlb/src/types/model.rs`を新規作成。`common/types.rs`のL37-178から`SyncState`/`SyncProgress`/`ModelType`/`RuntimeType`/`ModelCapability`/`ModelCapabilities`を移動。
- [ ] T004 [P] [US1] `llmlb/src/types/media.rs`を新規作成。`common/types.rs`のL182-251から`AudioFormat`/`ImageSize`/`ImageQuality`/`ImageStyle`/`ImageResponseFormat`を移動。
- [ ] T005 [US1] `llmlb/src/types/endpoint.rs`を更新。`common/types.rs`のL15-30の`GpuDeviceInfo`を削除し、既存の`GpuDevice`（L34-43）に統一。`GpuDeviceInfo`を参照していた箇所を`GpuDevice`に更新（`count`フィールドは`Vec<GpuDevice>`の長さで代替）。
- [ ] T006 [US1] `llmlb/src/types/mod.rs`を更新。`pub mod health;`/`pub mod model;`/`pub mod media;`を追加し、全型をre-export。
- [ ] T007 [US1] `llmlb/src/common/types.rs`を削除。`common/mod.rs`から`pub mod types;`を削除。
- [ ] T008 [US1] コードベース全体の`use crate::common::types::`を`use crate::types::`に一括更新。対象ファイルをgrepで特定し、全てのimportパスを修正。`cargo check`で検証。
- [ ] T009 [US1] `common/protocol.rs`のRequestResponseRecord（L61-99）のフィールドをリネーム: `node_id`→`endpoint_id`、`node_machine_name`→`endpoint_name`、`node_ip`→`endpoint_ip`。全参照箇所を更新。
- [ ] T010 [US1] `common/types.rs`から移動した`HealthMetrics`の`node_id`→`endpoint_id`リネームに伴い、`health/endpoint_checker.rs`/`balancer/mod.rs`等の参照箇所を全て更新。
- [ ] T011 [US1] `common/types.rs`から移動した`Request`の`node_id`→`endpoint_id`リネームに伴い、`db/request_history.rs`等の参照箇所を全て更新。
- [ ] T012 [US1] コードベース全体でローカル変数名・関数引数名の`node_id`/`node_name`/`node_ip`/`node_machine_name`を`endpoint_id`/`endpoint_name`/`endpoint_ip`に一括リネーム。grepで残存確認しゼロ件を達成。

**チェックポイント**: `cargo test` 全通過、`grep -rn "node_id\|node_name\|node_ip\|node_machine_name" llmlb/src/ --include="*.rs"` がゼロ件

---

## Phase 2: エラー基盤 (US2 - エラーハンドリングの統一とセキュリティ修正)

**目標**: AppError一本化、全APIハンドラーのエラー返却を統一

**独立テスト**: APIエラーレスポンスに内部IPが含まれないことをテストで検証

### テスト (RED)

- [ ] T013 [US2] `llmlb/src/api/`のテストモジュールに、エラーレスポンスボディに内部IPアドレスパターン（`10.`/`192.168.`/`172.`）が含まれないことを検証するテストを追加。現在のapi/models.rsのAppErrorで失敗することを確認。

### 実装

- [ ] T014 [US2] `llmlb/src/api/models.rs`のL962-996の重複AppError定義を削除。`use super::error::AppError;`に置換。コンパイルエラーを解消。
- [ ] T015 [P] [US2] `llmlb/src/api/endpoints.rs`のインラインErrorResponse構築（約20箇所）を全てAppError返却に置換。`(StatusCode, Json(ErrorResponse { ... }))`パターンを`AppError(LbError::Xxx)`に変更。必要に応じてLbErrorバリアントを`common/error.rs`に追加。
- [ ] T016 [P] [US2] `llmlb/src/api/dashboard.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T017 [P] [US2] `llmlb/src/api/audio.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T018 [P] [US2] `llmlb/src/api/images.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T019 [P] [US2] `llmlb/src/api/responses.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T020 [P] [US2] `llmlb/src/api/cloud_models.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T021 [P] [US2] `llmlb/src/api/auth.rs`のインラインErrorResponse構築をAppError返却に置換。
- [ ] T022 [US2] コードベース全体でインラインErrorResponse構築が残っていないことをgrepで確認。`grep -rn "ErrorResponse {" llmlb/src/api/ --include="*.rs"`の結果が型定義のみであることを検証。

**チェックポイント**: `cargo test` 全通過、T013のセキュリティテスト成功

---

## Phase 3: API層リファクタリング (US3〜US5)

**目標**: CloudProvider trait化、auth_disabled廃止、RequestResponseRecordファクトリ導入

**独立テスト**: 3プロバイダへのプロキシが正常動作、auth_disabledがゼロ、手動Record構築がゼロ

### US3 - CloudProvider trait化

#### テスト (RED)

- [ ] T023 [US3] `llmlb/src/api/cloud_proxy.rs`（新規）のテストモジュールに、CloudProvider traitの各メソッド（`api_base_url`/`auth_header`/`transform_request`/`transform_response`/`map_error`）をモック実装でテストするユニットテストを追加。ファイル未作成の状態で失敗確認。

#### 実装

- [ ] T024 [US3] `llmlb/src/api/cloud_proxy.rs`を新規作成。`CloudProvider` traitを定義: `fn api_base_url(&self) -> &str`, `fn auth_header(&self) -> Result<(String, String), AppError>`, `fn transform_request(&self, payload: Value, model: &str) -> Result<(Value, String), AppError>`, `fn transform_response(&self, response: Response) -> Result<Response, AppError>`, `fn transform_stream_chunk(&self, chunk: &[u8]) -> Result<Vec<u8>, AppError>`, `fn map_error(&self, error: reqwest::Error) -> AppError`。
- [ ] T025 [P] [US3] `llmlb/src/api/cloud_proxy.rs`に`OpenAiProvider`構造体を実装。`api/openai.rs`のL729-829の`proxy_openai_provider`関数からロジックを抽出してCloudProvider traitを実装。
- [ ] T026 [P] [US3] `llmlb/src/api/cloud_proxy.rs`に`GoogleProvider`構造体を実装。`api/openai.rs`のL830-956の`proxy_google_provider`関数からロジックを抽出してCloudProvider traitを実装。
- [ ] T027 [P] [US3] `llmlb/src/api/cloud_proxy.rs`に`AnthropicProvider`構造体を実装。`api/openai.rs`のL957-1198の`proxy_anthropic_provider`関数からロジックを抽出してCloudProvider traitを実装。
- [ ] T028 [US3] `llmlb/src/api/cloud_proxy.rs`にジェネリック`proxy_cloud_provider<P: CloudProvider>(provider: &P, http_client: &reqwest::Client, payload: Value, stream: bool, model: String) -> Result<CloudProxyResult, AppError>`関数を実装。3つの個別proxy関数を置換。
- [ ] T029 [US3] `llmlb/src/api/openai_util.rs`を新規作成。`api/openai.rs`から以下の関数を移動: `sanitize_openai_payload_for_history()`、`map_openai_messages_to_google_contents()`、`map_openai_messages_to_anthropic()`、エラーレスポンスヘルパー群（`openai_error_response`/`queue_error_response`/`model_unavailable_response`等）。
- [ ] T030 [US3] `llmlb/src/api/openai.rs`を更新。移動した関数への参照を`cloud_proxy`と`openai_util`からのimportに置換。3つのproxy_xxx_provider関数を削除し、proxy_cloud_providerの呼び出しに統一。`api/mod.rs`にモジュール宣言追加。

### US4 - 認証フロー簡素化

#### テスト (RED)

- [ ] T031 [US4] `llmlb/src/auth/`のテストモジュールに、`#[cfg(debug_assertions)]`環境でadmin/testユーザーとsk_debug APIキーが自動生成されることを検証するテストを追加。現在の実装で失敗確認。

#### 実装

- [ ] T032 [US4] `llmlb/src/auth/bootstrap.rs`を更新。`#[cfg(debug_assertions)]`ブロックで、DB初期化時にadmin/testユーザーとsk_debug APIキーを自動作成するロジックを追加。既に存在する場合はスキップ。
- [ ] T033 [US4] `llmlb/src/config.rs`から`is_auth_disabled()`関数と関連する`AUTH_DISABLED`定数/環境変数読み取りを削除。
- [ ] T034 [US4] `llmlb/src/auth/middleware.rs`から`inject_dummy_admin_claims_with_state`関数を削除。
- [ ] T035 [US4] `llmlb/src/api/mod.rs`の`create_app()`から16箇所の`if auth_disabled { ... } else { ... }`分岐を削除。`apply_auth_layer(router: Router, state: AppState, config: JwtOrApiKeyPermissionConfig) -> Router`ヘルパー関数を作成し、各ルートの認証レイヤー適用を宣言的に記述。
- [ ] T036 [US4] `cargo test`で全テスト通過を確認。`grep -rn "auth_disabled\|is_auth_disabled\|inject_dummy_admin_claims" llmlb/src/ --include="*.rs"`がゼロ件であることを検証。

### US5 - RequestResponseRecordファクトリ

#### テスト (RED)

- [ ] T037 [US5] `llmlb/src/common/protocol.rs`のテストモジュールに、`RequestResponseRecord::error()`と`RequestResponseRecord::success()`ファクトリ関数のテストを追加。フィールドが正しく設定されること、endpoint_idフィールドが存在することを検証。現在のコードで失敗確認。

#### 実装

- [ ] T038 [US5] `llmlb/src/common/protocol.rs`にRequestResponseRecordのファクトリ関数を追加: `pub fn error(model: String, request_type: RequestType, request_body: Option<String>, message: String) -> Self`（endpoint_id=Uuid::nil, endpoint_name="N/A", endpoint_ip=UNSPECIFIED）、`pub fn success(endpoint_id: Uuid, endpoint_name: String, endpoint_ip: IpAddr, model: String, request_type: RequestType, ...) -> Self`。
- [ ] T039 [US5] `llmlb/src/api/openai.rs`の全RequestResponseRecord手動構築箇所（約10箇所）をファクトリ関数呼び出しに置換。
- [ ] T040 [P] [US5] `llmlb/src/api/audio.rs`のRequestResponseRecord手動構築箇所（約2箇所）をファクトリ関数呼び出しに置換。
- [ ] T041 [P] [US5] `llmlb/src/api/images.rs`のRequestResponseRecord手動構築箇所（約3箇所）をファクトリ関数呼び出しに置換。
- [ ] T042 [US5] コードベース全体でRequestResponseRecordの直接構築（`RequestResponseRecord { ... }`）が残っていないことをgrepで確認。ファクトリ関数とテスト内の構築のみ許容。

**チェックポイント**: `cargo test` 全通過、`grep -rn "auth_disabled" llmlb/src/`がゼロ件、RequestResponseRecord直接構築ゼロ件、openai.rs 500行以内

---

## Phase 4: DB層 (US6 - DB操作のテスタビリティ向上)

**目標**: Repository traitパターン導入、DB migration、レガシー命名一掃

**独立テスト**: Repository traitモック実装でのユニットテストが動作

### テスト (RED)

- [ ] T043 [US6] `llmlb/src/db/traits.rs`（新規）にEndpointRepositoryとRequestHistoryRepositoryのtrait定義を作成し、テストモジュールでモック実装を使ったテストを追加。trait未実装の状態で失敗確認。

### 実装

- [ ] T044 [US6] `llmlb/src/db/traits.rs`を新規作成。以下のRepository traitを定義: `EndpointRepository`（CRUD + status更新 + ヘルスチェック記録）、`RequestHistoryRepository`（保存 + クエリ + 集計）、`ModelRepository`（CRUD + 検索）、`UserRepository`（CRUD + 認証）、`ApiKeyRepository`（CRUD + 検証）、`InvitationRepository`（CRUD + 使用）、`EndpointDailyStatsRepository`（記録 + クエリ）、`DownloadTaskRepository`（CRUD + 進捗更新）。各traitは`#[async_trait]`を使用。
- [ ] T045 [P] [US6] `llmlb/src/db/endpoints.rs`を更新。既存の関数群を`impl EndpointRepository for SqlitePool`に再構成。公開インターフェースは維持。
- [ ] T046 [P] [US6] `llmlb/src/db/request_history.rs`を更新。既存の関数群を`impl RequestHistoryRepository for SqlitePool`に再構成。
- [ ] T047 [P] [US6] `llmlb/src/db/models.rs`を更新。既存の関数群を`impl ModelRepository for SqlitePool`に再構成。
- [ ] T048 [P] [US6] `llmlb/src/db/users.rs`/`api_keys.rs`/`invitations.rs`を更新。各Repository traitを実装。
- [ ] T049 [P] [US6] `llmlb/src/db/endpoint_daily_stats.rs`/`download_tasks.rs`を更新。各Repository traitを実装。
- [ ] T050 [US6] `llmlb/migrations/017_rename_node_to_endpoint.sql`を新規作成。`request_history`テーブルのカラムリネーム: `ALTER TABLE request_history RENAME COLUMN node_id TO endpoint_id; ALTER TABLE request_history RENAME COLUMN node_machine_name TO endpoint_name; ALTER TABLE request_history RENAME COLUMN node_ip TO endpoint_ip;`。
- [ ] T051 [US6] `llmlb/src/db/request_history.rs`のSQLクエリ内の`node_id`/`node_machine_name`/`node_ip`カラム参照を`endpoint_id`/`endpoint_name`/`endpoint_ip`に更新。
- [ ] T052 [US6] `llmlb/src/db/mod.rs`を更新。`pub mod traits;`を追加し、全Repository traitをre-export。
- [ ] T053 [US6] `llmlb/src/api/mod.rs`の`/nodes/{node_id}/logs`ルートを`/endpoints/{id}/logs`に変更。対応するハンドラー関数のパラメータ名も更新。

**チェックポイント**: `cargo test` 全通過、DB migration正常動作

---

## Phase 5: バランサー分割 (US7 - 可読性向上)

**目標**: balancer/mod.rsをtypes.rs/lease.rs/mod.rsに分割

**独立テスト**: 全テスト通過（純粋なファイル分割）

- [ ] T054 [P] [US7] `llmlb/src/balancer/types.rs`を新規作成。`balancer/mod.rs`のL39-767から型定義群を移動: `RequestOutcome`/`WaitResult`/`QueueWaiterGuard`/`AdmissionDecision`/`ModelTpsState`/`EndpointTpsSummary`/`EndpointLoadState`/`EndpointLoadSnapshot`/`SystemSummary`。関連するimpl/derive/serde属性も全て移動。
- [ ] T055 [P] [US7] `llmlb/src/balancer/lease.rs`を新規作成。`balancer/mod.rs`のL898-989から`RequestLease`構造体と関連implを移動。
- [ ] T056 [US7] `llmlb/src/balancer/mod.rs`を更新。`pub mod types;`/`pub mod lease;`を追加し、移動した型をre-export。LoadManager本体とヘルパー関数のみが残る状態にする。use文を更新。

**チェックポイント**: `cargo test` 全通過

---

## Phase 6: エントリポイント分割 (US7 - 可読性向上)

**目標**: main.rsを200行以内にスリム化

**独立テスト**: サーバー正常起動、全テスト通過

- [ ] T057 [US7] `llmlb/src/main.rs`のL255-429のマイグレーションチェックサム互換コード（`MIGRATION_005_OLD_CHECKSUM`〜`reconcile_migration_checksums()`関数）を全削除。`run_server()`内の`reconcile_migration_checksums()`呼び出し（L599）も削除。
- [ ] T058 [US7] `llmlb/src/bootstrap.rs`を新規作成。`main.rs`の`run_server()`（L561-732）から初期化ロジックを抽出: DB接続(`init_db_pool`)、マイグレーション実行、EndpointRegistry初期化、LoadManager初期化、HTTPクライアント生成、redetect_all_endpoints、EndpointHealthChecker起動、RequestHistoryStorage初期化、auth bootstrap、JWT secret取得、InferenceGate/ShutdownController/UpdateManager初期化。`pub async fn initialize(config: &Config) -> Result<AppState>`関数として提供。
- [ ] T059 [US7] `llmlb/src/server.rs`を新規作成。`main.rs`の`run_server()`からaxumサーバー起動部分を抽出: `create_app()`呼び出し、`axum::serve()`、シグナルハンドリング。`pub async fn run(state: AppState, config: &Config) -> Result<()>`関数として提供。
- [ ] T060 [US7] `llmlb/src/main.rs`を更新。CLI解析 + `bootstrap::initialize()` + `server::run()`呼び出しのみに簡素化。`lib.rs`に`pub mod bootstrap;`/`pub mod server;`を追加。200行以内であることを確認。

**チェックポイント**: `cargo test` 全通過、`wc -l llmlb/src/main.rs` ≤ 200

---

## Phase 7: テスト基盤 (US6 - テスタビリティ向上)

**目標**: TestAppStateBuilder導入、テストユーティリティ統合

**独立テスト**: 全テストがTestAppStateBuilder経由で動作

- [ ] T061 [US6] `llmlb/src/db/test_utils.rs`を拡張。`pub async fn test_db_pool() -> SqlitePool`関数（インメモリSQLite + マイグレーション実行）を追加。
- [ ] T062 [US6] `llmlb/src/db/test_utils.rs`に`TestAppStateBuilder`を追加。`TestAppStateBuilder::new().await`でデフォルトAppStateを構築、`.with_endpoints(vec![...])` `.with_auth_config(...)` `.with_queue_config(...)` `.build()`でカスタマイズ可能。内部で`test_db_pool()`を使用。
- [ ] T063 [US6] `llmlb/src/api/openai.rs`のテストモジュール内`create_local_state()`（L1782付近）をTestAppStateBuilderに置換。
- [ ] T064 [P] [US6] `llmlb/src/api/responses.rs`のテストモジュール内`create_local_state()`（L417付近）をTestAppStateBuilderに置換。
- [ ] T065 [US6] コードベース全体の`SqlitePool::connect("sqlite::memory:")`パターン（15箇所以上）をtest_db_pool()呼び出しに置換。grepで残存確認。

**チェックポイント**: `cargo test` 全通過、`grep -rn "sqlite::memory:" llmlb/src/ --include="*.rs"` がテストユーティリティ内の1箇所のみ

---

## Phase 8: Dashboard (US8 - Dashboard開発体験の改善)

**目標**: API分割、Playground共通基盤

**独立テスト**: `pnpm --filter @llm/dashboard build`成功、E2Eテスト通過

### 8a. API分割

- [ ] T066 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/client.ts`を新規作成。`lib/api.ts`から`fetchWithAuth()`/`ApiError`クラス/`BASE_URL`定数を抽出。
- [ ] T067 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/types.ts`を新規作成。`lib/api.ts`から共通型（`PaginatedResponse`等のジェネリック型）を抽出。
- [ ] T068 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/auth.ts`を新規作成。`lib/api.ts`から`authApi`オブジェクトと`LoginRequest`/`LoginResponse`型を抽出。
- [ ] T069 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/dashboard.ts`を新規作成。`lib/api.ts`から`dashboardApi`オブジェクトと関連型を抽出。
- [ ] T070 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/endpoints.ts`を新規作成。`lib/api.ts`から`endpointsApi`オブジェクトと`Endpoint`/`EndpointType`等の型を抽出。
- [ ] T071 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/models.ts`を新規作成。`lib/api.ts`から`modelsApi`オブジェクトと関連型を抽出。
- [ ] T072 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/chat.ts`を新規作成。`lib/api.ts`から`chatApi`オブジェクトと`ChatMessage`型を抽出。
- [ ] T073 [P] [US8] `llmlb/src/web/dashboard/src/lib/api/system.ts`/`api-keys.ts`/`invitations.ts`/`users.ts`を新規作成。対応するAPIオブジェクトと型を抽出。
- [ ] T074 [US8] `llmlb/src/web/dashboard/src/lib/api/index.ts`を新規作成。全APIモジュールをre-export。
- [ ] T075 [US8] `llmlb/src/web/dashboard/src/`の全コンポーネント/ページから`import { ... } from '../lib/api'`のインポートパスを`'../lib/api/index'`または個別モジュールに更新。旧`lib/api.ts`を削除。ビルド成功を確認。

### 8b. Playground共通基盤

- [ ] T076 [P] [US8] `llmlb/src/web/dashboard/src/components/playground/types.ts`を新規作成。`LoadBalancerPlayground.tsx`と`EndpointPlayground.tsx`から共通型（`Message`/`MessageAttachment`/`API_KEY_STORAGE_KEY`）を抽出。
- [ ] T077 [P] [US8] `llmlb/src/web/dashboard/src/hooks/usePlayground.ts`を新規作成。両Playgroundから共通のストリーミング送信・受信ロジック、メッセージ状態管理、API key管理を抽出してカスタムフックとして実装。
- [ ] T078 [P] [US8] `llmlb/src/web/dashboard/src/components/playground/MessageList.tsx`を新規作成。メッセージ表示ロジック（マークダウンレンダリング、添付ファイル表示、ロール別スタイリング）を共通コンポーネントとして抽出。
- [ ] T079 [P] [US8] `llmlb/src/web/dashboard/src/components/playground/ChatForm.tsx`を新規作成。入力フォーム（テキスト入力、添付ファイル、送信ボタン、モデル選択）を共通コンポーネントとして抽出。
- [ ] T080 [US8] `llmlb/src/web/dashboard/src/components/playground/PlaygroundBase.tsx`を新規作成。レイアウト構造（サイドバー + メイン + フォーム）を共通コンポーネントとして実装。MessageList/ChatFormを内包。
- [ ] T081 [US8] `llmlb/src/web/dashboard/src/pages/LoadBalancerPlayground.tsx`をPlaygroundBase + usePlayground + 差分のみの実装に書き換え。LB固有ロジック（ロードテスト機能等）のみを保持。
- [ ] T082 [US8] `llmlb/src/web/dashboard/src/pages/EndpointPlayground.tsx`をPlaygroundBase + usePlayground + 差分のみの実装に書き換え。エンドポイント固有ロジック（直接プロキシ等）のみを保持。
- [ ] T083 [US8] `pnpm --filter @llm/dashboard build`を実行し成功を確認。ビルド成果物を`llmlb/src/web/static/`に出力。

**チェックポイント**: ダッシュボードビルド成功、Message型重複ゼロ

---

## Phase 9: 仕上げ＆横断的関心事

**目的**: 品質チェック全通過、ドキュメント更新

- [ ] T084 `cargo fmt --check`を実行し、フォーマット違反をゼロにする。
- [ ] T085 `cargo clippy -- -D warnings`を実行し、警告をゼロにする。
- [ ] T086 `cargo test`を実行し、全テスト通過を確認。
- [ ] T087 [P] `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`を実行し、markdown警告をゼロにする。
- [ ] T088 [P] `docs/architecture.md`を更新。Node→Endpoint用語統一を反映。
- [ ] T089 [P] `README.md`/`README.ja.md`を更新。v5.0.0の破壊的変更を記載。
- [ ] T090 `.specify/scripts/checks/check-tasks.sh`を実行し通過を確認。
- [ ] T091 `.specify/scripts/checks/check-commits.sh --from origin/main --to HEAD`を実行しcommitlint通過を確認。
- [ ] T092 最終確認: 成功基準SC-001〜SC-010の全項目を検証し、全て達成されていることを確認。

**チェックポイント**: 全品質チェック通過、全成功基準達成

---

## 依存関係＆実行順序

### Phase依存関係

```text
Phase 1 (型基盤: T001-T012)
  ↓ 必須
Phase 2 (エラー基盤: T013-T022)
  ↓ 必須
Phase 3 (API層: T023-T042)  ←→  Phase 8 (Dashboard: T066-T083) ※並行可能
  ↓ 必須
Phase 4 (DB層: T043-T053)
  ↓ 必須
Phase 5 (バランサー: T054-T056) ※Phase 4完了後
Phase 6 (エントリポイント: T057-T060) ※Phase 4完了後、Phase 5と並行可能
Phase 7 (テスト基盤: T061-T065) ※Phase 4完了後、Phase 5/6と並行可能
  ↓ 全Phase完了後
Phase 9 (仕上げ: T084-T092)
```

### 並列機会

- **Phase 1内**: T002/T003/T004は並列実行可能（異なるファイル作成）
- **Phase 2内**: T015〜T021は並列実行可能（異なるapi/*.rsファイル）
- **Phase 3内**: T025/T026/T027は並列実行可能（異なるProvider実装）
- **Phase 3+8**: API層とDashboardは並行実施可能
- **Phase 4内**: T045〜T049は並列実行可能（異なるdb/*.rsファイル）
- **Phase 5+6+7**: バランサー・エントリポイント・テスト基盤は並行実施可能
- **Phase 8a内**: T066〜T073は並列実行可能（異なるAPIファイル作成）
- **Phase 8b内**: T076〜T079は並列実行可能（異なるコンポーネント作成）
- **Phase 9内**: T087〜T089は並列実行可能

---

## 実装戦略

### Agent Team並列戦略

Phase 1-2は順序依存のため逐次実行。Phase 3以降はAgent Teamで最大効率化:

**Wave 1** (Phase 3+8 並行):

- Agent A: US3 CloudProvider trait化 (T023-T030)
- Agent B: US4 auth_disabled廃止 (T031-T036)
- Agent C: US5 RequestResponseRecordファクトリ (T037-T042)
- Agent D: US8 Dashboard API分割 (T066-T075)

**Wave 2** (Phase 4):

- Agent A: Repository trait定義 + DB migration (T043-T044, T050-T053)
- Agent B-D: 各db/*.rsのRepository実装 (T045-T049) ※並列

**Wave 3** (Phase 5+6+7 並行):

- Agent A: バランサー分割 (T054-T056)
- Agent B: エントリポイント分割 (T057-T060)
- Agent C: テスト基盤 (T061-T065)
- Agent D: Dashboard Playground共通基盤 (T076-T083)

**Wave 4** (Phase 9):

- 全員: 品質チェック + ドキュメント (T084-T092)
