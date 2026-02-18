# リサーチ: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-617247d2` | **日付**: 2024-12-14

## 1. 音声認識ランタイム (ASR)

### 決定: whisper.cpp

**理由**:

- llama.cppと同じ作者（Georgi Gerganov）によるプロジェクト
- ggml基盤で統一されており、既存のビルドシステムと親和性が高い
- Metal (macOS) と CUDA (Linux) の両方をネイティブサポート
- OpenAI Whisperモデル（tiny〜large-v3）を直接実行可能

**検討した代替案**:

| 代替案 | 却下理由 |
|--------|---------|
| faster-whisper | Python依存、C++統合が複雑 |
| Vosk | 精度がWhisperに劣る、大語彙モデルが大きい |
| OpenAI API | 外部依存、レイテンシ、コスト |
| sherpa-onnx | 追加ランタイム必要、whisper.cppの方がシンプル |

**統合方法**:

```cmake
# node/CMakeLists.txt
add_subdirectory(third_party/whisper.cpp)
target_link_libraries(xllm PRIVATE whisper)
```

## 2. 音声合成ランタイム (TTS)

### 決定: ONNX Runtime

**理由**:

- 多くのTTSモデル（VITS, Tacotron2, VibeVoice等）がONNX形式で配布
- GPU加速（CUDA, DirectML, CoreML）対応
- C++ APIが充実しており統合が容易
- Microsoft公式サポートで長期メンテナンス保証

**検討した代替案**:

| 代替案 | 却下理由 |
|--------|---------|
| llama.cpp (音声) | TTSモデルのサポートなし |
| TensorRT | NVIDIA専用、移植性なし |
| PyTorch C++ | ビルド複雑、バイナリサイズ大 |
| espeak-ng | 品質が低い（合成音声） |

**統合方法**:

```cmake
# node/CMakeLists.txt
find_package(onnxruntime REQUIRED)
target_link_libraries(xllm PRIVATE onnxruntime)
```

## 3. 音声フォーマット処理

### 決定: libsndfile + ffmpeg (オプション)

**理由**:

- libsndfileは軽量でWAV/FLAC/OGGの読み書きに対応
- MP3デコードはffmpegまたはminimp3で対応
- whisper.cppは内部で16kHz PCMを期待

**実装アプローチ**:

```cpp
// 入力: 任意のフォーマット
// 出力: 16kHz mono float32 PCM (whisper.cpp用)
std::vector<float> decode_audio(const std::string& path);
```

## 4. API設計

### 決定: OpenAI Audio API互換

**理由**:

- 既存のOpenAIクライアントライブラリがそのまま使用可能
- 仕様が明確に文書化されている
- llmlbの既存OpenAI互換パターンと一貫性

**エンドポイント**:

| パス | メソッド | 用途 |
|------|---------|-----|
| `/v1/audio/transcriptions` | POST | ASR (音声→テキスト) |
| `/v1/audio/speech` | POST | TTS (テキスト→音声) |
| `/v1/audio/translations` | POST | ASR + 翻訳 (将来) |

## 5. ノード能力申告

### 決定: RuntimeType列挙型を追加

**理由**:

- ノードが対応可能なモデルタイプを明示的に申告
- ロードバランサーが適切なノードを選択可能
- 既存のHealthCheckRequestを拡張

**実装**:

```rust
// common/src/types.rs
pub enum RuntimeType {
    LlamaCpp,      // テキスト生成
    WhisperCpp,    // 音声認識
    OnnxRuntime,   // TTS/汎用
}

// HealthCheckRequest拡張
pub struct HealthCheckRequest {
    // 既存フィールド...
    pub supported_runtimes: Vec<RuntimeType>,
    pub loaded_audio_models: Vec<String>,
}
```

## 6. GPUメモリ管理

### 決定: ランタイム別コンテキスト分離

**理由**:

- llama.cppとwhisper.cppは両方ggml使用、メモリ競合の可能性
- ONNX Runtimeは独自のGPUメモリ管理
- 同時実行時の安定性確保が必要

**実装アプローチ**:

1. **シンプル版**: 音声処理中はLLM推論をキューイング
2. **高度版**: GPUメモリをランタイムごとに割り当て制限

**初期実装**: シンプル版を採用（Phase 1ではスループットより安定性優先）

## 7. モデル管理

### 決定: 既存のモデル管理システムを拡張

**理由**:

- 新しいシステムを作るより既存拡張が低リスク
- HuggingFaceからのダウンロードフローを再利用
- ダッシュボードUIも既存のものを拡張

**変更点**:

- ModelInfo構造体にruntime_typeフィールド追加
- モデル登録時にランタイム自動判定
- 音声モデル用のバリデーション追加

## 8. パフォーマンス目標

| 指標 | 目標値 | 測定方法 |
|------|--------|---------|
| ASR速度 | 2x realtime | 10秒音声を5秒以内に処理 |
| TTS速度 | 100文字/3秒 | 100文字入力から3秒以内に音声生成 |
| 並列処理 | 10件同時 | 10件の音声リクエストを並列処理 |
| 認識精度 | WER 5%以下 | 標準テストセットで測定 |

## 参考リソース

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- [ONNX Runtime](https://onnxruntime.ai/)
- [OpenAI Audio API](https://platform.openai.com/docs/api-reference/audio)
- [VibeVoice](https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B)
