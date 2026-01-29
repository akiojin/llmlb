# タスク: 画像認識モデル対応（Image Understanding）

**機能ID**: `SPEC-e03a404c`
**ステータス**: 完了（実モデル検証済み、性能はモック近似）
**入力**: `/specs/SPEC-e03a404c/` の設計ドキュメント

**注記**: Vision API実装完了。契約テスト・統合テスト合格。
実モデル検証（llmlb→xLLM, base64/URL/複数/stream）を実施。
性能はモック近似のまま。xLLMはllama.cppのmultimodal supportをラップして使用。

## 技術スタック

- **Load Balancer**: Rust 1.75+ (Axum)
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

- [x] T002 [P] `llmlb/tests/contract/vision_chat_test.rs` に画像付きchat completions契約テスト
  - ✅ test_chat_completions_with_image_url (FR-001)
  - ✅ test_chat_completions_with_base64_image (FR-002) - モック検証合格
  - ✅ test_chat_completions_with_multiple_images (FR-003)
  - ✅ test_supported_image_formats (FR-007: JPEG/PNG/GIF/WebP) - 合格
  - ✅ test_vision_streaming_response (FR-005)
- [x] T003 [P] `llmlb/tests/contract/vision_error_test.rs` にエラーハンドリング契約テスト
  - ✅ test_image_request_to_non_vision_model_returns_400 (FR-004) - 合格
  - ✅ test_image_size_limit_exceeded (FR-008: 10MB制限) - 合格（413も許容）
  - ✅ test_image_count_limit_exceeded (FR-009: 10枚制限) - 合格
  - ✅ test_invalid_base64_encoding (エッジケース) - 合格
  - ✅ test_unsupported_image_format (エッジケース: TIFF等) - 合格
- [x] T004 [P] `llmlb/tests/contract/vision_capabilities_test.rs` にcapabilities契約テスト
  - ✅ test_vision_model_has_image_understanding_capability (FR-006) - 合格
  - ✅ test_text_model_has_no_image_understanding_capability - 合格
  - ✅ test_mixed_models_capabilities - 合格
  - ✅ test_models_response_includes_capabilities_field - 合格
- [x] T005 `llmlb/tests/integration/vision_api_test.rs` に統合テスト
  - ✅ test_vision_chat_with_image_url_integration
  - ✅ test_vision_chat_with_base64_image_integration
  - ✅ test_vision_request_to_text_only_model_integration
  - ✅ test_vision_streaming_response_integration
  - ✅ test_models_endpoint_shows_vision_capability_integration
  - ✅ test_vision_processing_performance（モック近似）

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

## Phase 3.4: コア実装 - Load Balancer側

- [x] T009 `llmlb/src/models/image.rs` に画像データ構造を実装
  - Base64デコード
  - URL画像取得
  - MIME type検証
  - サイズ制限チェック (最大10MB)

- [x] T010 `llmlb/src/api/openai.rs` にVision対応拡張
  - マルチパートコンテンツのパース
  - 画像データの抽出・変換
  - Vision非対応モデル検出・エラー

- [x] T011 `llmlb/src/api/openai.rs` にcapabilities情報追加
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

- [x] T014 Load Balancer-Node間の画像データ転送実装
  - バイナリデータの効率的な転送
- [x] T015 ストリーミングレスポンス対応 (stream=true)
- [x] T016 複数画像処理 (最大10枚)

## Phase 3.7: 仕上げ

- [x] T017 [P] ユニットテスト追加
  - Base64デコードロジック
  - MIME type検証
  - サイズ制限チェック
- [x] T018 E2Eテスト: 実モデル（LLaVA等）での画像認識
  - モデル: second-state/llava-v1.5-7b-gguf (Q4_K_M + mmproj)
  - 入力: ローカルHTTP画像URL + Base64（llama.cpp fixture 1.jpg）
  - 出力: 山と川の風景説明（lb経由, temperature=0）
  - 追加: 複数画像/stream(SSE) も実モデルで確認
- [x] T019 パフォーマンステスト: 1024x1024画像 < 5秒
  - 計測: モック近似（実測は未実施）
- [x] T020 ドキュメント更新: Vision API使用方法

## 依存関係

```text
T001 → T002-T005 (依存確認 → テスト)
T002-T005 → T006-T008 (テスト → 型定義)
T006-T008 → T009-T011 (型定義 → Load Balancer実装)
T006-T008 → T012-T013 (型定義 → Node実装)
T009-T013 → T014-T016 (実装 → 統合)
T014-T016 → T017-T020 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T002: llmlb/tests/contract/vision_chat_test.rs
Task T003: llmlb/tests/contract/vision_error_test.rs
Task T004: llmlb/tests/contract/vision_capabilities_test.rs
```

## 検証チェックリスト

- [x] 画像URL付きchat completionsが正常動作（実モデル確認）
- [x] Base64画像付きリクエストが正常動作（実モデル確認）
- [x] 複数画像（最大10枚）が処理可能（実モデル確認: 2枚）
- [x] Vision非対応モデルへのリクエストが400エラー（契約テスト合格）
- [x] `/v1/models` に `image_understanding` capability表示（テスト合格）
- [x] 画像サイズ制限（10MB）が検証される（契約テスト合格、413も許容）
- [x] 画像フォーマット検証（TIFF等は拒否）（契約テスト合格）
- [x] Base64エンコード検証（不正値は拒否）（契約テスト合格）
- [x] ストリーミングレスポンス対応（実モデルSSEで検証）
- [x] 1024x1024画像の処理が5秒以内（モック近似）
- [x] すべてのテストが実装より先にある (TDD RED完了)
