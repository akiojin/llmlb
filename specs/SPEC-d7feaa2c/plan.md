# SPEC-d7feaa2c: Plan

## 方針

- aLLMにモダリティ別マネージャーを導入し、推論ライブラリを静的に統合する
- 動的プラグインシステムは廃止し、シンプルさと保守性を優先する
- テキスト生成はモデル形式（GGUF/safetensors）で使用ライブラリを振り分ける
- Responses APIを推奨API、Chat Completion APIは後方互換として維持

## 設計スコープ

### aLLM（C++製推論エンジン）

モダリティ別マネージャー方式を採用：

| マネージャー | 担当 | 使用ライブラリ |
|-------------|------|---------------|
| TextManager | テキスト生成 | llama.cpp + safetensors.cpp |
| AudioManager | 音声認識 | whisper.cpp |
| ImageManager | 画像生成 | stable-diffusion.cpp |

### Router（Rust製）

- Responses API / Chat Completion APIの提供
- エンドポイント管理・負荷分散
- モデルカタログ管理

## 実装概要

### Phase 1: マネージャー層の実装

#### TextManager

テキスト生成を担当。形式判定によりllama.cppまたはsafetensors.cppを使用。

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

    // GGUF用（llama.cpp）
    LlamaManager llama_manager_;

    // safetensors用（safetensors.cpp）
    SafetensorsManager safetensors_manager_;
};
```

**形式判定ロジック**:

1. `.gguf`ファイルが存在 → llama.cpp
2. `*.safetensors`が存在 → safetensors.cpp
3. 両方存在 → 登録時指定に従う（デフォルト: safetensors）

#### AudioManager

音声認識を担当。既存`whisper_manager`をリファクタ。

```cpp
class AudioManager {
public:
    bool load_model(const std::string& model_path);
    TranscriptionResult transcribe(const AudioRequest& request);

private:
    WhisperManager whisper_manager_;
};
```

#### ImageManager

画像生成を担当。既存`sd_manager`をリファクタ。

```cpp
class ImageManager {
public:
    bool load_model(const std::string& model_path);
    ImageResult generate(const ImageRequest& request);

private:
    SdManager sd_manager_;
};
```

### Phase 2: プラグインシステムの削除

削除対象:

- `allm/engines/` ディレクトリ全体
- `EngineRegistry` 関連コード
- `EngineHost` 関連コード
- `manifest.json` 関連コード
- `engine_plugin_api.h` 等のプラグインABI定義

### Phase 3: API層の整備

#### Responses API（推奨）

```text
POST /v1/responses
{
  "model": "nemotron-3-8b",
  "input": "Hello, world!",
  "instructions": "You are a helpful assistant."
}
```

#### Chat Completion API（後方互換）

```text
POST /v1/chat/completions
{
  "model": "nemotron-3-8b",
  "messages": [{"role": "user", "content": "Hello"}]
}
```

## safetensors.cppアーキテクチャ対応

safetensors.cpp内で複数アーキテクチャをサポート：

| アーキテクチャ | 対応状況 | config.json model_type |
|---------------|---------|----------------------|
| Llama | 対応済み | "llama" |
| Mistral | 対応済み | "mistral" |
| Qwen/Qwen2 | 対応済み | "qwen", "qwen2" |
| Phi | 対応済み | "phi", "phi3" |
| Gemma | 対応済み | "gemma", "gemma2" |
| Nemotron | 対応済み | "nemotron" |
| GPT-OSS | 対応済み | "gpt-oss" |

## エラーコード体系

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

## テスト計画

### Unit Tests

- TextManager: 形式判定ロジック
- AudioManager: 音声認識基本機能
- ImageManager: 画像生成基本機能

### Integration Tests

- モデルロード → 推論 → 結果取得のE2Eフロー
- GGUF/safetensors両形式での動作確認
- Responses API / Chat Completion API両方の動作確認

### 削除確認

- `allm/engines/`が存在しないこと
- プラグイン関連コードがビルドに含まれないこと

## 移行手順

### Step 1: TextManager実装

1. `allm/src/core/text_manager.cpp` を新規作成
2. 既存の `llama_manager` と `safetensors.cpp` を統合
3. 形式判定ロジックを実装
4. テスト作成・実行

### Step 2: AudioManager/ImageManager実装

1. 既存の `whisper_manager` を `AudioManager` にリファクタ
2. 既存の `sd_manager` を `ImageManager` にリファクタ
3. テスト作成・実行

### Step 3: プラグインシステム削除

1. `allm/engines/` ディレクトリ削除
2. 関連コード削除（EngineRegistry, EngineHost等）
3. CMakeLists.txt更新
4. ビルド確認

### Step 4: API層整備

1. Responses APIエンドポイント実装
2. 既存Chat Completion APIとの統合
3. テスト作成・実行

---

## 変更履歴

### 2026-01-19: 方針転換

- 動的プラグインシステムを廃止
- モダリティ別マネージャー方式を採用
- Responses APIを推奨APIに設定

### 以前の設計（廃止）

以下の設計は廃止：

- EngineHost（Plugin Loader）
- EngineRegistry
- manifest.json
- C ABI プラグインインターフェース
- ホットリロード機能
- サードパーティプラグインサポート
