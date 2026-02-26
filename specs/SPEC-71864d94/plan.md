# 実装計画: llmlb全体的リファクタリング（メジャーバージョンアップ）

**機能ID**: `SPEC-71864d94` | **日付**: 2026-02-22 | **仕様**: [spec.md](spec.md)
**入力**: `/specs/SPEC-71864d94/spec.md` の機能仕様

## 概要

llmlbコードベース全体のリファクタリング。9つのPhaseで構成され、
型基盤→エラー基盤→API層→DB層→バランサー→エントリポイント→テスト基盤→Dashboard→仕上げ
の順序で実施する。破壊的変更を許容するメジャーバージョンアップ（v5.0.0）。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+ (バックエンド), TypeScript 5.x (Dashboard)
**主要依存関係**: axum, sqlx, reqwest, tokio, serde (Rust) / React, TanStack Query, Tailwind CSS (TS)
**ストレージ**: SQLite (sqlx + migrations)
**テスト**: cargo test (265テスト), Playwright (E2E)
**対象プラットフォーム**: macOS/Linux サーバー
**プロジェクトタイプ**: Rust APIサーバー + 組み込みSPA Dashboard
**パフォーマンス目標**: リファクタリング前後でレスポンスレイテンシの劣化なし
**制約**: 全265テスト通過必須、品質チェック（fmt/clippy/test/markdownlint）ゼロエラー
**スケール/スコープ**: Rust 36,675行 + Dashboard TSX

## 憲章チェック

| ゲート | 判定 | 備考 |
|--------|------|------|
| I. Router-Nodeアーキテクチャ | 合格 | Node→Endpoint命名統一は憲章「明確な責任分離」に合致 |
| III. テストファースト | 合格 | 各PhaseでTDD Red-Green-Refactorを厳守 |
| V. シンプルさと開発者体験 | 合格 | 型の一元管理、エラー統一は複雑さの削減 |
| VII. 可観測性 | 合格 | ログ/トレーシングは変更なし |
| VIII. 認証・アクセス制御 | 合格 | auth_disabled廃止は憲章「認証スキップ禁止」に完全合致 |
| IX. バージョニング | 合格 | feat!: でメジャーバージョンアップ |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-71864d94/
├── spec.md              # 機能仕様
├── plan.md              # この実装計画
└── tasks.md             # タスク分解（/speckit.tasksで生成）
```

### ソースコード変更対象

```text
llmlb/src/
├── main.rs              → 200行以内に縮小（server.rs/bootstrap.rsに分離）
├── server.rs            → 新規: run_server()
├── bootstrap.rs         → 新規: 初期化ロジック
├── lib.rs               → AppState維持
├── config.rs            → auth_disabled関連削除
├── api/
│   ├── mod.rs           → auth_disabledパターン廃止、ヘルパー関数導入
│   ├── error.rs         → AppError一本化（変更なし、他が統一）
│   ├── openai.rs        → 500行以内に縮小
│   ├── cloud_proxy.rs   → 新規: CloudProvider trait + 実装
│   ├── openai_util.rs   → 新規: ペイロードサニタイズ・変換ユーティリティ
│   ├── models.rs        → 重複AppError削除、HF関連整理
│   ├── endpoints.rs     → インラインErrorResponse→AppError統一
│   ├── dashboard.rs     → 同上
│   ├── auth.rs          → 同上
│   ├── audio.rs         → 同上
│   ├── images.rs        → 同上
│   ├── responses.rs     → 同上
│   └── cloud_models.rs  → 同上
├── common/
│   ├── protocol.rs      → RequestResponseRecordファクトリ + endpoint_リネーム
│   └── error.rs         → 維持
│   (types.rs削除 → types/へ移動)
├── types/
│   ├── mod.rs           → re-export
│   ├── endpoint.rs      → 既存 + GpuDevice統一版
│   ├── health.rs        → 新規: HealthMetrics(endpoint_id), Request, RequestStatus
│   ├── model.rs         → 新規: ModelType, RuntimeType, ModelCapability等
│   └── media.rs         → 新規: AudioFormat, ImageSize等
├── db/
│   ├── mod.rs           → Repository trait re-export
│   ├── traits.rs        → 新規: Repository trait定義
│   ├── endpoints.rs     → impl EndpointRepository for SqlitePool
│   ├── request_history.rs → impl RequestHistoryRepository
│   ├── models.rs        → impl ModelRepository
│   ├── users.rs         → impl UserRepository
│   ├── api_keys.rs      → impl ApiKeyRepository
│   ├── invitations.rs   → impl InvitationRepository
│   ├── endpoint_daily_stats.rs → impl EndpointDailyStatsRepository
│   ├── download_tasks.rs → impl DownloadTaskRepository
│   └── test_utils.rs    → TestAppStateBuilder, test_db_pool()
├── balancer/
│   ├── mod.rs           → LoadManager本体のみ
│   ├── types.rs         → 新規: RequestOutcome, WaitResult等
│   └── lease.rs         → 新規: RequestLease
├── auth/
│   ├── middleware.rs     → inject_dummy_admin_claims削除
│   └── bootstrap.rs     → debug_assertions対応
├── registry/
├── health/
├── detection/
├── events/
├── sync/
├── metrics/
├── metadata/
├── lock/
├── token/
├── update/
├── gui/
├── cli/
└── web/
    └── dashboard/src/
        ├── lib/
        │   ├── api/           → 新規ディレクトリ
        │   │   ├── client.ts  → fetchWithAuth, ApiError
        │   │   ├── auth.ts    → authApi + 関連型
        │   │   ├── dashboard.ts → dashboardApi
        │   │   ├── endpoints.ts → endpointsApi
        │   │   ├── models.ts  → modelsApi
        │   │   ├── chat.ts    → chatApi
        │   │   ├── system.ts  → systemApi
        │   │   ├── api-keys.ts → apiKeysApi
        │   │   ├── invitations.ts → invitationsApi
        │   │   ├── users.ts   → usersApi
        │   │   ├── types.ts   → 共通型
        │   │   └── index.ts   → re-export
        │   └── api.ts         → 削除（分割先へ）
        ├── components/
        │   └── playground/    → 新規ディレクトリ
        │       ├── PlaygroundBase.tsx → 共通基盤
        │       ├── MessageList.tsx    → メッセージ表示
        │       ├── ChatForm.tsx       → 入力フォーム
        │       └── types.ts          → Message, MessageAttachment
        ├── hooks/
        │   └── usePlayground.ts → ストリーミング・状態管理フック
        └── pages/
            ├── LoadBalancerPlayground.tsx → 共通基盤利用に書き換え
            └── EndpointPlayground.tsx    → 同上
