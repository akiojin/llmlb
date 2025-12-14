# 実装計画: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000` | **日付**: 2024-12-14 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-26006000/spec.md`の機能仕様

## 実行フロー (/speckit.plan コマンドのスコープ)

```text
1. 入力パスから機能仕様を読み込み
   → specs/SPEC-26006000/spec.md を確認済み
2. 技術コンテキストを記入 (要明確化をスキャン)
   → プロジェクトタイプ: single (Router + Node)
3. 下記の憲章チェックセクションを評価
   → 違反なし
4. Phase 0 を実行 → research.md
   → 完了
5. Phase 1 を実行 → contracts, data-model.md, quickstart.md
   → 完了
6. 憲章チェックセクションを再評価
   → 合格
7. Phase 2 を計画 → タスク生成アプローチを記述
   → 本ファイルで完了
8. 停止 - /speckit.tasks コマンドの準備完了
```

## 概要

llm-routerに音声モデル対応を追加する:

- **ASR (Speech-to-Text)**: whisper.cppを使用した音声認識
- **TTS (Text-to-Speech)**: ONNX Runtimeを使用した音声合成
- **OpenAI API互換**: `/v1/audio/transcriptions`, `/v1/audio/speech`
- **実装範囲**: Router側 (Rust) + Node側 (C++)

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+, C++20
**主要依存関係**: llama.cpp (既存), whisper.cpp (新規ASR), ONNX Runtime (新規TTS)
**ストレージ**: 既存のモデル管理システム
**テスト**: cargo test, Google Test (C++)
**対象プラットフォーム**: Linux (CUDA), macOS (Metal)
**プロジェクトタイプ**: single (Router + Node)
**パフォーマンス目標**: ASR 2x realtime, TTS 100文字/3秒
**制約**: GPUメモリ共有時の同時実行制限
**スケール/スコープ**: 10並列音声処理

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 2 (router, node) - 既存構成維持
- フレームワークを直接使用? はい (whisper.cpp, ONNX Runtime直接利用)
- 単一データモデル? はい (既存のModelType拡張)
- パターン回避? はい (余分な抽象化なし)

**アーキテクチャ**:

- すべての機能をライブラリとして? はい (llm-common crateを拡張)
- ライブラリリスト:
  - `llm-common`: 共通型定義 (ModelType, RequestType拡張)
  - `llm-router`: APIルーティング (audio.rs追加)
  - `llm-node`: 推論エンジン (whisper_manager, onnx_tts_manager追加)
- ライブラリごとのCLI: N/A (サーバーコンポーネント)
- ライブラリドキュメント: 既存のllms.txt形式を拡張

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? はい
- Gitコミットはテストが実装より先に表示? はい
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? はい
- 実依存関係を使用? はい (実際のwhisper.cpp/ONNX Runtime)
- Integration testの対象: 新しいライブラリ、契約変更、共有スキーマ? はい
- 禁止: テスト前の実装、REDフェーズのスキップ

**可観測性**:

- 構造化ロギング含む? はい (既存のspdlog/tracing拡張)
- フロントエンドログ → バックエンド? N/A (APIのみ)
- エラーコンテキスト十分? はい (OpenAI APIエラーフォーマット)

**バージョニング**:

- バージョン番号割り当て済み? はい (semantic-release)
- 変更ごとにBUILDインクリメント? はい
- 破壊的変更を処理? はい (ModelType拡張は後方互換)

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-26006000/
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
│   ├── transcriptions.yaml
│   └── speech.yaml
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード (リポジトリルート)

```text
# Router (Rust)
router/src/api/
├── audio.rs             # 新規: /v1/audio/* エンドポイント
├── proxy.rs             # 変更: RuntimeType別ルーティング
└── mod.rs               # 変更: audioモジュール追加

common/src/
├── types.rs             # 変更: ModelType拡張
└── protocol.rs          # 変更: RequestType, AudioRequest追加

# Node (C++)
node/include/core/
├── whisper_manager.h    # 新規: whisper.cpp管理
└── onnx_tts_manager.h   # 新規: ONNX TTS管理

node/src/core/
├── whisper_manager.cpp  # 新規: ASR実装
└── onnx_tts_manager.cpp # 新規: TTS実装

node/src/api/
└── audio_endpoints.cpp  # 新規: 音声APIハンドラ

node/CMakeLists.txt      # 変更: whisper.cpp, ONNX Runtime追加
```

**構造決定**: オプション1 (単一プロジェクト) - 既存のRouter + Node構成を維持

## Phase 0: アウトライン＆リサーチ

### 技術コンテキストの不明点

1. **whisper.cpp統合方法**: llama.cppと同じggml基盤のため、サブモジュールとして追加可能
2. **ONNX Runtime統合方法**: FetchContentまたはシステムインストールで依存追加
3. **GPUメモリ競合**: ランタイムごとにメモリ分離（別プロセスまたはコンテキスト分離）

