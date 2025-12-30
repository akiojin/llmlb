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

- [x] T001 `node/src/` から auto_repair 関連コードを特定・削除
  - ✅ auto_repair関連コードは存在しない（SPEC-3df1b977で廃止済み、未実装）
- [x] T002 auto_repair 関連の環境変数・設定を削除
  - ✅ 環境変数・設定は存在しない
- [x] T003 関連するテストコードを削除
  - ✅ テストコードは存在しない
- [x] T003.1 外部ソース取得の許可リストとHTTPクライアント方針を整理
  - FR-006（許可リスト内の外部ダウンロード許可）対応

## Phase 3.2: テストファースト (TDD RED)

- [x] T004 [P] `node/tests/unit/model_resolver_test.cpp` に共有パス参照の contract test
  - ✅ ResolveFromSharedPathWhenNotLocal (FR-002)
  - ✅ SharedPathDoesNotCopyToLocal (FR-002)
  - ✅ LocalPathTakesPriority (FR-001)
- [x] T005 [P] `node/tests/unit/model_resolver_test.cpp` に外部ソース/プロキシ経由ダウンロードの contract test
  - ✅ DownloadFromOriginWhenSharedInaccessible (FR-003)
  - ✅ DownloadedModelSavedToLocal (FR-004)
  - ✅ SharedPathInaccessibleTriggersOriginFallback (FR-003)
- [x] T006 [P] `node/tests/unit/model_resolver_test.cpp` にモデル不在時のエラーハンドリング contract test
  - ✅ ReturnErrorWhenModelNotFound (FR-005)
  - ✅ ErrorResponseWithinOneSecond (成功基準3)
  - ✅ ClearErrorMessageWhenModelNotFoundAnywhere (US3)
- [x] T007 `node/tests/unit/model_resolver_test.cpp` に統合テスト: 解決フロー全体
  - ✅ FullFallbackFlow (local -> shared -> origin -> error)
  - ✅ HuggingFaceDirectDownloadAllowedWithAllowlist (FR-006)
  - ✅ NoAutoRepairFunctionality (FR-007/成功基準4)
- [x] T007.1 エッジケーステスト追加
  - 🔴 NetworkDisconnectionToSharedPathTriggersRouterFallback - RED
  - 🔴 IncompleteDownloadIsRetried - RED
  - 🔴 PreventDuplicateDownloads - RED: hasDownloadLock未実装
- [x] T007.2 ユーザーストーリー受け入れシナリオテスト
  - ✅ UpdatedSharedPathModelIsUsed (US1-シナリオ2)
- [x] T007.3 技術制約テスト追加
  - ✅ AllowlistBlocksUnknownOrigin
  - ✅ DownloadValidatesArtifactFormat
- [ ] T007.4 Clarificationsテスト追加
  - 🔴 OriginDownloadHasTimeout - RED: タイムアウト設定未実装
  - 🔴 ConcurrentDownloadLimit - RED: 同時ダウンロード制限未実装

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

- [x] T010 `node/src/model_resolver.cpp` に外部ソース/プロキシ経由ダウンロード
  - マニフェストに基づく外部URL取得
  - ルータープロキシ（`/v0/models/registry/.../files/...`）の利用
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

- [x] T012 既存の推論フローに ModelResolver を統合
- [x] T013 重複ダウンロード防止（ミューテックス）
- [x] T014 設定ファイルから共有パス・許可リスト・ルーターURL読み込み

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