```

### DB マイグレーション（新規追加）

```text
llmlb/migrations/
└── 017_rename_node_to_endpoint.sql  → node_→endpoint_カラムリネーム
```

## Phase別実装計画

### Phase 1: 型基盤 (FR-001〜FR-004)

**目的**: types/モジュールに全ドメイン型を集約し、レガシー命名を一掃する。

**変更対象ファイル**:

- `common/types.rs` → 削除（内容をtypes/配下に分散）
- `types/mod.rs` → サブモジュール宣言追加
- `types/health.rs` → 新規: HealthMetrics, Request, RequestStatus（endpoint_idにリネーム済み）
- `types/model.rs` → 新規: ModelType, RuntimeType, ModelCapability, ModelCapabilities, SyncState, SyncProgress
- `types/media.rs` → 新規: AudioFormat, ImageSize, ImageQuality, ImageStyle, ImageResponseFormat
- `types/endpoint.rs` → GpuDeviceInfo統合（GpuDeviceに統一、countフィールドはVec長で代替）
- `common/mod.rs` → types.rsのre-export削除
- `lib.rs` → use文更新

**依存関係への影響**: 全ファイルのuse文更新（grep: `use crate::common::types::`）

**テスト戦略**: 型移動はコンパイルエラーベースで検証。全265テスト通過で完了。

---

### Phase 2: エラー基盤 (FR-005〜FR-007)

**目的**: AppErrorを一本化し、全APIハンドラーのエラー返却を統一する。

**変更対象ファイル**:

- `api/models.rs` L962-996 → 重複AppError定義を削除、`use super::error::AppError;`に変更
- `api/endpoints.rs` → 52箇所のインラインErrorResponseをAppError返却に置換
- `api/dashboard.rs` → 同上
- `api/audio.rs` → 同上
- `api/images.rs` → 同上
- `api/responses.rs` → 同上
- `api/cloud_models.rs` → 同上
- `common/error.rs` → 必要に応じてLbErrorバリアント追加

**テスト戦略**:

- RED: IPアドレスがエラーレスポンスに含まれないことを検証するテスト追加
- GREEN: AppError統一で自動的にexternal_message()が適用される
- 全既存テスト通過

---

### Phase 3: API層リファクタリング (FR-008〜FR-012)

**3a. CloudProvider trait化**

- `api/cloud_proxy.rs` 新規作成
  - `CloudProvider` trait: `fn api_base_url()`, `fn auth_header()`,
    `fn transform_request()`, `fn transform_response()`,
    `fn transform_stream_chunk()`, `fn map_error()`
  - `OpenAiProvider`, `GoogleProvider`, `AnthropicProvider` 実装
  - `proxy_cloud_provider<P: CloudProvider>()` ジェネリック関数
- `api/openai_util.rs` 新規作成
  - `sanitize_openai_payload_for_history()`
  - `map_openai_messages_to_google_contents()`
  - `map_openai_messages_to_anthropic()`
  - エラーレスポンスヘルパー群

**3b. auth_disabled廃止**

- `config.rs` → `is_auth_disabled()` 削除
- `auth/middleware.rs` → `inject_dummy_admin_claims_with_state` 削除
- `auth/bootstrap.rs` → `#[cfg(debug_assertions)]`で`admin`/`test`ユーザーを
  DB初期化時に自動作成。`sk_debug` APIキーも同様。
