# タスク: モデル自動解決機能

**機能ID**: `SPEC-48678000`
**ステータス**: 仕様改定に伴い再計画（2025-12-30）
**入力**: `/specs/SPEC-48678000/` の設計ドキュメント

## 重要な変更 (Session 2025-12-30)

- 共有パス機能は**廃止**
- 対象モデルは`supported_models.json`に定義されたもののみ
- 解決フロー: ローカル → 外部ソース → ルータープロキシ

## 技術スタック

- **Node**: C++17 (llama.cpp, httplib)
- **Router**: Rust 1.75+ (reqwest)
- **Storage**: ファイルシステム (`~/.llm-router/models/`)
- **Tests**: Google Test, cargo test

## Phase 3.1: セットアップ・クリーンアップ

- [ ] T001 共有パス関連コードの削除（廃止機能）
  - 共有パス参照ロジックの削除
  - 関連設定・環境変数の削除
- [ ] T002 共有パス関連テストコードの削除
  - ResolveFromSharedPathWhenNotLocal
  - SharedPathDoesNotCopyToLocal
  - UpdatedSharedPathModelIsUsed
- [ ] T003 `supported_models.json`参照インターフェースの設計
  - SPEC-6cd7f960との連携方法確認

## Phase 3.2: テストファースト (TDD RED)

- [ ] T004 [P] `node/tests/unit/model_resolver_test.cpp` にローカル確認の contract test
  - 🔴 LocalPathTakesPriority (FR-001)
  - 🔴 LocalModelUsedWhenExists (FR-001)
- [ ] T005 [P] `node/tests/unit/model_resolver_test.cpp` に外部ソースダウンロードの contract test
  - 🔴 DownloadFromHuggingFaceWhenNotLocal (FR-002)
  - 🔴 DownloadedModelSavedToLocal (FR-004)
  - 🔴 ProgressNotificationDuringDownload (FR-008)
- [ ] T006 [P] `node/tests/unit/model_resolver_test.cpp` にプロキシ経由ダウンロードの contract test
  - 🔴 FallbackToRouterProxyWhenOriginFails (FR-003)
  - 🔴 ProxyDownloadSavedToLocal (FR-004)
- [ ] T007 [P] `node/tests/unit/model_resolver_test.cpp` にエラーハンドリング contract test
  - 🔴 ReturnErrorWhenModelNotInSupportedModels (FR-005)
  - 🔴 ErrorResponseWithinOneSecond (成功基準2)
  - 🔴 ClearErrorMessageForUnsupportedModel (US3)
- [ ] T008 `node/tests/unit/model_resolver_test.cpp` に重複ダウンロード防止テスト
  - 🔴 PreventDuplicateDownloads (FR-007)
  - 🔴 ConcurrentRequestsWaitForSingleDownload (FR-007)
- [ ] T009 統合テスト: 解決フロー全体
  - 🔴 FullFallbackFlow (local -> origin -> proxy -> error)
  - 🔴 SupportedModelsJsonValidation

## Phase 3.3: コア実装

- [ ] T010 `node/src/model_resolver.cpp` にモデル解決クラスを実装
  - ローカルキャッシュ確認
  - `supported_models.json`参照
  - 外部ソース/プロキシ経由ダウンロード
  - エラーハンドリング

- [ ] T011 `node/src/model_resolver.cpp` に`supported_models.json`参照ロジック
  - モデル定義の読み込み
  - 外部ソースURL取得
  - 未定義モデルのエラー生成

- [ ] T012 `node/src/model_resolver.cpp` に外部ソースダウンロード
  - Hugging Face等からのHTTPダウンロード
  - ローカルへの保存処理
  - 進捗通知（10%単位）

- [ ] T013 `node/src/model_resolver.cpp` にルータープロキシ経由ダウンロード
  - ルータープロキシ（`/v1/models/blob/:model_name`）の利用
  - ローカルへの保存処理

- [ ] T014 `node/src/model_resolver.cpp` に重複ダウンロード防止
  - ダウンロードロック機構
  - 同時リクエストの待機処理

- [ ] T015 `node/src/model_resolver.cpp` にエラーハンドリング
  - モデル未サポートエラー
  - ネットワークエラー
  - ディスク容量不足エラー

## Phase 3.4: 統合

- [ ] T016 既存の推論フローに ModelResolver を統合
- [ ] T017 設定ファイルからルーターURL読み込み

## Phase 3.5: 仕上げ

- [ ] T018 [P] ユニットテスト追加
  - パス検証ロジック
  - エラーメッセージ生成
- [ ] T019 パフォーマンステスト: エラー応答 < 1秒
- [ ] T020 ドキュメント更新: モデル解決フローの説明

## 依存関係

```text
T001, T002, T003 → T004-T009 (クリーンアップ → テスト)
T004-T009 → T010-T015 (テスト → 実装)
T010 → T011, T012, T013, T014, T015 (基盤 → 詳細実装)
T010-T015 → T016-T017 (実装 → 統合)
T016-T017 → T018-T020 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T004: ローカル確認 contract test
Task T005: 外部ソースダウンロード contract test
Task T006: プロキシ経由ダウンロード contract test
Task T007: エラーハンドリング contract test
```

## 検証チェックリスト

- [ ] 共有パス関連コードが完全に削除されている (T001-T002)
- [ ] `supported_models.json`未定義モデルでエラーが返る
- [ ] 外部ソースからのダウンロードが正常に動作する
- [ ] ルータープロキシ経由ダウンロードが正常に動作する
- [ ] 重複ダウンロードが防止されている
- [ ] ダウンロード進捗が10%単位で通知される
- [ ] モデル未サポート時に1秒以内にエラーが返る
- [ ] すべてのテストが実装より先にある (TDD RED完了)
