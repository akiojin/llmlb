# タスク: モデル capabilities に基づくルーティング検証

**入力**: `/specs/SPEC-32637000/`の設計ドキュメント
**前提条件**: plan.md (必須)

## Phase 3.1: セットアップ

- [x] T001 既存のcommon/src/types.rsとllmlb/src/api/構造を確認

## Phase 3.2: テストファースト (TDD) - 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### ModelCapability enum テスト

- [x] T002 [P] `common/src/types.rs` に ModelCapability serialization テスト追加
  - `test_model_capability_serialization`: 各variantが正しくsnake_caseでシリアライズされることを確認
  - `test_model_capability_deserialization`: JSONから正しくデシリアライズされることを確認

- [x] T003 [P] `common/src/types.rs` に ModelCapability::from_model_type テスト追加
  - `test_model_capability_from_model_type`: 各ModelTypeから正しいcapabilitiesが推定されることを確認

### API capabilities検証テスト

- [x] T004 [P] `llmlb/src/api/audio.rs` に TTS capabilities検証テスト追加
  - テスト: TextToSpeech capability を持たないモデルで `/v1/audio/speech` を呼ぶとエラー
  - 期待エラー: "Model 'X' does not support text-to-speech"

- [x] T005 [P] `llmlb/src/api/audio.rs` に ASR capabilities検証テスト追加
  - テスト: SpeechToText capability を持たないモデルで `/v1/audio/transcriptions` を呼ぶとエラー
  - 期待エラー: "Model 'X' does not support speech-to-text"

- [x] T006 [P] `llmlb/src/api/openai.rs` に chat capabilities検証テスト追加
  - テスト: TextGeneration capability を持たないモデルで `/v1/chat/completions` を呼ぶとエラー
  - 期待エラー: "Model 'X' does not support text generation"

- [x] T007 [P] `llmlb/src/api/images.rs` に 画像生成 capabilities検証テスト追加
  - テスト: ImageGeneration capability を持たないモデルで `/v1/images/generations` を呼ぶとエラー
  - 期待エラー: "Model 'X' does not support image generation"

## Phase 3.3: コア実装 (テストが失敗した後のみ)

### ModelCapability enum 実装

- [x] T008 `common/src/types.rs` に ModelCapability enum 追加
  - 6つのvariant: TextGeneration, TextToSpeech, SpeechToText, ImageGeneration, Vision, Embedding
  - Derive: Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash
  - serde(rename_all = "snake_case")

- [x] T009 `common/src/types.rs` に ModelCapability::from_model_type 実装
  - ModelType::Llm → [TextGeneration]
  - ModelType::Embedding → [Embedding]
  - ModelType::SpeechToText → [SpeechToText]
  - ModelType::TextToSpeech → [TextToSpeech]
  - ModelType::ImageGeneration → [ImageGeneration]

### API capabilities検証実装

- [x] T010 `llmlb/src/api/audio.rs` の speech ハンドラーに TextToSpeech 検証追加
  - モデル取得 → capabilities確認 → 非対応ならエラー返却

- [x] T011 `llmlb/src/api/audio.rs` の transcriptions ハンドラーに SpeechToText 検証追加

- [x] T012 `llmlb/src/api/openai.rs` の chat_completions ハンドラーに TextGeneration 検証追加

- [x] T013 `llmlb/src/api/images.rs` の generations ハンドラーに ImageGeneration 検証追加

## Phase 3.4: 統合

- [x] T014 `llmlb/src/registry/models.rs` に ModelInfo.capabilities フィールド追加
  - 型: `Vec<ModelCapability>`
  - デフォルト: ModelType から自動推定

- [x] T015 `llmlb/src/api/models.rs` の `/v1/models` レスポンスに capabilities を含める
  - 各モデルのcapabilitiesをレスポンスに追加

## Phase 3.5: 仕上げ

- [x] T016 [P] 後方互換性テスト追加
  - capabilities未設定モデルがModelTypeから正しく推定されることを確認

- [x] T017 全テスト実行・確認
  - `cargo test` ですべてのテストがパス

- [x] T018 `cargo fmt --check` と `cargo clippy -- -D warnings` 確認

## 依存関係

```
T001 (セットアップ)
  ↓
T002, T003 (enum テスト) [並列]
  ↓
T008, T009 (enum 実装)
  ↓
T004, T005, T006, T007 (API テスト) [並列]
  ↓
T010, T011, T012, T013 (API 実装)
  ↓
T014, T015 (統合)
  ↓
T016, T017, T018 (仕上げ)
```

## 並列実行例

```
# T002-T003 を一緒に起動 (enum テスト):
Task: "common/src/types.rs に ModelCapability serialization テスト"
Task: "common/src/types.rs に ModelCapability::from_model_type テスト"

# T004-T007 を一緒に起動 (API テスト):
Task: "llmlb/src/api/audio.rs に TTS capabilities検証テスト"
Task: "llmlb/src/api/audio.rs に ASR capabilities検証テスト"
Task: "llmlb/src/api/openai.rs に chat capabilities検証テスト"
Task: "llmlb/src/api/images.rs に 画像生成 capabilities検証テスト"
```

## 注意事項

- [P] タスク = 異なるファイル、依存関係なし
- 実装前にテストが失敗することを確認 (RED フェーズ)
- 各タスク後にコミット
- テストコミット → 実装コミットの順序を厳守