- `api/mod.rs` → 16箇所の`if auth_disabled`分岐を削除。
  `apply_auth_layer(router, state, permission_config)` ヘルパー関数で
  認証レイヤー適用を宣言的に記述。

**3c. RequestResponseRecordファクトリ**

- `common/protocol.rs` に追加:
  - `RequestResponseRecord::error(model, request_type, request_body, message)` → エラー記録
  - `RequestResponseRecord::success(endpoint, model, request_type, ...)` → 成功記録
  - フィールドリネーム: `node_id`→`endpoint_id`, `node_machine_name`→`endpoint_name`,
    `node_ip`→`endpoint_ip`

---

### Phase 4: DB層 (FR-013〜FR-015)

**4a. Repository traitパターン導入**

- `db/traits.rs` 新規作成:

```text
EndpointRepository: CRUD + status更新 + ヘルスチェック
RequestHistoryRepository: 保存 + クエリ + 集計
ModelRepository: CRUD + 検索
UserRepository: CRUD + 認証
ApiKeyRepository: CRUD + 検証
InvitationRepository: CRUD + 使用
EndpointDailyStatsRepository: 記録 + クエリ
DownloadTaskRepository: CRUD + 進捗更新
```

- 各db/*.rsファイル → `impl XxxRepository for SqlitePool`

**4b. DB migration**

- `017_rename_node_to_endpoint.sql`:
  - `request_history` テーブル: `node_id`→`endpoint_id`,
    `node_machine_name`→`endpoint_name`, `node_ip`→`endpoint_ip`
  - SQLiteはALTER TABLE RENAME COLUMNをサポート（v3.25.0+）

**4c. レガシー命名一掃**

- Rust全ファイルのnode_変数・フィールド → endpoint_
- APIレスポンスJSON: node_id → endpoint_id（必要な場合のみ）
- `/nodes/{node_id}/logs` → `/endpoints/{id}/logs`

---

### Phase 5: バランサー (FR-016〜FR-017)

**目的**: balancer/mod.rsをファイル分割し可読性向上。

- `balancer/types.rs` 新規: RequestOutcome, WaitResult, QueueWaiterGuard,
  AdmissionDecision, ModelTpsState, EndpointTpsSummary, EndpointLoadState,
  EndpointLoadSnapshot, SystemSummary（L39-767相当）
- `balancer/lease.rs` 新規: RequestLease（L898-989相当）
- `balancer/mod.rs` → LoadManager本体 + ヘルパー関数のみ

**テスト戦略**: 純粋なファイル分割のためコンパイル通過 + 全テスト通過で検証。

---

### Phase 6: エントリポイント (FR-018〜FR-019)

- `main.rs` L255-429 → チェックサム互換コード全削除
  （メジャーバージョンアップのため旧データベースからの直接移行は非サポート）
- `server.rs` 新規: `pub async fn run_server(config, ...)` （L561-732相当）
- `bootstrap.rs` 新規: DB初期化、レジストリ初期化、ヘルスチェッカー起動等
- `main.rs` → CLI解析 + server/bootstrap呼び出しのみ（200行以内目標）

---

### Phase 7: テスト基盤 (FR-020〜FR-022)

- `db/test_utils.rs` 拡張:
  - `pub async fn test_db_pool() -> SqlitePool` （インメモリDB + migrate）
  - `TestAppStateBuilder::new()` → `.with_endpoints(vec![...])` `.with_auth(...)` `.build()`
- 全テストファイルの`create_local_state()` → `TestAppStateBuilder`に置換
- `api/openai.rs`と`api/responses.rs`の重複`create_local_state()`を削除

---

### Phase 8: Dashboard (FR-023〜FR-025)

**8a. API分割**

- `lib/api.ts` → `lib/api/` ディレクトリに分割
  - `client.ts`: fetchWithAuth, ApiError, BASE_URL
  - `auth.ts`: authApi + LoginRequest/LoginResponse型
  - `dashboard.ts`: dashboardApi + DashboardStats型
  - `endpoints.ts`: endpointsApi + Endpoint/EndpointType型
  - `models.ts`: modelsApi + Model/ModelInfo型
  - `chat.ts`: chatApi + ChatMessage型
  - `system.ts`: systemApi + SystemInfo型
  - `api-keys.ts`: apiKeysApi + ApiKey型
  - `invitations.ts`: invitationsApi + Invitation型
  - `users.ts`: usersApi + User型
  - `types.ts`: 共通型（PaginatedResponse等）
  - `index.ts`: 全API re-export

**8b. Playground共通基盤**

- `components/playground/types.ts`: Message, MessageAttachment
- `components/playground/MessageList.tsx`: メッセージ表示コンポーネント
- `components/playground/ChatForm.tsx`: 入力フォーム + 添付ファイル
- `components/playground/PlaygroundBase.tsx`: レイアウト + 状態管理
- `hooks/usePlayground.ts`: ストリーミング送信・受信ロジック
- `LoadBalancerPlayground.tsx` → PlaygroundBase + 差分のみ
- `EndpointPlayground.tsx` → 同上

---

### Phase 9: 仕上げ (FR-026〜FR-027)

- `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` 全通過
- `pnpm --filter @llm/dashboard build` 成功
- markdownlint全通過
- `check-tasks.sh` / `check-commits.sh` 通過
- ドキュメント更新:
  - `docs/architecture.md`: Node→Endpoint用語統一
  - `README.md` / `README.ja.md`: 破壊的変更に関する記載
  - CHANGELOG: v5.0.0のBreaking Changes一覧

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| CloudProvider trait | プロバイダ間の共通化に必要 | 変換関数のみ抽出では800行×3のプロキシ関数重複が残る |
| Repository trait | テスト時のモック差し替えに必要 | 直接SQLxでは全テストがインメモリDB必須のまま |
| DB migration追加 | node_→endpoint_カラムリネームに必要 | sqlx属性のrenameだけではDB上の命名が不一致のまま |

## Phase間依存関係

```text
Phase 1 (型基盤)
  ↓
Phase 2 (エラー基盤)
  ↓
Phase 3 (API層)  ←→  Phase 8 (Dashboard) ※並行可能
  ↓
Phase 4 (DB層)
  ↓
Phase 5 (バランサー) ※Phase 4完了後並行可能
Phase 6 (エントリポイント) ※Phase 4完了後並行可能
Phase 7 (テスト基盤) ※Phase 4完了後並行可能
  ↓
Phase 9 (仕上げ) ← 全Phase完了後
```

## リスクと軽減策

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| 型移動でのコンパイルエラー連鎖 | Phase 1が長期化 | grep + sed で一括use文更新 |
| CloudProvider traitの抽象化漏れ | 新プロバイダ追加時に修正必要 | 3プロバイダの差異を網羅的に洗い出してからtrait設計 |
| DB migration失敗 | 既存データ破損 | テスト環境で事前検証、バックアップ手順明記 |
| auth_disabled廃止でCI破損 | 全CIが失敗 | debug_assertionsでの認証情報自動生成を先に実装 |
| Dashboard API分割でimport漏れ | ビルド失敗 | TypeScript strict modeで即座に検出 |
