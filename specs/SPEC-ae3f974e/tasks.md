# タスク: 画像生成モデル対応（Image Generation）

**機能ID**: `SPEC-ae3f974e`
**ステータス**: 実装完了（コンパイル検証待ち）
**入力**: `/specs/SPEC-ae3f974e/` の設計ドキュメント

## 技術スタック

- **Router**: Rust 1.75+ (Axum)
- **Node**: C++17 (stable-diffusion.cpp, stb_image)
- **Tests**: cargo test, Google Test
- **対象モデル**: GGML/GGUF形式 (SD 1.x, SD 2.x, SDXL)

## Phase 3.1: セットアップ

- [x] T001 `node/CMakeLists.txt` に BUILD_WITH_SD オプション追加
- [x] T002 stable-diffusion.cpp ライブラリのリンク設定

## Phase 3.2: テストファースト (TDD)

- [x] T003 [P] `router/tests/contract/images_generations_test.rs` に generations API 契約テスト
- [x] T004 [P] `router/tests/contract/images_edits_test.rs` に edits API 契約テスト
- [x] T005 [P] `router/tests/contract/images_variations_test.rs` に variations API 契約テスト
- [x] T006 `router/tests/integration/images_api_test.rs` に画像APIルーティング統合テスト

## Phase 3.3: コア実装 - 型定義・プロトコル

- [x] T007 `common/src/types.rs` に画像関連型を追加
  - RuntimeType::StableDiffusion
  - ModelType::ImageGeneration
  - ImageSize, ImageQuality, ImageStyle, ImageResponseFormat

- [x] T008 `common/src/protocol.rs` にリクエスト/レスポンス型を追加
  - ImageGenerationRequest
  - ImageEditRequest
  - ImageVariationRequest
  - ImageResponse, ImageData

## Phase 3.4: コア実装 - Router側API

- [x] T009 `router/src/api/images.rs` に画像APIエンドポイント実装
  - POST /v1/images/generations
  - POST /v1/images/edits
  - POST /v1/images/variations

- [x] T010 `router/src/api/mod.rs` に images モジュール追加・ルート登録

## Phase 3.5: コア実装 - Node側

- [x] T011 `node/include/core/sd_manager.h` SDマネージャーヘッダー定義
- [x] T012 `node/src/core/sd_manager.cpp` stable-diffusion.cpp統合実装
  - モデルロード
  - text-to-image生成
  - inpainting
  - img2img (バリエーション)

- [x] T013 `node/include/api/image_endpoints.h` 画像エンドポイントヘッダー
- [x] T014 `node/src/api/image_endpoints.cpp` 画像エンドポイント実装
  - /v1/images/generations
  - /v1/images/edits
  - /v1/images/variations

- [x] T015 `node/src/main.cpp` SDManager・ImageEndpoints登録

## Phase 3.6: 統合

- [x] T016 Router-Node間の画像リクエストルーティング統合
- [x] T017 StableDiffusionノードのロードバランシング統合
- [x] T018 エラーハンドリング（ファイルサイズ制限、フォーマット検証）

## Phase 3.7: 仕上げ

- [x] T019 コンパイル検証（BUILD_WITH_SD=ON）
- [x] T020 E2Eテスト: 実モデルでの画像生成
- [x] T021 パフォーマンステスト: 1024x1024画像 < 30秒
  - 実測: stable-diffusion/sd_turbo.safetensors, steps=4, 23s
- [x] T022 ドキュメント更新

## 依存関係

```text
T001, T002 → T011-T015 (ビルド設定 → Node実装)
T003-T006 → T007-T010 (テスト → Router実装)
T007, T008 → T009, T010 (型定義 → API実装)
T011, T012 → T013, T014 (SDManager → エンドポイント)
T009-T015 → T016-T018 (実装 → 統合)
T016-T018 → T019-T022 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T003: router/tests/contract/images_generations_test.rs
Task T004: router/tests/contract/images_edits_test.rs
Task T005: router/tests/contract/images_variations_test.rs
```

## 検証チェックリスト

- [x] すべてのユーザーストーリーに対応するタスクがある
- [x] すべてのテストが実装より先にある (TDD)
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] コンパイル検証完了 (BUILD_WITH_SD=ON)
- [x] E2Eテスト完了
