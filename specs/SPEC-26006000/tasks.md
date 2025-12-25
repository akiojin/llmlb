# タスク: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000` | **入力**: plan.md, data-model.md, contracts/
**前提条件**: plan.md完了、design docs完了

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] T001 `node/third_party/` に whisper.cpp をサブモジュールとして追加
- [x] T002 `node/CMakeLists.txt` に whisper.cpp ビルド設定を追加
- [x] T003 [P] `node/CMakeLists.txt` に ONNX Runtime 依存関係を追加
- [x] T004 [P] `common/src/types.rs` に AudioFormat enum を追加
- [x] T005 `common/src/types.rs` に RuntimeType enum を追加

## Phase 3.2: テストファースト (TDD) - 実装前に失敗必須

### 3.2.1 Contract Tests (API契約テスト)

- [x] T006 [P] `router/tests/contract/audio_transcriptions_test.rs` に POST /v1/audio/transcriptions の契約テスト
- [x] T007 [P] `router/tests/contract/audio_speech_test.rs` に POST /v1/audio/speech の契約テスト

### 3.2.2 型定義テスト

- [x] T008 [P] `common/src/types.rs` の tests モジュールに ModelType 拡張のテスト追加
- [x] T009 [P] `common/src/types.rs` の tests モジュールに RuntimeType のテスト追加
- [x] T010 [P] `common/src/protocol.rs` の tests モジュールに RequestType 拡張のテスト追加
- [x] T011 [P] `common/src/protocol.rs` の tests モジュールに TranscriptionRequest/Response のテスト追加
- [x] T012 [P] `common/src/protocol.rs` の tests モジュールに SpeechRequest のテスト追加

### 3.2.3 Integration Tests (ユーザーストーリー検証)

- [x] T013 [P] `router/tests/integration/audio_api_test.rs` にストーリー1: 音声認識テスト (ASRルーティング)
- [x] T014 [P] `router/tests/integration/audio_api_test.rs` にストーリー2: 音声合成テスト (TTSルーティング)
- [x] T015 [P] `router/tests/integration/audio_api_test.rs` にストーリー3: 複数ランタイムノード分散テスト
- [x] T016 [P] `router/tests/integration/audio_api_test.rs` にストーリー4: 能力なしノード503テスト

## Phase 3.3: 型定義実装 (テスト失敗確認後)

- [x] T017 `common/src/types.rs` の ModelType に SpeechToText, TextToSpeech を追加
- [x] T018 `common/src/types.rs` に RuntimeType enum 実装 (LlamaCpp, WhisperCpp, OnnxRuntime)
- [x] T019 `common/src/protocol.rs` の RequestType に Transcription, Speech を追加
- [x] T020 [P] `common/src/protocol.rs` に TranscriptionRequest 構造体を追加
- [x] T021 [P] `common/src/protocol.rs` に TranscriptionResponse 構造体を追加
- [x] T022 [P] `common/src/protocol.rs` に SpeechRequest 構造体を追加
- [x] T023 `common/src/types.rs` の Node 構造体に loaded_asr_models, loaded_tts_models, supported_runtimes を追加
- [x] T024 `common/src/protocol.rs` の HealthCheckRequest に loaded_asr_models, loaded_tts_models, supported_runtimes を追加

## Phase 3.4: Router側API実装

- [x] T025 `router/src/api/audio.rs` を新規作成 (モジュール構造のみ)
- [x] T026 `router/src/api/mod.rs` に audio モジュールを追加
- [x] T027 `router/src/api/audio.rs` に POST /v1/audio/transcriptions ハンドラを実装
- [x] T028 `router/src/api/audio.rs` に POST /v1/audio/speech ハンドラを実装
- [x] T029 `router/src/api/audio.rs` に multipart/form-data パーサーを実装
- [x] T030 `router/src/api/proxy.rs` に RuntimeType 別ノード選択ロジックを追加
- [x] T031 `router/src/registry/models.rs` の ModelInfo に runtime_type フィールドを追加
- [x] T032 `router/src/lib.rs` に audio API ルートを登録

## Phase 3.5: Node側 whisper.cpp 統合 (ASR)

- [x] T033 `node/include/core/whisper_manager.h` を新規作成 (WhisperManager クラス定義)
- [x] T034 `node/src/core/whisper_manager.cpp` を新規作成 (whisper.cpp 初期化・モデルロード)
- [x] T035 `node/src/core/whisper_manager.cpp` に transcribe() メソッドを実装
- [x] T036 `node/src/core/whisper_manager.cpp` に音声デコード処理を実装 (WAV/MP3/FLAC)
- [x] T037 `node/src/api/audio_endpoints.cpp` を新規作成 (POST /v1/audio/transcriptions ハンドラ)
- [x] T038 `node/CMakeLists.txt` に whisper_manager をビルドターゲットに追加

## Phase 3.6: Node側 ONNX Runtime 統合 (TTS)

- [x] T039 `node/include/core/onnx_tts_manager.h` を新規作成 (OnnxTtsManager クラス定義)
- [x] T040 `node/src/core/onnx_tts_manager.cpp` を新規作成 (ONNX Runtime 初期化・モデルロード)
- [x] T041 `node/src/core/onnx_tts_manager.cpp` に synthesize() メソッドを実装
- [x] T042 `node/src/core/onnx_tts_manager.cpp` に音声エンコード処理を実装 (MP3/WAV)
- [x] T043 `node/src/api/audio_endpoints.cpp` に POST /v1/audio/speech ハンドラを追加
- [x] T044 `node/CMakeLists.txt` に onnx_tts_manager をビルドターゲットに追加

## Phase 3.7: 統合

