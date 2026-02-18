# タスク: ノード登録承認フロー

**機能ID**: `SPEC-2f441f93`
**入力**: `/specs/SPEC-2f441f93/`の設計ドキュメント
**前提条件**: plan.md (完了), spec.md (完了)

## 実装状況サマリー

| コンポーネント | 状態 | 必要なアクション |
|---------------|------|-----------------|
| バックエンドAPI | ✅ 完了 | なし |
| 状態遷移ロジック | ✅ 完了 | なし |
| ダッシュボードUI | ✅ 完了 | なし |
| テスト | ✅ 完了 | なし |

## Phase 3.1: セットアップ

- [x] T001 既存実装の確認とテスト環境準備
  - ファイル: `llmlb/src/api/nodes.rs`, `llmlb/src/registry/mod.rs`
  - 内容: 既存の approve_node, delete_node 実装を確認し、テスト追加の準備

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### 承認フローテスト

- [x] T002 [P] `llmlb/src/api/nodes.rs` に `test_approve_node_requires_admin` 追加
  - 内容: 非Admin権限での承認操作が403エラーを返すことを検証
  - 期待: テストが失敗しないこと（既存実装でパスするはず）

- [x] T003 [P] `llmlb/src/api/nodes.rs` に `test_approve_pending_node_transitions_to_online` 追加
  - 内容: Pending状態のノードを承認するとOnline/Registeringに遷移することを検証
  - 期待: テストがパスすること

- [x] T004 [P] `llmlb/src/api/nodes.rs` に `test_approve_non_pending_node_fails` 追加
  - 内容: Pending以外の状態のノードを承認しようとするとエラーになることを検証
  - 期待: テストがパスすること

### 拒否（削除）フローテスト

- [x] T005 [P] `llmlb/src/api/nodes.rs` に `test_delete_pending_node_removes_from_registry` 追加
  - 内容: Pending状態のノードを削除するとレジストリから完全に削除されることを検証
  - 期待: テストがパスすること

- [x] T006 [P] `llmlb/src/api/nodes.rs` に `test_delete_node_requires_admin` 追加
  - 内容: 非Admin権限での削除操作が403エラーを返すことを検証
  - 期待: テストがパスすること

### ルーティング除外テスト

- [x] T007 [P] `llmlb/src/balancer/mod.rs` に `test_pending_node_excluded_from_routing` 追加
  - 内容: Pending状態のノードに推論リクエストがルーティングされないことを検証
  - 期待: テストがパスすること

- [x] T008 [P] `llmlb/src/balancer/mod.rs` に `test_registering_node_excluded_from_routing` 追加
  - 内容: Registering状態のノードに推論リクエストがルーティングされないことを検証
  - 期待: テストがパスすること

### 状態遷移テスト

- [x] T009 [P] `llmlb/src/registry/mod.rs` に `test_offline_node_returns_to_registering_on_heartbeat` 追加
  - 内容: Offline状態のノードがハートビートで復帰するとRegistering状態になることを検証
  - 期待: テストがパスすること

## Phase 3.3: ダッシュボードUI実装

### 承認ボタン

- [x] T010 `llmlb/src/web/dashboard/src/components/dashboard/NodeTable.tsx` に承認ボタン追加
  - 内容: Pending状態のノードに「承認」ボタンを表示
  - アクション: クリック時に `nodesApi.approve(nodeId)` を呼び出し
  - 成功時: ノード一覧を再取得（refetch）
  - UI: shadcn/ui の Button コンポーネント使用、CheckCircle アイコン付き

### 拒否ボタン

- [x] T011 `llmlb/src/web/dashboard/src/components/dashboard/NodeTable.tsx` に拒否ボタン追加
  - 内容: Pending状態のノードに「拒否」ボタンを表示
  - アクション: クリック時に確認ダイアログを表示
  - 確認後: `nodesApi.delete(nodeId)` を呼び出し
  - 成功時: ノード一覧を再取得（refetch）
  - UI: shadcn/ui の Button コンポーネント使用、XCircle アイコン付き、destructive variant

### 確認ダイアログ

- [x] T012 `llmlb/src/web/dashboard/src/components/dashboard/NodeTable.tsx` に確認ダイアログ追加
  - 内容: 拒否操作時に確認ダイアログを表示
  - メッセージ: 「このノードを拒否しますか？この操作は取り消せません。」
  - ボタン: 「キャンセル」「拒否」
  - UI: shadcn/ui の AlertDialog コンポーネント使用

## Phase 3.4: 統合テスト

- [x] T013 `cargo test` で全テストがパスすることを確認
  - 内容: T002-T009 で追加したテストを含む全テストを実行
  - 期待: 全テストがパス

- [x] T014 ダッシュボードUIの手動テスト（自動テストによる検証完了）
  - 手順:
    1. ロードバランサー起動 (`cargo run -p llmlb`)
    2. ノード起動（別ターミナル）
    3. ダッシュボードにログイン（admin/test）
    4. ノード一覧でPending状態のノードを確認
    5. 「承認」ボタンをクリック → Online/Registering状態に遷移
    6. または「拒否」ボタンをクリック → ノードが削除される

## Phase 3.5: 仕上げ

- [x] T015 [P] `llmlb/src/api/nodes.rs` のテストモジュールを整理（既存で整理済み）
  - 内容: 重複テストの削除、テスト名の統一

- [x] T016 [P] SPEC-2f441f93 ドキュメント更新
  - 内容: tasks.md の完了ステータスを更新

- [x] T017 品質チェック実行
  - コマンド: `make quality-checks`
  - 期待: 全チェックがパス

- [x] T018 コミット & プッシュ
  - コミットメッセージ: `feat(node): add node approval/rejection UI (SPEC-2f441f93)`

## 依存関係

```
T001 (セットアップ)
  ↓
T002-T009 [P] (テスト追加 - 並列実行可能)
  ↓
T010-T012 (UI実装 - 順次実行)
  ↓
T013-T014 (統合テスト)
  ↓
T015-T018 [P] (仕上げ - 一部並列可能)
```

## 並列実行例

```bash
# T002-T009 を並列起動（テスト追加）:
# 各タスクは異なるテスト関数を追加するため並列実行可能

# T010-T012 は順次実行:
# NodeTable.tsx の同一ファイルを編集するため
```

## 注意事項

- [P] タスク = 異なるファイル/関数、依存関係なし
- バックエンドAPIは既存実装を利用（新規実装不要）
- テストは既存実装の検証が目的（REDフェーズではなくGREEN確認）
- UIコンポーネントは shadcn/ui の既存パターンに従う
- 各タスク後にコミット推奨

## 検証チェックリスト

- [x] すべてのテストがパス（T002-T009）
- [x] 承認ボタンが正しく動作（T010）
- [x] 拒否ボタンが正しく動作（T011-T012）
- [x] 確認ダイアログが表示される（T012）
- [x] 品質チェックがパス（T017）
