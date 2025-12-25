# タスク: 画像認識モデル対応（Image Understanding）

**機能ID**: `SPEC-e03a404c`
**ステータス**: 計画中
**入力**: `/specs/SPEC-e03a404c/` の設計ドキュメント

## 技術スタック

- **Router**: Rust 1.75+ (Axum)
- **Node**: C++17 (llama.cpp multimodal support)
- **対応モデル**: LLaVA, Qwen-VL, その他Vision対応モデル
- **API形式**: OpenAI Vision API互換
- **Tests**: cargo test, Google Test

## Phase 3.1: セットアップ

- [x] T001 依存SPECの実装状況確認
  - SPEC-63acef08 (統一APIプロキシ) ✅ 実装済み
  - SPEC-32637000 (capabilities検証) ✅ 実装済み
  - SPEC-47649000 (モデルメタデータ) ✅ 実装済み

## Phase 3.2: テストファースト (TDD)

- [ ] T002 [P] `router/tests/contract/vision_chat_test.rs` に画像付きchat completions契約テスト
  - 画像URL形式
  - Base64形式
  - 複数画像
- [ ] T003 [P] `router/tests/contract/vision_error_test.rs` にエラーハンドリング契約テスト
  - Vision非対応モデルへのリクエスト拒否
  - 画像取得失敗
  - サイズ制限超過
- [ ] T004 [P] `router/tests/contract/vision_capabilities_test.rs` にcapabilities契約テスト
  - `/v1/models` での `image_understanding` 表示
- [ ] T005 `router/tests/integration/vision_api_test.rs` に統合テスト

## Phase 3.3: コア実装 - 型定義

- [ ] T006 `common/src/types.rs` に画像関連型を追加
  - ImageContent (URL/Base64)
  - ImageContentType (MIME type)
  - VisionCapability

- [ ] T007 `common/src/protocol.rs` にVision用メッセージ型を追加
  - ContentPart (text/image_url)
  - ImageUrl
  - Vision対応ChatCompletionRequest拡張

- [ ] T008 `common/src/types/capabilities.rs` に `image_understanding` capability追加

## Phase 3.4: コア実装 - Router側

- [ ] T009 `router/src/models/image.rs` に画像データ構造を実装
  - Base64デコード
  - URL画像取得
  - MIME type検証
  - サイズ制限チェック (最大10MB)

- [ ] T010 `router/src/api/chat.rs` にVision対応拡張
  - マルチパートコンテンツのパース
  - 画像データの抽出・変換
  - Vision非対応モデル検出・エラー

- [ ] T011 `router/src/api/models.rs` にcapabilities情報追加
  - `/v1/models` レスポンスに `image_understanding` を含める

## Phase 3.5: コア実装 - Node側

- [ ] T012 `node/src/core/vision_processor.cpp` に画像プリプロセス実装
  - 画像デコード
  - リサイズ/正規化
  - CLIP embeddings生成

- [ ] T013 `node/src/api/chat_endpoints.cpp` にVision対応拡張
  - 画像データ受信
  - llama.cpp multimodal連携

## Phase 3.6: 統合

- [ ] T014 Router-Node間の画像データ転送実装
  - バイナリデータの効率的な転送
- [ ] T015 ストリーミングレスポンス対応 (stream=true)
- [ ] T016 複数画像処理 (最大10枚)

## Phase 3.7: 仕上げ

- [ ] T017 [P] ユニットテスト追加
  - Base64デコードロジック
  - MIME type検証
  - サイズ制限チェック
- [ ] T018 E2Eテスト: 実モデル（LLaVA等）での画像認識
- [ ] T019 パフォーマンステスト: 1024x1024画像 < 5秒
- [ ] T020 ドキュメント更新: Vision API使用方法

## 依存関係

```text
T001 → T002-T005 (依存確認 → テスト)
T002-T005 → T006-T008 (テスト → 型定義)
T006-T008 → T009-T011 (型定義 → Router実装)
T006-T008 → T012-T013 (型定義 → Node実装)
T009-T013 → T014-T016 (実装 → 統合)
T014-T016 → T017-T020 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T002: router/tests/contract/vision_chat_test.rs
Task T003: router/tests/contract/vision_error_test.rs
Task T004: router/tests/contract/vision_capabilities_test.rs
```

## 検証チェックリスト

- [ ] 画像URL付きchat completionsが正常動作
- [ ] Base64画像付きリクエストが正常動作
- [ ] 複数画像（最大10枚）が処理可能
- [ ] Vision非対応モデルへのリクエストが400エラー
- [ ] `/v1/models` に `image_understanding` capability表示
- [ ] ストリーミングレスポンス対応
- [ ] 1024x1024画像の処理が5秒以内
- [ ] すべてのテストが実装より先にある (TDD)
