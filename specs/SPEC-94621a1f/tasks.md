# タスク一覧: ノード自己登録システム

**機能ID**: `SPEC-94621a1f`
**ステータス**: ✅ 更新済み

## 既存実装（完了）

- [x] Router: `POST /v0/nodes` でノード登録（GPU必須バリデーション、到達性チェック）
- [x] Router: `GET /v0/nodes` でノード一覧
- [x] Router: `POST /v0/health` でヘルスチェック受信（`X-Node-Token` 認証）
- [x] Token: 登録時に `node_token` を発行しDBに保存（以降のノード通信に必須）
- [x] Registry: ノード状態をDBと同期し、起動時にロード
- [x] Node: 定期的に `/v0/health` を送信して状態・メトリクスを更新
- [x] Tests: 主要フローのテストを追加

## 追加実装（承認フロー）

- [x] Router: NodeStatus に `pending` を追加し、登録時は常に pending にする
- [x] Router: `POST /v0/nodes/:id/approve`（管理者のみ）を追加
- [x] Registry: pending から approve で `registering` / `online` に遷移（ready_models で判定）
- [x] Router: pending 中のハートビートは受理しつつ状態遷移は抑止
- [x] Dashboard: pending 表示と承認アクション追加
- [x] Tests: 承認フローの TDD 追加（contract/integration/E2E）
- [x] Docs: `spec.md`, `quickstart.md`, `contracts` 更新

## 参照実装

- Router: `router/src/api/nodes.rs`, `router/src/api/health.rs`
- Node: `node/src/api/router_client.cpp`
- Protocol: `common/src/protocol.rs`

---

## Phase 2: SQLite移行 (2025-12-20)

### 背景

- spec.mdのFR-004で「ストレージ: SQLite」と明記されている
- 現在の実装はJSONファイル（`nodes.json`）を使用しており、Specと不整合
- 認証システム（users, api_keys, node_tokens）と同じDBに統合する

### タスク

- [x] T050 [P] `router/migrations/001_init.sql` にマイグレーション統合
  - ✅ nodesテーブル定義（line 51-73）
  - ✅ node_gpu_devicesテーブル定義（line 80-86）
  - ✅ node_loaded_modelsテーブル定義（line 91-96）
  - ✅ node_supported_runtimesテーブル定義（line 109-113）
  - 注: 003_nodes.sqlではなく001_init.sqlに統合

- [x] T051 `router/src/db/nodes.rs` SQLite対応テスト作成 (RED)
  - ✅ test_save_and_load_node()
  - ✅ test_load_nodes()
  - ✅ test_delete_node()
  - ✅ test_update_node()
  - 4テスト合格

- [x] T052 `router/src/db/nodes.rs` SQLite実装 (GREEN)
  - ✅ NodeStorage構造体（SqlitePool使用）
  - ✅ save_node() - UPSERT処理
  - ✅ load_nodes() - 全ノード読み込み
  - ✅ delete_node() - 削除処理
  - ✅ 関連テーブル（gpu_devices, loaded_models, tags, runtimes）処理

- [x] T053 JSON→SQLite移行ロジック
  - ✅ 新規インストールはSQLiteのみ使用
  - ✅ import_nodes_from_json() スタブ実装
  - ⚠️ レガシー移行は将来対応（現時点で必要なし）

- [x] T054 `router/src/registry/mod.rs` DB使用に変更
  - ✅ NodeStorage使用（line 7, 25）
  - ✅ load_nodes()でDB読み込み（line 59）
  - ✅ save_node()でDB書き込み（line 160）

- [x] T055 品質チェック
  - ✅ cargo test -p llm-router --lib -- db::nodes: 4テスト合格