### リサーチ結果サマリー

| 項目 | 決定 | 理由 |
|------|------|------|
| ASRランタイム | whisper.cpp | llama.cppと同じ作者、ggml互換、Metal/CUDA対応 |
| TTSランタイム | ONNX Runtime | クロスプラットフォーム、GPU対応、VibeVoice等対応 |
| 音声フォーマット | WAV, MP3, FLAC | libsndfile/ffmpeg経由でデコード |
| APIフォーマット | OpenAI Audio API互換 | multipart/form-data (ASR), JSON (TTS) |

**出力**: [research.md](./research.md) を参照

## Phase 1: 設計＆契約

*前提条件: research.md完了*

### 1. データモデル

**ModelType拡張** (`common/src/types.rs`):

```rust
pub enum ModelType {
    Llm,           // 既存
    Embedding,     // 既存
    SpeechToText,  // 新規: ASR
    TextToSpeech,  // 新規: TTS
}
```

**RuntimeType追加** (`common/src/types.rs`):

```rust
pub enum RuntimeType {
    LlamaCpp,      // 既存のllama.cpp
    WhisperCpp,    // 新規: whisper.cpp
    OnnxRuntime,   // 新規: ONNX Runtime
}
```

**RequestType拡張** (`common/src/protocol.rs`):

```rust
pub enum RequestType {
    Chat,          // 既存
    Generate,      // 既存
    Embeddings,    // 既存
    Transcription, // 新規: ASR
    Speech,        // 新規: TTS
}
```

**出力**: [data-model.md](./data-model.md) を参照

### 2. API契約

**POST /v1/audio/transcriptions** (ASR):

```yaml
Request (multipart/form-data):
  file: binary (required) - 音声ファイル
  model: string (required) - whisper-large-v3等
  language: string (optional) - ja, en等
  response_format: string (optional) - json, text, srt, vtt

Response (200 OK):
  { "text": "認識されたテキスト" }
```

**POST /v1/audio/speech** (TTS):

```yaml
Request (application/json):
  model: string (required) - vibevoice-v1等
  input: string (required) - 読み上げテキスト
  voice: string (optional) - nova, alloy等
  response_format: string (optional) - mp3, wav, opus

Response (200 OK):
  Content-Type: audio/mpeg
  Body: binary audio data
```

**出力**: [contracts/](./contracts/) を参照

### 3. クイックスタート

```bash
# ASR: 音声認識
curl -X POST http://localhost:8080/v1/audio/transcriptions \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@audio.wav" \
  -F "model=whisper-large-v3"

# TTS: 音声合成
curl -X POST http://localhost:8080/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"vibevoice-v1","input":"こんにちは","voice":"nova"}' \
  --output speech.mp3
```

**出力**: [quickstart.md](./quickstart.md) を参照

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:

- `/templates/tasks-template.md` をベースとして読み込み
- Phase 1設計ドキュメント (contracts, data model, quickstart) からタスクを生成
- 各contract → contract testタスク [P]
- 各entity → model作成タスク [P]
- 各ユーザーストーリー → integration testタスク
- テストを合格させる実装タスク

**順序戦略**:

1. **Setup**: 依存関係追加 (whisper.cpp, ONNX Runtime)
2. **Types**: 型定義拡張 (ModelType, RuntimeType, RequestType)
3. **Contract Tests**: API契約テスト (先に作成、失敗確認)
4. **Router実装**: audio.rs, proxy.rs拡張
5. **Node実装**: whisper_manager, onnx_tts_manager
6. **Integration Tests**: E2E音声処理テスト
7. **Polish**: ドキュメント、エラーハンドリング強化

**並列化可能タスク**:

- [P] whisper.cpp統合とONNX Runtime統合は独立して進行可能
- [P] ASR契約テストとTTS契約テストは並列作成可能
- [P] Router側とNode側の型定義は同時に進行可能

**推定出力**: tasks.mdに25-30個の番号付き、順序付きタスク

**重要**: このフェーズは/speckit.tasksコマンドで実行、/speckit.planではない

## Phase 3+: 今後の実装

*これらのフェーズは/planコマンドのスコープ外*

**Phase 3**: タスク実行 (/speckit.tasksコマンドがtasks.mdを作成)
**Phase 4**: 実装 (憲章原則に従ってtasks.mdを実行)
**Phase 5**: 検証 (テスト実行、quickstart.md実行、パフォーマンス検証)

## 複雑さトラッキング

*憲章チェックに正当化が必要な違反がある場合のみ記入*

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

*このチェックリストは実行フロー中に更新される*

**フェーズステータス**:

- [x] Phase 0: Research完了 (/speckit.plan コマンド)
- [x] Phase 1: Design完了 (/speckit.plan コマンド)
- [x] Phase 2: Task planning完了 (/speckit.plan コマンド - アプローチのみ記述)
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---
*憲章 v2.1.1 に基づく - `/memory/constitution.md` 参照*