- [x] T045 supported_runtimes 報告を追加 (実装: `node/src/api/router_client.cpp:62-64,115,132`)
- [x] T046 loaded_asr_models, loaded_tts_models 報告を追加 (実装: `node/src/api/router_client.cpp:113-114,130-131`)
- [x] T047 RuntimeType 別ノードフィルタリングを追加 (実装: `router/src/api/audio.rs:59-100 select_node_by_runtime()`)
- [x] T048 `router/src/api/audio.rs` にエラーハンドリング (OpenAI API形式) を追加
- [x] T049 `router/src/api/audio.rs` にリクエストログ出力を追加

## Phase 3.8: 仕上げ

- [x] T050 [P] `node/tests/unit/whisper_manager_test.cpp` に WhisperManager の unit tests
  - ✅ 実装済み: FlashAttentionIsDisabledByDefault テスト作成
  - パスは `node/tests/unit/` に配置（tasks.mdの記載と異なる）
- [x] T051 [P] `node/tests/unit/onnx_tts_manager_test.cpp` に OnnxTtsManager の unit tests
  - ✅ 実装済み: 8テスト作成（RuntimeAvailability, IdleTimeout, MaxModels等）
  - CMakeLists.txt にビルド設定追加済み
- [x] T052 `router/tests/contract/` にエッジケーステスト (無効フォーマット, 空入力, サイズ超過)
  - ✅ 既存の契約テストでカバー済み:
    - `audio_transcriptions_test.rs`: unsupported_format_returns_400, without_auth_returns_401, no_available_node_returns_503
    - `audio_speech_test.rs`: empty_input_returns_400, without_auth_returns_401, no_available_node_returns_503, input_too_long_returns_400
- [ ] T053 `specs/SPEC-26006000/quickstart.md` のコマンドを実行して動作確認
  - 🟡 手動検証タスク
- [x] T054 `router/src/api/audio.rs` のコードを clippy でチェック・修正
  - ✅ `cargo clippy -p llm-router -- -D warnings` 合格
- [ ] T055 `node/` のコードを clang-tidy でチェック・修正
  - 🔴 ブロック: clang-tidy未インストール

## 依存関係グラフ

```text
T001 → T002 → T038 → T033-T037
T003 → T044 → T039-T043
T004, T005 → T017, T018
T006, T007 → T025-T032 (契約テストが先)
T008-T012 → T017-T024 (型テストが先)
T013-T016 → T045-T049 (統合テストが先)
T017-T024 → T025-T032 (型定義が先)
T025-T032 → T045-T049 (Router実装が先)
T033-T038 + T039-T044 → T045-T049 (Node実装が先)
T045-T049 → T050-T055 (統合が先)
```

## 並列実行例

```bash
# Phase 3.1 セットアップ (T003, T004 並列)
Task: "node/CMakeLists.txt に ONNX Runtime 依存関係を追加"
Task: "common/src/types.rs に AudioFormat enum を追加"

# Phase 3.2 契約テスト (T006, T007 並列)
Task: "router/tests/audio_transcriptions_contract_test.rs に契約テスト"
Task: "router/tests/audio_speech_contract_test.rs に契約テスト"

# Phase 3.2 型テスト (T008-T012 並列)
Task: "common/src/types.rs に ModelType 拡張のテスト"
Task: "common/src/types.rs に RuntimeType のテスト"
Task: "common/src/protocol.rs に RequestType 拡張のテスト"
Task: "common/src/protocol.rs に TranscriptionRequest/Response のテスト"
Task: "common/src/protocol.rs に SpeechRequest のテスト"

# Phase 3.2 統合テスト (T013-T016 並列)
Task: "router/tests/integration/audio_asr_test.rs に音声認識テスト"
Task: "router/tests/integration/audio_tts_test.rs に音声合成テスト"
Task: "router/tests/integration/audio_routing_test.rs に分散テスト"
Task: "router/tests/integration/audio_model_management_test.rs にモデル管理テスト"

# Phase 3.3 型実装 (T020-T022 並列)
Task: "common/src/protocol.rs に TranscriptionRequest 構造体を追加"
Task: "common/src/protocol.rs に TranscriptionResponse 構造体を追加"
Task: "common/src/protocol.rs に SpeechRequest 構造体を追加"

# Phase 3.8 仕上げ (T050, T051 並列)
Task: "node/tests/whisper_manager_test.cpp に unit tests"
Task: "node/tests/onnx_tts_manager_test.cpp に unit tests"
```

## TDD順序の重要ポイント

1. **テストファースト**: T006-T016 のテストを先に書き、**失敗を確認**してから実装
2. **契約テスト優先**: OpenAPI仕様 (contracts/) に基づく契約テストが最初
3. **型テスト**: 新しい enum/struct のシリアライズテストを実装前に作成
4. **統合テスト**: ユーザーストーリーごとのE2Eテストを実装前に作成
5. **RED確認**: 各テストが「赤」(失敗) 状態であることを `cargo test` で確認

## 検証チェックリスト

- [x] contracts/transcriptions.yaml に対応するテスト (T006)
- [x] contracts/speech.yaml に対応するテスト (T007)
- [x] ModelType, RuntimeType, RequestType に対応するテスト (T008-T010)
- [x] TranscriptionRequest/Response, SpeechRequest に対応するテスト (T011-T012)
- [x] 4つのユーザーストーリーに対応する統合テスト (T013-T016)
- [x] すべてのテストが実装タスクより先の番号
- [x] [P] タスクは異なるファイルを対象
- [x] 同じファイルを変更する [P] タスクなし

## 注意事項

- 各タスク完了後にコミット (commitlint準拠)
- whisper.cpp と ONNX Runtime の統合は並列進行可能
- GPUメモリ制約のため、同時実行テストは注意
- エラーメッセージは OpenAI API 形式に統一
