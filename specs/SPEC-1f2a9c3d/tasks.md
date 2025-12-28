# タスク: Node / Router Log Retrieval API

**機能ID**: `SPEC-1f2a9c3d`
**ステータス**: ✅ 実装完了

## 実装タスク

- [x] **T001** Node: `/v0/logs?tail=N` エンドポイントを実装（デフォ200、上限1000、ログなし→200 `entries: []`）
  - ✅ `router/src/api/logs.rs:67-109` で実装済み
  - ✅ `DEFAULT_LIMIT=200`, `MAX_LIMIT=1000` 定義済み
- [x] **T002** Node: ログファイルパスを設定可能にし、単体テストで tail 切り出しを検証
  - ✅ `common/src/log.rs` に `tail_json_logs` 実装
  - ✅ `router/src/api/logs.rs:142-271` にユニットテスト
- [x] **T003** Router: `/v0/nodes/:node_id/logs` プロキシを実装（timeout/非200→502）
  - ✅ `router/src/api/logs.rs:67-109` で実装済み
  - ✅ 10秒タイムアウト、エラーハンドリング実装
- [x] **T004** Contract test: 200/502 ケースをカバー（ノードスタブを用意）
  - ✅ `router/src/api/logs.rs:227-270` wiremock使用テスト
- [x] **T005** Dashboard: ログパネルが新APIでログを取得・表示するように差し替え（tail指定UI含む）
  - ✅ `router/src/web/dashboard/src/lib/api.ts` にAPI統合
- [x] **T006** E2E/Smoke: ダッシュボードからログが表示されることを手動またはスモークで確認
  - ✅ 統合テストでカバー

## 検証チェックリスト

- [x] `/v0/dashboard/logs/router` エンドポイント動作
- [x] `/v0/nodes/:node_id/logs` エンドポイント動作
- [x] ダッシュボードからログ表示
- [x] ユニットテスト合格
- [x] wiremockによる契約テスト合格

---
*実装完了後の遡及的文書化 - 2025-12-25*
