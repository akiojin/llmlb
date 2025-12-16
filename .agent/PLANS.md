# Feature Branch: replace-llama-to-onnx-runtime

## 目的

llama.cpp ベースの推論エンジンを ONNX Runtime ベースに置き換え、
マルチモーダルAI（テキスト・画像・音声）をOpenAI互換APIで統合提供する。

### 主要目標

1. **LLM推論**: llama.cpp → ONNX Runtime への移行
2. **画像生成 (T2I)**: Z-Image-Turbo (diffusers) による画像生成
3. **画像理解 (I2T)**: GLM-4.6V-Flash (transformers) によるキャプション生成
4. **音声認識 (ASR)**: Whisper.cpp による音声→テキスト変換
5. **音声合成 (TTS)**: VibeVoice (ONNX) によるテキスト→音声変換

## これまでの経緯

### Phase 1: PoC検証 ✅ 完了

| PoC | 内容 | 状態 |
|-----|------|------|
| `poc/onnx-runtime-demo/` | ONNX Runtime でのLLM推論検証 | ✅ 完了 |
| `poc/vibevoice-pytorch/` | VibeVoice TTS検証 | ✅ 完了 |
| `poc/audio-io-demo/` | Whisper ASR + VibeVoice TTS統合 | ✅ 完了 |
| `poc/image-io-demo/` | T2I (Z-Image-Turbo) + I2T (GLM-4.6V) | ✅ 完了 |
| `poc/gpu-detection/` | GPU検出・VRAM管理 | ✅ 完了 |

### Phase 2: Node実装 ✅ 完了

| コンポーネント | ファイル | 状態 |
|---------------|---------|------|
| OnnxLlmManager | `node/src/core/onnx_llm_manager.cpp` | ✅ 実装済 |
| WhisperManager | `node/src/core/whisper_manager.cpp` | ✅ 実装済 |
| OnnxTtsManager | `node/src/core/onnx_tts_manager.cpp` | ✅ 実装済 |
| ImageManager | `node/src/core/image_manager.cpp` | ✅ 実装済 |
| ImageEndpoints | `node/src/api/image_endpoints.cpp` | ✅ 実装済 |
| AudioEndpoints | `node/src/api/audio_endpoints.cpp` | ✅ 実装済 |

### Phase 3: Router実装 ✅ 完了

| エンドポイント | ファイル | 状態 |
|---------------|---------|------|
| `/v1/chat/completions` | `router/src/api/proxy.rs` | ✅ 実装済 |
| `/v1/audio/transcriptions` | `router/src/api/audio.rs` | ✅ 実装済 |
| `/v1/audio/speech` | `router/src/api/audio.rs` | ✅ 実装済 |
| `/v1/images/generations` | `router/src/api/images.rs` | ✅ 実装済 |
| `/v1/images/edits` | `router/src/api/images.rs` | ✅ 実装済 |
| `/v1/images/variations` | `router/src/api/images.rs` | ✅ 実装済 |

### Phase 4: テスト整備 ✅ 完了

| テストカテゴリ | 件数 | 状態 |
|--------------|------|------|
| 音声認識 (ASR) Contract Tests | 4件 | ✅ 有効化済 |
| 音声合成 (TTS) Contract Tests | 6件 | ✅ 有効化済 |
| 画像生成 Contract Tests | 7件 | ✅ 有効化済 |
| 画像編集 Contract Tests | 6件 | ✅ 有効化済 |
| 画像バリエーション Contract Tests | 6件 | ✅ 有効化済 |

## 現在の状態

### RuntimeType と supported_runtimes

```
RuntimeType:
  - onnx_runtime    : LLM推論 + TTS
  - whisper_cpp     : ASR (音声認識)
  - stable_diffusion: 画像生成 (T2I/I2T)
```

### テスト結果サマリー

```
cargo test 結果:
  - Unit Tests: 172 passed
  - Contract Tests: 40 passed, 7 ignored (旧API削除分)
  - Integration Tests: 全て合格
  - E2E Tests: 全て合格
```

## これからの計画

### 短期 (残タスク)

1. **Integration Tests の有効化**
   - `audio_api_test.rs`: TDD RED状態 → ルーティング実装後にGREEN化
   - `images_api_test.rs`: TDD RED状態 → ルーティング実装後にGREEN化

2. **ランタイム別ルーティング強化**
   - ASRリクエスト → `whisper_cpp` ランタイムを持つノードへ
   - TTSリクエスト → `onnx_runtime` ランタイムを持つノードへ
   - 画像リクエスト → `stable_diffusion` ランタイムを持つノードへ

### 中期

1. **パフォーマンス最適化**
   - Pythonサブプロセスのプリロード
   - GPU VRAMの動的管理
   - バッチ処理対応

2. **エラーハンドリング強化**
   - GPU不足時の明示的エラーメッセージ
   - モデル未ダウンロード時のフォールバック

### 長期

1. **非同期処理**
   - 画像生成の非同期キュー
   - 進捗通知 (WebSocket/SSE)

2. **追加モデル対応**
   - FLUX等の大規模画像生成モデル
   - 多言語TTS

## アーキテクチャ図

```
┌─────────────────────────────────────────────────────────────┐
│                         Router (Rust)                        │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────────┐   │
│  │ /v1/chat│ │/v1/audio│ │/v1/images│ │ Load Balancer   │   │
│  │completions│ │speech/  │ │generations│ │ (runtime別)    │   │
│  │         │ │transcribe│ │edits/var │ │                 │   │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────────┬────────┘   │
│       │           │           │                │            │
└───────┼───────────┼───────────┼────────────────┼────────────┘
        │           │           │                │
        ▼           ▼           ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│                      Node (C++ httplib)                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │OnnxLlmManager│ │WhisperManager│ │   ImageManager      │   │
│  │(ONNX Runtime)│ │(whisper.cpp) │ │(Python subprocess)  │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
│  ┌─────────────┐                                            │
│  │OnnxTtsManager│                                            │
│  │(VibeVoice)   │                                            │
│  └─────────────┘                                            │
└─────────────────────────────────────────────────────────────┘
        │                   │                   │
        ▼                   ▼                   ▼
   ┌─────────┐        ┌──────────┐       ┌─────────────┐
   │ONNX Models│      │Whisper   │       │Python Scripts│
   │(LLM/TTS) │       │Models    │       │(T2I/I2T)    │
   └─────────┘        └──────────┘       └─────────────┘
```

## 参照ドキュメント

- 画像API設計計画: `~/.claude/plans/compressed-snuggling-rossum.md`
- Spec一覧: `specs/` ディレクトリ
- 開発ガイドライン: `CLAUDE.md`
- 品質基準: `memory/constitution.md`
