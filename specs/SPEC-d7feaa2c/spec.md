# SPEC-d7feaa2c: aLLMエンジン統合アーキテクチャ

**ステータス**: 実装中

## 背景 / 問題

aLLMは複数の推論ライブラリ（llama.cpp、safetensors.cpp、whisper.cpp、stable-diffusion.cpp）を
統合する必要がある。当初は動的プラグインシステムを検討したが、以下の理由により方針を転換する：

- 後から追加するユースケースがない（固定的なエンジン構成）
- 動的ロードの複雑さ（ABI互換性、manifest管理等）が不要
- シンプルさと保守性を優先

## 目的

- aLLMでモダリティ別マネージャー方式を採用し、推論ライブラリを静的に統合する
- テキスト生成はモデル形式（GGUF/safetensors）で使用ライブラリを振り分ける
- Responses APIを推奨API、Chat Completion APIは後方互換として維持

## システム全体像

### aLLM（C++製推論エンジン）

```text
aLLM
├── API層（OpenAI互換 + Responses API）
│   ├── /v1/responses          # Responses API（推奨）
│   ├── /v1/chat/completions   # Chat Completion API（後方互換）
│   ├── /v1/audio/transcriptions
│   └── /v1/images/generations
│
├── マネージャー層（モダリティ別）
│   ├── TextManager            # テキスト生成
│   ├── AudioManager           # 音声認識
│   └── ImageManager           # 画像生成
│
└── ライブラリ層（静的リンク）
    ├── llama.cpp              # GGUF形式
    ├── safetensors.cpp        # safetensors形式
    ├── whisper.cpp            # 音声認識
    └── stable-diffusion.cpp   # 画像生成
```

### モダリティ別マネージャー

| マネージャー | 担当モダリティ | 使用ライブラリ | API |
|-------------|---------------|---------------|-----|
| TextManager | テキスト生成 | llama.cpp / safetensors.cpp | /v1/responses, /v1/chat/completions |
| AudioManager | 音声認識 | whisper.cpp | /v1/audio/transcriptions |
| ImageManager | 画像生成 | stable-diffusion.cpp | /v1/images/generations |

### TextManager内の形式振り分け

```text
TextManager
├── モデル形式判定（config.json / ファイル拡張子）
│
├── GGUF形式 ──────────► llama.cpp
│   └── Llama, Mistral, Qwen, Phi, Gemma等
│
└── safetensors形式 ───► safetensors.cpp
    └── Llama, Qwen, Nemotron, GPT-OSS等
        └── アーキテクチャ対応はsafetensors.cpp内で実装
```

## API方針

### Responses API（推奨）

新規開発ではResponses APIを推奨する。

```json
POST /v1/responses
{
  "model": "nemotron-3-8b",
  "input": "Hello, world!",
  "instructions": "You are a helpful assistant."
}
```

### Chat Completion API（後方互換）

既存クライアントとの互換性のため維持する。

```json
POST /v1/chat/completions
{
  "model": "nemotron-3-8b",
  "messages": [
    {"role": "user", "content": "Hello, world!"}
  ]
}
```

## ゴール

- モダリティ別マネージャー（Text/Audio/Image）が実装される
- TextManagerがGGUF/safetensors形式を自動判定し、適切なライブラリで推論を実行する
- 動的プラグインシステムは使用しない（静的リンク）
- Responses APIがテキスト生成の推奨エンドポイントになる

## 非ゴール

- 動的プラグインシステム（廃止）
- サードパーティエンジンの動的追加
- ABI互換性管理
- manifest.jsonによるエンジン定義

## アーキテクチャ（詳細）

### TextManager

テキスト生成を担当。モデル形式に応じてllama.cppまたはsafetensors.cppを使用。

```cpp
class TextManager {
public:
    // モデルロード（形式を自動判定）
    bool load_model(const std::string& model_path);

    // 推論実行
    GenerationResult generate(const GenerationRequest& request);

private:
    // 形式判定
    ModelFormat detect_format(const std::string& model_path);

    // llama.cpp（GGUF用）
    llama_context* llama_ctx_;

    // safetensors.cpp（safetensors用）
    stcpp::GgmlModel* stcpp_model_;
    stcpp::GgmlContext* stcpp_ctx_;
};
```

### 形式判定ロジック

