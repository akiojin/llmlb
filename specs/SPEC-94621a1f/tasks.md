# タスク一覧: ノード自己登録システム

**機能ID**: `SPEC-94621a1f`
**ステータス**: ✅ 実装済み

## 実装内容（完了）

- [x] Router: `POST /api/nodes` でノード登録（GPU必須バリデーション、到達性チェック）
- [x] Router: `GET /api/nodes` でノード一覧
- [x] Router: `POST /api/health` でヘルスチェック受信（`X-Agent-Token` 認証）
- [x] Token: 登録時に `agent_token` を発行しDBに保存（以降のノード通信に必須）
- [x] Registry: ノード状態をDBと同期し、起動時にロード
- [x] Node: 定期的に `/api/health` を送信して状態・メトリクスを更新
- [x] Tests: 主要フローのテストを追加

## 参照実装

- Router: `router/src/api/nodes.rs`, `router/src/api/health.rs`
- Node: `node/src/api/router_client.cpp`
- Protocol: `common/src/protocol.rs`
