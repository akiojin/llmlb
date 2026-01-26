# 実装計画: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03

## 概要

ロードバランサーのモデル管理アーキテクチャを根本的に変更する大規模リファクタリング。

**現状の問題点**:

1. ロードバランサーが「対応モデル」を中央管理し、全ノードが全モデル対応と仮定
2. `select_node()` でモデルIDによるフィルタリングがない（致命的）
3. GPUバックエンド（Metal/CUDA/DirectML）の明示的な識別がない
4. `SupportedModel.platforms` フィールドは存在するが活用されていない

**変更後**:

- 各ノードがGPUバックエンドに基づき実行可能なモデルを報告
- ロードバランサーがオンラインノードの実行可能モデルを集約
- リクエストは対応ノードにのみルーティング

## 確定した設計判断

| 項目 | 決定内容 |
|------|----------|
| **platforms情報** | supported_models.json のみで管理（ノード側に埋め込み）|
| **変更範囲** | Load Balancer側 + Node側（C++）両方 |
| **後方互換** | 不要（/v1/modelsが空のノードは登録拒否）|
| **/v1/models** | オンラインノードの対応モデルのみ返す |
| **executable_models** | ノードのGPUで実行可能な全モデル（起動時に固定）|
| **取得方法** | ロードバランサーがノード登録時に `/v1/models` をプルして取得 |
| **取得タイミング** | ノード登録時のみ（定期更新なし、再起動で再取得）|
| **GPUバックエンド** | ロードバランサー側では不要（ノード側のみで使用）|
| **ストレージ** | メモリのみ（DB永続化なし）|
| **エラー応答** | `/v1/models`にないモデル → 404 Model Not Found |
| **推論失敗時** | モデル単位で即除外（ノード再起動で復帰）|
| **モデルIDマッチング** | 完全一致（大文字小文字区別）|
| **コールドスタート** | ノード未登録時は503を即座に返す |
| **マルチGPU** | プライマリGPUのみ使用 |
| **GPU優先度** | なし（負荷分散のみで選択）|
| **SPEC-dcaeaec4 FR-9** | 完全廃止 |
| **Load Balancer supported_models.json** | 完全削除 |
| **Node supported_models.json** | ビルド時に埋め込み、GPU互換性判定に使用 |
| **モデルエントリ検証** | 「id」フィールドのみ必須、不正エントリはスキップ |
| **ノード再登録** | excluded_modelsをクリア（除外リセット）|
| **進行中リクエスト** | 除外は新規リクエストのみに影響 |
| **フィーチャーフラグ** | 不要（一括切り替え）|
| **ダッシュボードUI** | スコープ外（変更なし）|

## 実装フェーズ

### Phase 1: データモデル（基盤）

1. `Node` 構造体に `executable_models: Vec<String>` を追加
2. `Node` 構造体に `excluded_models: HashSet<String>` を追加（メモリのみ）
3. `RegisterRequest` は変更なし（既存のまま）

### Phase 2: Node側実装

1. `GpuBackend` 列挙型追加 (`node/src/system/gpu_detector.cpp`)
2. `GpuDetector::getGpuBackend()` 実装
3. `ModelRegistry::listExecutableModels(GpuBackend)` 実装
4. `ModelRegistry::isCompatible(ModelInfo, GpuBackend)` 実装
5. `/v1/models` APIを拡張し、GPU互換モデルIDのみを返す

### Phase 3: Load Balancer側実装（コア）

1. ノード登録時にノードの `/v1/models` を呼び出して取得（プル型）
2. `NodeRegistry::get_nodes_for_model()` 実装
3. `NodeRegistry::exclude_model_from_node()` 実装（モデル単位除外）
4. `select_node()` にモデルフィルタ追加（最重要）
5. `select_available_node_with_queue()` に `model_id` 引数追加
6. 各OpenAI APIエンドポイントの修正
7. 推論失敗時のモデル除外処理を追加

### Phase 4: Load Balancer側実装（API）

1. `/v1/models` APIをノードベース集約に変更
2. `NoCapableNodes` エラー型を追加
3. 404 Model Not Found エラーハンドリングを実装

### Phase 5: 廃止対応

1. REGISTERED_MODELS と supported_models.json を削除
2. SPEC-dcaeaec4 FR-9 を廃止としてマーク

### Phase 6: テスト

1. Unit Tests: モデルフィルタリング、モデル除外/復帰
2. Integration Tests: 複数ノードでのルーティング
3. E2E Tests: Metal専用モデルがCUDAノードにルーティングされないこと

## 影響を受けるファイル一覧

### Load Balancer

- `llmlb/src/api/openai.rs` - `/v1/models`、各エンドポイント
- `llmlb/src/api/proxy.rs` - ノード選択、推論失敗時のモデル除外
- `llmlb/src/api/nodes.rs` - ノード登録時の/v1/models取得
- `llmlb/src/balancer/mod.rs` - `select_node()`, `select_node_by_metrics()`
- `llmlb/src/registry/mod.rs` - Node構造体（executable_models, excluded_models追加）
- `llmlb/src/error.rs` - エラー型（NoCapableNodes追加）

### Node

- `node/src/api/openai_endpoints.cpp` - `/v1/models`（GPU互換モデルのみ返す）
- `node/src/system/gpu_detector.cpp` - GPU検出、GpuBackend列挙型
- `node/src/system/gpu_detector.mm` - Metal検出
- `node/src/model/model_registry.cpp` - listExecutableModels(), isCompatible()

### Database

- 変更なし（メモリのみで管理）

## リスクと軽減策

| リスク | 軽減策 |
|--------|--------|
| 既存ノードとの互換性 | 後方互換不要と決定済み |
| ロードバランサー再起動時のモデル情報消失 | ノードが10秒毎に再登録するため影響は軽微 |
| モデル除外の誤判定 | 1回の失敗で即除外だが、ノード再起動で復帰可能 |
| モデル互換性誤判定 | `platforms` フィールドを信頼、不明な場合は全バックエンド対応扱い |
