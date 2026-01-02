# 実装計画: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03

## 概要

ルーターのモデル管理アーキテクチャを根本的に変更する大規模リファクタリング。

**現状の問題点**:

1. ルーターが「対応モデル」を中央管理し、全ノードが全モデル対応と仮定
2. `select_node()` でモデルIDによるフィルタリングがない（致命的）
3. GPUバックエンド（Metal/CUDA/DirectML）の明示的な識別がない
4. `SupportedModel.platforms` フィールドは存在するが活用されていない

**変更後**:

- 各ノードがGPUバックエンドに基づき実行可能なモデルを報告
- ルーターがオンラインノードの実行可能モデルを集約
- リクエストは対応ノードにのみルーティング

## 確定した設計判断

| 項目 | 決定内容 |
|------|----------|
| **platforms情報** | supported_models.json のみで管理（ノード側に埋め込み）|
| **変更範囲** | Router側 + Node側（C++）両方 |
| **後方互換** | 不要（executable_modelsを報告しないノードは対象外）|
| **/v1/models** | オンラインノードの対応モデルのみ返す |
| **executable_models** | ノードのGPUで実行可能な全モデル（ロード状態に関係なく）|
| **取得方法** | Node側の `/v1/models` APIから取得 |
| **エラー応答** | `/v1/models`にないモデル → 404 Model Not Found |
| **SPEC-dcaeaec4 FR-9** | 完全廃止（全ノード全モデル対応 → GPU互換モデルのみ対応）|
| **Router supported_models.json** | 完全削除 |
| **Node supported_models.json** | ビルド時に埋め込み、GPU互換性判定に使用 |

## 実装フェーズ

### Phase 1: データモデル（基盤）

1. `GpuBackend` 列挙型追加 (`common/src/types.rs`)
2. `Node` 構造体拡張（`gpu_backend`, `executable_models`）
3. `RegisterRequest` / `HealthCheckRequest` 拡張
4. DBスキーマ変更

### Phase 2: Node側実装

1. `GpuDetector::getGpuBackend()` 実装
2. `ModelRegistry::listExecutableModels()` 実装
3. `/v1/models` API拡張
4. ハートビート拡張

### Phase 3: Router側実装（コア）

1. ノード登録時の `gpu_backend` 処理
2. ヘルスチェックでの `executable_models` 更新
3. `NodeRegistry::get_nodes_for_model()` 実装
4. `select_node()` にモデルフィルタ追加（最重要）
5. `select_available_node_with_queue()` に model_id 引数追加
6. 各OpenAI APIエンドポイントの修正

### Phase 4: Router側実装（API）

1. `/v1/models` APIをノードベース集約に変更
2. エラーハンドリング実装

### Phase 5: テスト

1. Unit Tests: モデルフィルタリング、GPU互換性判定
2. Integration Tests: 複数ノード・複数バックエンドでのルーティング
3. E2E Tests: Metal専用モデルがCUDAノードにルーティングされないこと

## 影響を受けるファイル一覧

### Router

- `router/src/api/openai.rs` - `/v1/models`、各エンドポイント
- `router/src/api/proxy.rs` - ノード選択
- `router/src/api/nodes.rs` - ノード登録
- `router/src/balancer/mod.rs` - `select_node()`, `select_node_by_metrics()`
- `router/src/registry/mod.rs` - Node構造体、Registry
- `router/src/health/mod.rs` - ヘルスチェック
- `router/src/error.rs` - エラー型
- `common/src/types.rs` - 共通型
- `common/src/protocol.rs` - プロトコル

### Node

- `node/src/api/openai_endpoints.cpp` - `/v1/models`
- `node/src/system/gpu_detector.cpp` - GPU検出
- `node/src/system/gpu_detector.mm` - Metal検出
- `node/src/model/model_registry.cpp` - モデル登録
- `node/src/health/heartbeat.cpp` - ハートビート

### Database

- SQLiteスキーマ変更

## リスクと軽減策

| リスク | 軽減策 |
|--------|--------|
| 既存ノードとの互換性 | 後方互換不要と決定済み |
| パフォーマンス低下 | `executable_models` をキャッシュ、ハートビート時のみ更新 |
| モデル互換性誤判定 | `platforms` フィールドを信頼、不明な場合は全バックエンド対応扱い |
