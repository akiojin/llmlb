# タスク: モデル自動解決機能

**機能ID**: `SPEC-48678000`
**ステータス**: Phase 3.5完了（実装・統合・ドキュメント更新）
**入力**: `/specs/SPEC-48678000/` の設計ドキュメント

## 技術スタック

- **Node**: C++17 (llama.cpp, httplib)
- **Router**: Rust 1.75+ (reqwest)
- **Storage**: ファイルシステム (`~/.llm-router/models/`)
- **Tests**: Google Test, cargo test

## Phase 3.1: セットアップ・クリーンアップ

- [x] T001 `node/src/` から auto_repair 関連コードを特定・削除
  - ✅ auto_repair関連コードは存在しない（SPEC-3df1b977で廃止済み、未実装）
- [x] T002 auto_repair 関連の環境変数・設定を削除
  - ✅ 環境変数・設定は存在しない
- [x] T003 関連するテストコードを削除
  - ✅ テストコードは存在しない
- [x] T003.1 未使用のhf_client（ダミー実装）を削除
  - FR-006（HuggingFace直接ダウンロード禁止）対応

## Phase 3.2: テストファースト (TDD RED)

- [x] T004 [P] `node/tests/unit/model_resolver_test.cpp` に共有パス参照の contract test
  - ✅ ResolveFromSharedPathWhenNotLocal (FR-002)
  - ✅ SharedPathDoesNotCopyToLocal (FR-002)
  - ✅ LocalPathTakesPriority (FR-001)
- [x] T005 [P] `node/tests/unit/model_resolver_test.cpp` にルーターAPI経由ダウンロードの contract test
  - 🔴 DownloadFromRouterAPIWhenSharedInaccessible (FR-003) - RED: router_attempted未実装
  - 🔴 DownloadedModelSavedToLocal (FR-004) - RED: downloadFromRouter未実装
  - 🔴 SharedPathInaccessibleTriggersRouterFallback (FR-003) - RED
- [x] T006 [P] `node/tests/unit/model_resolver_test.cpp` にモデル不在時のエラーハンドリング contract test
  - ✅ ReturnErrorWhenModelNotFound (FR-005)
  - ✅ ErrorResponseWithinOneSecond (成功基準3)
  - ✅ ClearErrorMessageWhenModelNotFoundAnywhere (US3)
- [x] T007 `node/tests/unit/model_resolver_test.cpp` に統合テスト: 解決フロー全体
  - ✅ FullFallbackFlow (local -> shared -> error)
  - ✅ HuggingFaceDirectDownloadProhibited (FR-006)
  - ✅ NoAutoRepairFunctionality (FR-007/成功基準4)
- [x] T007.1 エッジケーステスト追加
  - 🔴 NetworkDisconnectionToSharedPathTriggersRouterFallback - RED
  - 🔴 IncompleteDownloadIsRetried - RED
  - 🔴 PreventDuplicateDownloads - RED: hasDownloadLock未実装
- [x] T007.2 ユーザーストーリー受け入れシナリオテスト
  - ✅ UpdatedSharedPathModelIsUsed (US1-シナリオ2)
- [x] T007.3 技術制約テスト追加
  - 🔴 OnlyGGUFFormatSupported - RED: GGUF検証未実装
  - 🔴 RouterDownloadValidatesGGUFFormat - RED: GGUF magic bytes検証未実装
- [x] T007.4 Clarificationsテスト追加
  - 🔴 RouterDownloadHasTimeout - RED: タイムアウト設定未実装
  - 🔴 ConcurrentDownloadLimit - RED: 同時ダウンロード制限未実装

## Phase 3.3: コア実装

- [x] T008 `node/src/model_resolver.cpp` にモデル解決クラスを実装
  - ローカルキャッシュ確認
  - 共有パス直接参照（コピーなし）
  - ルーターAPI経由ダウンロード
  - エラーハンドリング

- [x] T009 `node/src/model_resolver.cpp` に共有パス参照ロジック
  - NFSパスアクセス確認
  - ファイル存在チェック
  - 直接パス返却（コピーしない）

- [x] T010 `node/src/model_resolver.cpp` にルーターAPI経由ダウンロード
  - `GET /v0/models/blob/:model_name` エンドポイント呼び出し
  - ローカルへの保存処理
  - 進捗表示（オプション）

- [x] T011 `node/src/model_resolver.cpp` にエラーハンドリング
  - モデル未発見エラー
  - ネットワークエラー
  - ディスク容量不足エラー

## Phase 3.4: 統合

- [x] T012 既存の推論フローに ModelResolver を統合
- [x] T013 重複ダウンロード防止（ミューテックス）
- [x] T014 設定ファイルから共有パス・ルーターURL読み込み

## Phase 3.5: 仕上げ

- [x] T015 [P] `node/tests/` にユニットテスト追加
  - パス検証ロジック（既存テストでカバー）
  - エラーメッセージ生成（既存テストでカバー）
- [x] T016 パフォーマンステスト: エラー応答 < 1秒
- [x] T017 ドキュメント更新: モデル解決フローの説明

## 依存関係

```text
T001, T002, T003 → T004-T007 (クリーンアップ → テスト)
T004-T007 → T008-T011 (テスト → 実装)
T008 → T009, T010, T011 (基盤 → 詳細実装)
T008-T011 → T012-T014 (実装 → 統合)
T012-T014 → T015-T017 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T004: node/tests/ 共有パス参照 contract test
Task T005: node/tests/ ルーターAPI経由ダウンロード contract test
Task T006: node/tests/ モデル不在時エラー contract test
```

## 検証チェックリスト

- [x] auto_repair 関連コードが完全に削除されている (T001-T003)
- [x] 共有パスからの直接参照でコピーが発生しない (テスト: SharedPathDoesNotCopyToLocal)
- [x] ルーターAPI経由ダウンロードが正常に動作する (Phase 3.3で実装済み)
- [x] モデル不在時に1秒以内にエラーが返る (テスト: ErrorResponseWithinOneSecond)
- [x] Hugging Face への直接ダウンロードが禁止されている (テスト: HuggingFaceDirectDownloadProhibited)
- [x] すべてのテストが実装より先にある (TDD RED完了)
