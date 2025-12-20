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
- [x] Tests: 承認フローの TDD 追加（unit/integration）
- [x] Docs: `spec.md`, `quickstart.md`, `contracts` 更新

## 参照実装

- Router: `router/src/api/nodes.rs`, `router/src/api/health.rs`
- Node: `node/src/api/router_client.cpp`
- Protocol: `common/src/protocol.rs`
