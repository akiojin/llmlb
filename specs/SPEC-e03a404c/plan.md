# 実装計画: 画像認識モデル対応（Image Understanding）

**機能ID**: `SPEC-e03a404c`
**作成日**: 2025-12-24
**ステータス**: 計画中

## 概要

本計画書は、LLM Routerに画像認識（Vision）モデル対応を追加するための実装計画を定義します。

## 技術コンテキスト

### 関連技術スタック

- **ルーター**: Rust (Axum)
- **ノード**: C++ (llama.cpp multimodal support)
- **対応モデル**: LLaVA, Qwen-VL, その他Vision対応モデル
- **API形式**: OpenAI Vision API互換

### 依存するSPEC

| SPEC ID | 機能名 | 依存理由 |
|---------|--------|---------|
| SPEC-63acef08 | 統一APIプロキシ | 基盤APIルーティング |
| SPEC-32637000 | capabilities検証 | Vision capability判定 |
| SPEC-47649000 | モデルメタデータ | Vision対応フラグ管理 |

## 実装フェーズ

### Phase 1: 基盤実装

1. **画像データ構造定義**
   - `ImageContent` 型の定義（URL/Base64対応）
   - MIME type検証
   - サイズ制限チェック

2. **リクエストパーサー拡張**
   - OpenAI Vision API形式のパース
   - 複数画像対応

3. **capabilities拡張**
   - `image_understanding` capabilityの追加
   - `/v1/models` レスポンスへの反映

### Phase 2: ノード連携

1. **ノードプロトコル拡張**
   - 画像データのバイナリ転送
   - multipart/form-data対応検討

2. **llama.cpp multimodal連携**
   - clip embeddings処理
   - 画像プリプロセス

### Phase 3: エラー処理・最適化

1. **エラーハンドリング**
   - 非対応モデルへのリクエスト拒否
   - 画像取得失敗時の適切なエラー

2. **パフォーマンス最適化**
   - 画像キャッシュ
   - 並列処理

## テスト戦略

### Contract Tests

- OpenAI Vision API互換性テスト
- capabilities検証テスト

### Integration Tests

- Visionモデル + 画像URLテスト
- Base64画像テスト
- エラーケーステスト

### E2E Tests

- 実モデルでの画像認識テスト
- ストリーミングテスト

## 成果物

- `router/src/api/vision.rs`: Vision API実装
- `router/src/models/image.rs`: 画像データ構造
- `common/src/types/capabilities.rs`: capabilities拡張
- `tests/integration/vision_test.rs`: 統合テスト

## リスクと緩和策

| リスク | 影響 | 緩和策 |
|-------|------|--------|
| GPUメモリ不足 | 処理失敗 | メモリ監視・早期拒否 |
| 画像転送の遅延 | レスポンス遅延 | 画像圧縮・キャッシュ |
| モデル互換性 | 動作不安定 | 対応モデルリスト管理 |
