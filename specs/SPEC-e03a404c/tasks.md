# タスク: 画像認識モデル対応（Image Understanding）

**機能ID**: `SPEC-e03a404c`
**ステータス**: Phase 3.2完了（TDD RED）/ Phase 3.3進行中
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

## Phase 3.2: テストファースト (TDD RED)

- [x] T002 [P] `router/tests/contract/vision_chat_test.rs` に画像付きchat completions契約テスト
  - 🔴 test_chat_completions_with_image_url (FR-001)
  - 🔴 test_chat_completions_with_base64_image (FR-002)
  - 🔴 test_chat_completions_with_multiple_images (FR-003)
  - 🔴 test_supported_image_formats (FR-007: JPEG/PNG/GIF/WebP)
  - 🔴 test_vision_streaming_response (FR-005)
- [x] T003 [P] `router/tests/contract/vision_error_test.rs` にエラーハンドリング契約テスト
  - 🔴 test_image_request_to_non_vision_model_returns_400 (FR-004)
  - 🔴 test_image_size_limit_exceeded (FR-008: 10MB制限)
  - 🔴 test_image_count_limit_exceeded (FR-009: 10枚制限)
  - 🔴 test_invalid_base64_encoding (エッジケース)
  - 🔴 test_unsupported_image_format (エッジケース: TIFF等)
- [x] T004 [P] `router/tests/contract/vision_capabilities_test.rs` にcapabilities契約テスト
  - 🔴 test_vision_model_has_image_understanding_capability (FR-006)
  - 🔴 test_text_model_has_no_image_understanding_capability
  - 🔴 test_mixed_models_capabilities
  - 🔴 test_models_response_includes_capabilities_field
- [x] T005 `router/tests/integration/vision_api_test.rs` に統合テスト
  - 🔴 test_vision_chat_with_image_url_integration [ignore]
  - 🔴 test_vision_chat_with_base64_image_integration [ignore]
  - 🔴 test_vision_request_to_text_only_model_integration [ignore]
  - 🔴 test_models_endpoint_shows_vision_capability_integration [ignore]
  - 🔴 test_vision_processing_performance [ignore]

## Phase 3.3: コア実装 - 型定義

- [x] T006 `common/src/types.rs` に画像関連型を追加
  - ImageContent (URL/Base64)
  - ImageContentType (MIME type)
  - VisionCapability

- [x] T007 `common/src/protocol.rs` にVision用メッセージ型を追加
  - ContentPart (text/image_url)
  - ImageUrl
  - Vision対応ChatCompletionRequest拡張

- [x] T008 `common/src/types.rs` の ModelCapabilities に `image_understanding` を追加

## Phase 3.4: コア実装 - Router側

- [x] T009 `router/src/models/image.rs` に画像データ構造を実装
  - Base64デコード
  - URL画像取得
  - MIME type検証
  - サイズ制限チェック (最大10MB)

- [x] T010 `router/src/api/openai.rs` にVision対応拡張
  - マルチパートコンテンツのパース
  - 画像データの抽出・変換
  - Vision非対応モデル検出・エラー

- [x] T011 `router/src/api/openai.rs` にcapabilities情報追加
  - `/v1/models` レスポンスに `image_understanding` を含める

## Phase 3.5: コア実装 - Node側

- [x] T012 `node/src/core/vision_processor.cpp` に画像プリプロセス実装
  - 画像デコード
  - リサイズ/正規化
  - CLIP embeddings生成

- [x] T013 `node/src/api/openai_endpoints.cpp` にVision対応拡張
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

- [ ] 画像URL付きchat completionsが正常動作 (Phase 3.3で実装予定)
- [ ] Base64画像付きリクエストが正常動作 (Phase 3.3で実装予定)
- [ ] 複数画像（最大10枚）が処理可能 (Phase 3.3で実装予定)
- [ ] Vision非対応モデルへのリクエストが400エラー (Phase 3.4で実装予定)
- [ ] `/v1/models` に `image_understanding` capability表示 (Phase 3.4で実装予定)
- [ ] ストリーミングレスポンス対応 (Phase 3.6で実装予定)
- [ ] 1024x1024画像の処理が5秒以内 (Phase 3.7で検証予定)
- [x] すべてのテストが実装より先にある (TDD RED完了)
