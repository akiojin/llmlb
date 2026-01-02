# タスク一覧: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03

## Phase 1: データモデル（基盤）

### Setup

- [ ] [P] 1.1 `GpuBackend` 列挙型を `common/src/types.rs` に追加
- [ ] [P] 1.2 `Node` 構造体に `gpu_backend`, `executable_models` フィールドを追加
- [ ] 1.3 `RegisterRequest` に `gpu_backend` を追加 (`router/src/api/nodes.rs`)
- [ ] 1.4 `HealthCheckRequest` に `executable_models`, `gpu_backend` を追加 (`common/src/protocol.rs`)
- [ ] 1.5 DBスキーマ変更（nodes テーブルに gpu_backend、node_executable_models テーブル追加）

## Phase 2: Node側実装

### Core

- [ ] 2.1 `GpuDetector::getGpuBackend()` 関数を実装 (`node/src/system/gpu_detector.cpp`)
- [ ] 2.2 `ModelRegistry::listExecutableModels(GpuBackend)` を実装
- [ ] 2.3 `ModelRegistry::isCompatible(ModelInfo, GpuBackend)` を実装
- [ ] 2.4 `/v1/models` APIを拡張し、`gpu_backend` と GPU互換モデルのみを返す
- [ ] 2.5 ハートビートに `executable_models` と `gpu_backend` を追加

## Phase 3: Router側実装（コア）

### Core

- [ ] 3.1 `infer_gpu_backend()` 関数を実装（ノード登録時のGPU推定）
- [ ] 3.2 `NodeRegistry::update_executable_models()` を実装
- [ ] 3.3 `NodeRegistry::get_nodes_for_model()` を実装
- [ ] 3.4 ヘルスチェック処理で `executable_models` を更新
- [ ] **3.5** `select_node()` にモデルフィルタを追加（最重要）
- [ ] **3.6** `select_available_node_with_queue()` に `model_id` 引数を追加
- [ ] 3.7 `chat_completions()` で model_id を渡すよう修正
- [ ] 3.8 `embeddings()` で model_id を渡すよう修正
- [ ] 3.9 `completions()` で model_id を渡すよう修正

## Phase 4: Router側実装（API）

### Integration

- [ ] 4.1 `/v1/models` APIをノードベース集約に変更
- [ ] 4.2 `NoCapableNodes` エラー型を追加 (`router/src/error.rs`)
- [ ] 4.3 404 Model Not Found エラーハンドリングを実装
- [ ] 4.4 REGISTERED_MODELS と supported_models.json を削除

## Phase 5: テスト

### Test

- [ ] [P] 5.1 Unit Test: `GpuBackend` シリアライズ/デシリアライズ
- [ ] [P] 5.2 Unit Test: `infer_gpu_backend()` 関数
- [ ] [P] 5.3 Unit Test: `get_nodes_for_model()` フィルタリング
- [ ] 5.4 Integration Test: モデル対応ノードへのルーティング
- [ ] 5.5 Integration Test: 非対応モデルへの503エラー
- [ ] 5.6 Integration Test: 存在しないモデルへの404エラー
- [ ] 5.7 E2E Test: Metal専用モデルがCUDAノードにルーティングされないこと

## Phase 6: 廃止対応

### Polish

- [ ] 6.1 SPEC-dcaeaec4 FR-9 を廃止としてマーク
- [ ] 6.2 Router側の supported_models.json を削除
- [ ] 6.3 REGISTERED_MODELS 関連コードを削除

## 凡例

- `[P]` - 並列実行可能なタスク
- **太字** - 最重要タスク
