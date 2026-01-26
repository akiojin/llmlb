# タスク一覧: ノード自己登録システム

**機能ID**: `SPEC-94621a1f`
**ステータス**: ✅ 更新済み

## 既存実装（完了）

- [x] Load Balancer: `POST /v0/nodes` でノード登録（GPU必須バリデーション、到達性チェック）
- [x] Load Balancer: `GET /v0/nodes` でノード一覧
- [x] Load Balancer: `POST /v0/health` でヘルスチェック受信（`X-Node-Token` 認証）
- [x] Token: 登録時に `runtime_token` を発行しDBに保存（以降のノード通信に必須）
- [x] Registry: ノード状態をDBと同期し、起動時にロード
- [x] Node: 定期的に `/v0/health` を送信して状態・メトリクスを更新
- [x] Tests: 主要フローのテストを追加

## 追加実装（承認フロー）

- [x] Load Balancer: NodeStatus に `pending` を追加し、登録時は常に pending にする
- [x] Load Balancer: `POST /v0/nodes/:id/approve`（管理者のみ）を追加
- [x] Registry: pending から approve で `registering` / `online` に遷移（ready_models で判定）
- [x] Load Balancer: pending 中のハートビートは受理しつつ状態遷移は抑止
- [x] Dashboard: pending 表示と承認アクション追加
- [x] Tests: 承認フローの TDD 追加（contract/integration/E2E）
- [x] Docs: `spec.md`, `quickstart.md`, `contracts` 更新

## 参照実装

- Load Balancer: `llmlb/src/api/nodes.rs`, `llmlb/src/api/health.rs`
- Node: `node/src/api/router_client.cpp`
- Protocol: `common/src/protocol.rs`

---

## Phase 2: SQLite移行 (2025-12-20)

### 背景

- spec.mdのFR-004で「ストレージ: SQLite」と明記されている
- 現在の実装はJSONファイル（`nodes.json`）を使用しており、Specと不整合
- 認証システム（users, api_keys, runtime_tokens）と同じDBに統合する

### タスク

- [x] T050 [P] `llmlb/migrations/003_nodes.sql` マイグレーション作成
  - nodesテーブル定義（30+フィールド）
  - node_gpu_devicesテーブル定義
  - node_loaded_modelsテーブル定義
  - node_supported_runtimesテーブル定義
  - 依存: なし
  - 実装メモ: `llmlb/migrations/001_init.sql` にノード関連テーブルが定義済みのため追加マイグレーション不要（確認済み）

- [x] T051 `llmlb/src/db/mod.rs` SQLite対応テスト作成 (RED)
  - SQLite版のsave_node()テスト
  - SQLite版のload_nodes()テスト
  - SQLite版のdelete_node()テスト
  - 関連テーブル（gpu_devices, loaded_models）のテスト
  - 依存: T050

- [x] T052 `llmlb/src/db/mod.rs` SQLite実装 (GREEN)
  - ノードCRUDをSQLite使用に書き換え
  - 関連テーブルへのINSERT/DELETE処理
  - 既存のインターフェースを維持
  - 依存: T051

- [x] T053 JSON→SQLite移行ロジック実装
  - 起動時に既存nodes.jsonを検出
  - SQLiteにデータをインポート
  - JSONファイルを.migratedにリネーム
  - 依存: T052

- [x] T054 `llmlb/src/registry.rs` DB使用に変更
  - NodeRegistryがSQLiteを直接使用するよう変更
  - インメモリキャッシュとDBの同期
  - 依存: T052

- [x] T055 品質チェック＆コミット
  - `cargo fmt --check` 合格
  - `cargo clippy -- -D warnings` 合格
  - `cargo test` 全テスト合格
  - markdownlint 合格
  - 依存: T054
  - 実施メモ: 本マージ作業で `make quality-checks` を再実行し合格を確認
