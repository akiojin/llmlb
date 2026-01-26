# タスク: モデル自動解決機能

**機能ID**: `SPEC-48678000`
**ステータス**: 完了（自動認識への移行 2026-01-27完了）
**入力**: `/specs/SPEC-48678000/` の設計ドキュメント

## 重要な変更 (Session 2025-12-30)

- 共有パス機能は**廃止**
- ~~対象モデルは`supported_models.json`に定義されたもののみ~~ **廃止（2026-01-25）**
- 解決フロー: ローカル → 外部ソース（プロキシは任意）

## 重要な変更 (Session 2025-12-31)

- 対象は全モデル（GGUF / safetensors / Metal 等）
- Node は外部ソースから直接ダウンロード（ロードバランサーのバイナリ保持は不要）
- 変換パイプラインは廃止（登録時に変換しない）

## 重要な変更 (Session 2026-01-25)

- `supported_models.json` 依存を**完全廃止**
- 任意のHuggingFaceモデルを取得可能に変更
- `config.json` からアーキテクチャを自動検出
- 対応エンジンが存在するモデルのみ実行可能

## 追加対応（Session 2025-12-31）

- [x] ロードバランサープロキシ経由ダウンロードのテスト/実装を削除（旧仕様の整理）
- [x] download_url 依存を廃止し、repo/filename ベースに統一

## 技術スタック

- **Node**: C++17 (llama.cpp, httplib)
- **Load Balancer**: Rust 1.75+ (reqwest)
- **Storage**: ファイルシステム (`~/.llmlb/models/`)
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
- [x] T003.2 共有パス設定/参照の削除
  - ModelResolver/設定/README から共有パス前提を削除

## Phase 3.2: テストファースト (TDD RED)

- [x] T004 [P] `node/tests/unit/model_resolver_test.cpp` にローカル解決の contract test
  - ✅ LocalPathTakesPriority (FR-001)
- [x] T005 [P] `node/tests/unit/model_resolver_test.cpp` に外部ソース/プロキシ経由ダウンロードの contract test
  - ✅ DownloadFromOriginWhenNotLocal (FR-003)
  - ✅ DownloadFromProxyWhenNoOriginUrl (FR-003)
  - ✅ DownloadedModelSavedToLocal (FR-004)
  - ✅ OriginBlockedTriggersProxyFallback (FR-006)
  - ✅ DownloadFromOriginUsesRecommendedFilename (supported_models.json)
- [x] T006 [P] `node/tests/unit/model_resolver_test.cpp` にモデル不在時のエラーハンドリング contract test
  - ✅ ReturnErrorWhenModelNotFound (FR-005)
  - ✅ ErrorResponseWithinOneSecond (成功基準2)
  - ✅ ClearErrorMessageWhenModelNotFoundAnywhere (US3)
- [x] T007 `node/tests/unit/model_resolver_test.cpp` に追加テスト: auto_repair 非搭載確認
  - ✅ NoAutoRepairFunctionality (FR-007/成功基準4)
- [x] T007.1 エッジケーステスト追加
  - ✅ IncompleteDownloadIsRetried
  - ✅ PreventDuplicateDownloads
- [x] T007.3 技術制約テスト追加（仕様更新により再定義）
  - [x] SupportsSafetensorsAndGgufFormats
  - [x] MetalArtifactIsOptional
- [x] T007.4 Clarificationsテスト追加
  - ✅ RouterDownloadHasTimeout（外部ソース/プロキシのタイムアウト）
  - ✅ ConcurrentDownloadLimit（同時ダウンロード数の上限）

## Phase 3.3: コア実装

- [x] T010 `node/src/model_resolver.cpp` にモデル解決フローを実装
  - ローカルキャッシュ確認
  - `supported_models.json`参照（ロードバランサー /v0/models）
  - 外部ソース→ロードバランサープロキシの順でダウンロード
  - エラーハンドリング