1. `.gguf`ファイルが存在 → GGUF形式 → llama.cpp
2. `*.safetensors`または`*.safetensors.index.json`が存在 → safetensors形式 → safetensors.cpp
3. 両方存在する場合 → 登録時の指定に従う（デフォルトはsafetensors）

### AudioManager

音声認識を担当。whisper.cppを使用。

```cpp
class AudioManager {
public:
    bool load_model(const std::string& model_path);
    TranscriptionResult transcribe(const AudioRequest& request);

private:
    whisper_context* whisper_ctx_;
};
```

### ImageManager

画像生成を担当。stable-diffusion.cppを使用。

```cpp
class ImageManager {
public:
    bool load_model(const std::string& model_path);
    ImageResult generate(const ImageRequest& request);

private:
    sd_ctx_t* sd_ctx_;
};
```

## safetensors.cppのアーキテクチャ対応

safetensors.cpp内で複数のモデルアーキテクチャをサポート：

| アーキテクチャ | 対応状況 | config.json model_type |
|---------------|---------|----------------------|
| Llama | 対応済み | "llama" |
| Mistral | 対応済み | "mistral" |
| Qwen/Qwen2 | 対応済み | "qwen", "qwen2" |
| Phi | 対応済み | "phi", "phi3" |
| Gemma | 対応済み | "gemma", "gemma2" |
| Nemotron | 対応済み | "nemotron" |
| GPT-OSS | 対応済み | "gpt-oss" |
| GLM | 計画中 | "glm" |

アーキテクチャ検出は`config.json`の`model_type`フィールドで行う。

## GPU対応

GPUバックエンドはggmlが提供。マネージャー層では意識しない。

| OS | GPUバックエンド |
|----|----------------|
| macOS | Metal（Apple Silicon） |
| Windows | CUDA |
| Linux | CUDA / ROCm / Vulkan |

## エラーコード

粗粒度の共通エラーコード（約10種類）を定義。

| コード | 名前 | 説明 |
|--------|------|------|
| 0 | OK | 成功 |
| 1 | OOM_VRAM | VRAM不足 |
| 2 | OOM_RAM | RAM不足 |
| 3 | MODEL_CORRUPT | モデルファイル破損 |
| 4 | TIMEOUT | タイムアウト |
| 5 | CANCELLED | キャンセル |
| 6 | UNSUPPORTED | 未サポート機能/アーキテクチャ |
| 7 | INTERNAL | 内部エラー |
| 8 | FORMAT_MISMATCH | モデル形式不一致 |
| 9 | LOAD_FAILED | ロード失敗 |

## 受け入れ条件

- [ ] TextManager, AudioManager, ImageManagerが実装される
- [ ] TextManagerがGGUF/safetensors形式を自動判定する
- [ ] Responses APIエンドポイントが実装される
- [ ] 既存のプラグインシステムコード（allm/engines/）が削除される
- [ ] テスト（unit/integration）が更新される

## 移行計画

### Phase 1: マネージャー層の実装

1. TextManager実装（llama.cpp + safetensors.cpp統合）
2. AudioManager実装（既存whisper_managerをリファクタ）
3. ImageManager実装（既存sd_managerをリファクタ）

### Phase 2: プラグインシステムの削除

1. `allm/engines/`ディレクトリの削除
2. EngineRegistry, EngineHost関連コードの削除
3. manifest.json関連コードの削除

### Phase 3: API層の整備

1. Responses APIエンドポイントの実装
2. Chat Completion APIとの統合
3. テスト更新

---

## 変更履歴

### 2026-01-19: 方針転換

- **動的プラグインシステムを廃止**
  - 理由: 後から追加するユースケースがなく、複雑さが不要
- **モダリティ別マネージャー方式を採用**
  - TextManager（llama.cpp + safetensors.cpp）
  - AudioManager（whisper.cpp）
  - ImageManager（stable-diffusion.cpp）
- **Responses APIを推奨APIに設定**
  - Chat Completion APIは後方互換として維持

### 以前の仕様（廃止）

以下の概念は廃止：

- Engine Plugin（動的ロード）
- Engine Host（プラグインローダー）
- manifest.json（プラグイン定義）
- EngineRegistry（ランタイム解決）
- ABI互換性管理
- サードパーティプラグインサポート
