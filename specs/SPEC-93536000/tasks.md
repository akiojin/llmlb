# タスク一覧: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**ステータス**: 完了
**作成日**: 2026-01-03
**更新日**: 2026-01-04

## Phase 1: データモデル（基盤）

### Setup

- [x] [P] 1.1 `Node` 構造体に `executable_models: Vec<String>` を追加
- [x] [P] 1.2 `Node` 構造体に `excluded_models: HashSet<String>` を追加

## Phase 2: Node側実装

### Core

- [x] 2.1 `GpuBackend` 列挙型を `node/src/system/gpu_detector.hpp` に追加
- [x] 2.2 `GpuDetector::getGpuBackend()` 関数を実装
- [x] 2.3 `ModelRegistry::listExecutableModels(GpuBackend)` を実装
- [x] 2.4 `ModelRegistry::isCompatible(ModelInfo, GpuBackend)` を実装
- [x] 2.5 `/v1/models` APIを拡張し、GPU互換モデルIDのみを返す

## Phase 3: Load Balancer側実装（コア）

### Core

- [x] 3.1 ノード登録時に `/v1/models` を呼び出してモデル一覧を取得（プル型）
- [x] 3.2 `/v1/models` 取得失敗時の登録拒否を実装
- [x] 3.3 空のモデルリスト時の登録拒否を実装
- [x] 3.4 `NodeRegistry::get_nodes_for_model()` を実装
- [x] 3.5 `NodeRegistry::exclude_model_from_node()` を実装（モデル単位除外）
- [x] **3.6** `select_node()` にモデルフィルタを追加（最重要）
- [x] **3.7** `select_available_node_with_queue()` に `model_id` 引数を追加
- [x] 3.8 `chat_completions()` で model_id を渡すよう修正
- [x] 3.9 `embeddings()` で model_id を渡すよう修正
- [x] 3.10 `completions()` で model_id を渡すよう修正
- [x] 3.11 推論失敗時のモデル除外処理を追加（proxy.rs）

## Phase 4: Load Balancer側実装（API）

### Integration

- [x] 4.1 `/v1/models` APIをノードベース集約に変更
- [x] 4.2 `NoCapableNodes` エラー型を追加 (`llmlb/src/error.rs`)
- [x] 4.3 404 Model Not Found エラーハンドリングを実装

## Phase 5: 廃止対応

### Polish

- [x] 5.1 REGISTERED_MODELS と supported_models.json を削除
- [x] 5.2 SPEC-dcaeaec4 FR-9 を廃止としてマーク

## Phase 6: テスト

### Test

- [x] [P] 6.1 Unit Test: `get_nodes_for_model()` フィルタリング
- [x] [P] 6.2 Unit Test: `exclude_model_from_node()` 動作確認
- [x] [P] 6.3 Unit Test: ノード登録時の/v1/models取得
- [x] 6.4 Integration Test: モデル対応ノードへのルーティング
- [x] 6.5 Integration Test: 非対応モデルへの503エラー
- [x] 6.6 Integration Test: 存在しないモデルへの404エラー
- [x] 6.7 Integration Test: 推論失敗後のモデル除外
- [x] 6.8 Integration Test: ノード再起動後のモデル復帰
- [x] 6.9 E2E Test: Metal専用モデルがCUDAノードにルーティングされないこと

## Phase 7: 自動認識への移行（2026-01-25追加）

### 廃止対応

- [x] 7.1 `xllm/include/models/supported_models_json.h` を削除
- [x] 7.2 `supported_models.json` 参照箇所を特定・削除（ModelRegistry等）
- [x] 7.3 `platforms` 属性に基づくフィルタリングを削除

### Core

- [x] **7.4** `config.json` からアーキテクチャを読み込む（既存: `model_storage.cpp` の `extract_architectures_from_config()`）
- [x] **7.5** GGUFファイルからアーキテクチャを読み込む（既存: llama.cpp の GGUF メタデータ経由）
- [x] 7.6 `ModelStorage::listModels()` を更新: config.json/GGUFからアーキテクチャ自動検出（既存実装済み）
- [x] 7.7 `/v1/models` APIを更新: 対応エンジンが存在するモデルのみを返す（既存: main.cppでengine.isModelSupported()でフィルタリング）

### Test

- [x] [P] 7.8 Unit Test: config.jsonからのアーキテクチャ検出
  - ExtractArchitecturesFromConfigJson（単一アーキテクチャ）
  - ExtractMultipleArchitecturesFromConfigJson（複数アーキテクチャ）
  - ExtractArchitecturesHandlesMissingConfigJson（config.json欠損）
  - ExtractArchitecturesHandlesEmptyArchitecturesArray（空配列）
  - ExtractArchitecturesHandlesMalformedConfigJson（不正形式）
  - ExtractArchitecturesSkipsNonStringValues（非文字列スキップ）
- [x] [P] 7.9 Unit Test: GGUFからのアーキテクチャ検出
  - GgufModelWithoutConfigJsonUsesLlamaCpp（GGUFのランタイム検出）
  - Note: GGUFメタデータからの検出はllama.cppロード時に実行、統合テストで確認
- [x] [P] 7.10 Unit Test: 未対応アーキテクチャの除外
  - NormalizesUnknownArchitecturesToCompactForm（未知アーキテクチャの正規化）
  - NormalizesKnownArchitectureFamilies（既知ファミリー12種のテスト）
  - DeduplicatesArchitectures（重複排除）
- [x] 7.11 Integration Test: 任意のHuggingFaceモデルの自動認識
  - ListAvailableIncludesArchitectures（モデル一覧）
  - ListAvailableDescriptorsIncludesArchitectureInfo（Descriptor経由）

## 凡例

- `[P]` - 並列実行可能なタスク
- **太字** - 最重要タスク