- [x] T011 `node/src/model_resolver.cpp` に`supported_models.json`参照ロジック
  - モデル定義の読み込み
  - 外部ソースURL取得（repo+recommended_filename）
  - 未定義モデルのエラー生成
- [x] T012 外部ソース（HF等）からのダウンロード実装
  - 許可リストで制御
  - ローカルへの保存処理
- [x] T013 ロードバランサープロキシ経由ダウンロード実装（旧仕様・廃止予定）
  - ロードバランサープロキシ（`/v1/models/blob/:model_name`）
  - ローカルへの保存処理
- [x] T014 重複ダウンロード防止
  - ダウンロードロック機構
  - 同時リクエストの待機処理
- [x] T015 エラーハンドリング
  - モデル未サポートエラー
  - ネットワークエラー

## Phase 3.4: 統合

- [x] T016 既存の推論フローに ModelResolver を統合
- [x] T017 設定ファイルから許可リスト・ロードバランサーURL読み込み（共有パス設定は廃止）

## Phase 3.5: 仕上げ

- [x] T018 [P] ユニットテスト追加
  - パス検証ロジック
  - エラーメッセージ生成
- [x] T019 パフォーマンステスト: エラー応答 < 1秒
- [x] T020 ドキュメント更新: モデル解決フローの説明

## Phase 3.6: 進捗可視化（追加）

- [x] T021 [P] Node: モデル同期の進捗/状態を取得できるようにする（モデル名・ファイル名・downloaded/total）
- [x] T022 [P] Load Balancer: ヘルスチェックに同期状態を取り込み、APIで取得できるようにする
- [x] T023 [P] Dashboard: ノード一覧/詳細に同期状態と進捗を表示する
- [x] T024 [P] ModelResolver経由のダウンロード進捗をsync_stateとして報告する
- [x] T025 [P] ModelResolverの進捗報告テストを追加する

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
Task T004: ローカル解決 contract test
Task T005: 外部ソース/プロキシ contract test
Task T006: エラーハンドリング contract test
Task T007.1: エッジケース contract test
```

## Phase 4: 自動認識への移行（2026-01-25追加）

### 廃止対応

- [x] T026 `supported_models.json` 参照を `config.json` ベースに変更（xllm/llmlb両方で完了）
- [x] T027 モデル定義チェックを削除（任意のHFモデル対応: llmlb/src/api/models.rs 更新済み）

### Core

- [x] **T028** ダウンロード後の `config.json` からアーキテクチャ自動検出を実装（既存: model_storage.cpp）
- [x] T029 対応エンジンがない場合のエラーハンドリングを追加（既存: engine.isModelSupported() + EngineRegistry）

### Test

- [x] [P] T030 Unit Test: config.jsonからのアーキテクチャ検出（既存: model_storage_test.cpp）
- [x] [P] T031 Unit Test: 未対応アーキテクチャのエラー応答
  - IsModelSupportedReturnsFalseForUnsupportedArchitecture
  - IsModelSupportedReturnsTrueForSupportedArchitecture
  - LoadModelUnsupportedArchitectureReturnsProperErrorFormat
  - LoadModelRejectsUnsupportedArchitecture（既存テスト）

## 検証チェックリスト

- [x] auto_repair 関連コードが完全に削除されている (T001-T003)
- [x] ~~`supported_models.json` の定義に基づく取得が動作する~~ **廃止予定**
- [x] 外部ソース/プロキシ経由ダウンロードが正常に動作する (Phase 3.3で実装済み)
- [x] モデル不在時に1秒以内にエラーが返る (テスト: ErrorResponseWithinOneSecond)
- [x] 外部ダウンロードが許可リストで制御されている (テスト: OriginBlockedTriggersProxyFallback)
- [x] すべてのテストが実装より先にある (TDD RED完了)
- [x] **新規**: config.jsonからアーキテクチャが自動検出される（SPEC-93536000で完了）
- [x] **新規**: 任意のHuggingFaceモデルが取得可能（llmlb/src/api/models.rsで実装済み）
