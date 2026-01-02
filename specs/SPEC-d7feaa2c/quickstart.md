# クイックスタート: Nodeエンジンローダー抽象化

## 前提条件

| 項目 | 要件 |
|------|------|
| プラットフォーム | macOS (Metal) または Windows (DirectML) |
| GPU | Apple Silicon または DirectX 12対応GPU |
| プラグイン | `engines/` 配下に配置済み |

## 基本的な使用例

### プラグインディレクトリ構成

```bash
# macOS (Metal) の場合
ls engines/llama_cpp/metal/

# 出力例:
# manifest.json
# libllama_engine.dylib
```

### manifest.json の確認

```bash
cat engines/llama_cpp/metal/manifest.json
```

```json
{
  "id": "llama_cpp",
  "version": "1.0.0",
  "abi_version": "1",
  "gpu_backend": "metal",
  "architectures": ["llama", "mistral", "gemma", "phi"],
  "formats": ["gguf"],
  "binary": "libllama_engine.dylib"
}
```

### ノードの起動

```bash
# Metal環境で起動
./llm-node --engines-dir ./engines

# カスタムVRAM上限を指定
./llm-node --engines-dir ./engines --vram-limit 8G
```

### プラグイン一覧の確認

```bash
# Node API経由でロード済みエンジンを確認
curl http://localhost:3000/api/engines

# レスポンス例:
{
  "engines": [
    {
      "id": "llama_cpp",
      "version": "1.0.0",
      "gpu_backend": "metal",
      "architectures": ["llama", "mistral", "gemma", "phi"],
      "status": "loaded"
    }
  ]
}
```

### モデルのロード

```bash
# 指定アーキテクチャのモデルをロード
curl -X POST http://localhost:3000/api/models/load \
  -H "Content-Type: application/json" \
  -d '{
    "model_id": "llama-3.2-1b",
    "format": "gguf"
  }'
```

### 推論の実行

```bash
# ストリーミング生成
curl -X POST http://localhost:3000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

## プラグイン開発

### 最小限のプラグイン実装

```c
// my_engine.c
#include "engine_api.h"

static EngineInfo info = {
    .abi_version = ENGINE_ABI_VERSION,
    .engine_id = "my_engine",
    .version = "1.0.0"
};

ENGINE_API EngineInfo* engine_get_info(void) {
    return &info;
}

ENGINE_API int engine_init(EngineConfig* config) {
    // 初期化処理
    return ERR_OK;
}

ENGINE_API int engine_load_model(const char* model_path) {
    // モデルロード処理
    return ERR_OK;
}

ENGINE_API int engine_generate(
    const char* prompt,
    TokenResult** results
) {
    // 推論処理
    return ERR_OK;
}

ENGINE_API void engine_shutdown(void) {
    // クリーンアップ処理
}
```

### ビルド（macOS）

```bash
clang -shared -fPIC -o libmy_engine.dylib my_engine.c
```

### manifest.json の作成

```json
{
  "id": "my_engine",
  "version": "1.0.0",
  "abi_version": "1",
  "gpu_backend": "metal",
  "architectures": ["custom"],
  "formats": ["safetensors"],
  "binary": "libmy_engine.dylib"
}
```

### プラグインの配置

```bash
mkdir -p engines/my_engine/metal
mv libmy_engine.dylib engines/my_engine/metal/
mv manifest.json engines/my_engine/metal/
```

## エラーハンドリング

### ABI不一致

```bash
# プラグインのABIバージョンがホストと不一致
{
  "error": {
    "message": "ABI version mismatch: expected 1, got 2",
    "type": "plugin_error",
    "code": "abi_mismatch"
  }
}
```

### アーキテクチャ不一致

```bash
# モデルアーキテクチャがプラグインで未対応
{
  "error": {
    "message": "Architecture 'nemotron' not supported by plugin 'llama_cpp'",
    "type": "unsupported_error",
    "code": "architecture_mismatch"
  }
}
```

### VRAM不足

```bash
# モデルロード時のVRAM不足
{
  "error": {
    "message": "Insufficient VRAM: required 16GB, available 8GB",
    "type": "resource_error",
    "code": "oom_vram"
  }
}
```

## 制限事項

| 項目 | 制限 |
|------|------|
| ABI互換 | 同一ABIバージョンのみ |
| GPU必須 | CPUフォールバック非対応 |
| プラグイン競合 | 同一IDは先着優先 |
| ネットワーク | プラグインからの外部通信禁止 |
| サンドボックス | なし（信頼前提） |

## 設定オプション

### 環境変数

```bash
# プラグインディレクトリ
export LLM_NODE_ENGINES_DIR=/custom/path/engines

# VRAM使用上限
export LLM_NODE_VRAM_LIMIT=8589934592  # 8GB in bytes

# リソース監視間隔
export LLM_NODE_MONITOR_INTERVAL_MS=1000
```

### コマンドラインオプション

```bash
llm-node \
  --engines-dir ./engines \
  --vram-limit 8G \
  --monitor-interval 1000
```

## トラブルシューティング

### プラグインが検出されない

```bash
# ディレクトリ構成を確認
ls -R engines/

# manifest.jsonの構文を確認
cat engines/llama_cpp/metal/manifest.json | jq .
```

### ABIエラー

```bash
# ホストのABIバージョンを確認
./llm-node --version

# プラグインのABIバージョンを確認
cat engines/llama_cpp/metal/manifest.json | jq .abi_version
```

### GPU検出失敗

```bash
# macOS: Metal対応を確認
system_profiler SPDisplaysDataType

# Windows: DirectX 12対応を確認
dxdiag
```
