# タスク一覧: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03
**更新日**: 2026-01-03

## Phase 1: データモデル（基盤）

### Setup

- [x] [P] 1.1 `Node` 構造体に `executable_models: Vec<String>` を追加
- [x] [P] 1.2 `Node` 構造体に `excluded_models: Vec<String>` を追加（一貫性のためVecを使用）

## Phase 2: Node側実装

### Core

- [x] 2.1 `GpuBackend` 列挙型を `node/include/system/gpu_detector.h` に追加
- [x] 2.2 `GpuDetector::getGpuBackend()` 関数を実装
- [x] 2.3 `ModelRegistry::listExecutableModels(GpuBackend)` を実装
- [x] 2.4 `ModelRegistry::isCompatible(ModelInfo, GpuBackend)` を実装
- [x] 2.5 `/v1/models` APIを拡張し、GPU互換モデルIDのみを返す

## Phase 3: Router側実装（コア）

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

## Phase 4: Router側実装（API）

### Integration

- [x] 4.1 `/v1/models` APIをノードベース集約に変更
- [x] 4.2 `NoCapableNodes` エラー型を追加 (`common/src/error.rs`)
- [x] 4.3 404 Model Not Found エラーハンドリングを実装

## Phase 5: 廃止対応

### Polish

- [x] 5.1 REGISTERED_MODELS と supported_models.json を削除
- [x] 5.2 SPEC-dcaeaec4 FR-9 を廃止としてマーク

## Phase 6: テスト

### Test

- [x] [P] 6.1 Unit Test: `get_nodes_for_model()` フィルタリング
- [x] [P] 6.2 Unit Test: `exclude_model_from_node()` 動作確認
- [x] [P] 6.3 Integration Test: ノード登録時の/v1/models取得
- [x] 6.4 Integration Test: モデル対応ノードへのルーティング (TDD RED)
- [x] 6.5 Integration Test: 非対応モデルへの503エラー (TDD RED)
- [x] 6.6 Integration Test: 存在しないモデルへの404エラー
- [x] 6.7 Integration Test: 推論失敗後のモデル除外 (TDD RED)
- [x] 6.8 Integration Test: ノード再起動後のモデル復帰 (TDD RED)
- [x] 6.9 E2E Test: Metal専用モデルがCUDAノードにルーティングされないこと (TDD RED)

## 凡例

- `[P]` - 並列実行可能なタスク
- **太字** - 最重要タスク
