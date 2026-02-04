# タスク: 推論中ノードへの多重リクエストキューイング

**機能ID**: `SPEC-05098000` | **入力**: spec.md, plan.md
**ステータス**: 完了
**前提条件**: plan.md完了

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] T001 `llmlb/src/lib.rs` にキュー設定を追加し、`llmlb/src/main.rs` で環境変数から読み込む
- [x] T002 [P] `llmlb/src/api/dashboard.rs` と `llmlb/src/web/dashboard/src/lib/api.ts` の stats 型を拡張

## Phase 3.2: テストファースト (TDD)

### 3.2.1 Contract / Integration Tests

- [x] T003 `llmlb/tests/contract/queueing_test.rs` に以下のテストを追加
  - 単一ノードでの待機処理（キュー待機ヘッダ）
  - キュー満杯時の 429 + Retry-After
  - タイムアウト時の 504
  - 複数ノード時のアイドル優先ルーティング

### 3.2.2 Node Unit Tests

- [x] T004 `node/tests/unit/request_guard_test.cpp` に単発リクエスト制限のユニットテスト

### 3.2.3 Dashboard Tests

- [x] T005 `llmlb/src/api/nodes.rs` の summary テストに待機数フィールドを追加

## Phase 3.3: Load Balancer 実装

- [x] T006 `llmlb/src/balancer/mod.rs` に待機ガードと `wait_for_idle_node_with_timeout` を実装
- [x] T007 `llmlb/src/api/proxy.rs` にキュー選択ヘルパーを追加
- [x] T008 `llmlb/src/api/openai.rs` にキュー待機・429/504レスポンス・待機ヘッダを追加

## Phase 3.4: Node 実装

- [x] T009 `node/include/runtime/state.h` と `node/src/runtime/state.cpp` にアクティブ要求カウンタを追加
- [x] T010 `node/src/api/openai_endpoints.cpp` と `node/src/api/audio_endpoints.cpp` に単発制限を適用
- [x] T011 `node/src/api/router_client.cpp` で active_requests を送信

## Phase 3.5: Dashboard

- [x] T012 `llmlb/src/api/dashboard.rs` で待機数の集計を追加
- [x] T013 `llmlb/src/web/dashboard/src/components/dashboard/StatsCards.tsx` に待機数/処理中数を表示
- [x] T014 `llmlb/src/web/dashboard` をビルドし、`llmlb/src/web/static` を更新

## Phase 3.6: 仕上げ

- [x] T015 仕様に沿ってエラーメッセージとヘッダ名を整理
- [x] T016 追加したテスト・品質チェックをすべて実行

## Phase 3.7: 安定化（2026-02-04）

- [x] T017 `llmlb/tests/contract/test_proxy_completions.rs` の
queue overflow テストを安定化（in-flight 反映後に2本目を送信）
- [x] T018 `llmlb/src/balancer/mod.rs` に
テスト向けの状態可視化（busy判定/待機数の待ち合わせヘルパー）を追加
- [ ] T019 `llmlb/tests/contract/queueing_test.rs` に
キュー満杯の再現用シナリオを追加（Notifyで先行リクエストを保持）

## 依存関係

```text
T001 → T006-T008
T002 → T012-T013
T003 → T006-T008
T004 → T009-T010
T005 → T012
T006-T008 → T012-T015
T009-T011 → T015
T013 → T014
T014 → T016
```

## 注意事項

- キュー待機中にリクエストが破棄された場合、待機数カウンタを減算する
- テストは Contract → Integration → Unit の順で追加する
- 変更後は `llmlb/src/web/static` を再生成する
