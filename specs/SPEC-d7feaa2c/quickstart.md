# クイックスタート: Nodeエンジンローダー抽象化

## 前提条件

| 項目 | 要件 |
|------|------|
| OS | macOS (Metal) / Windows (CUDA) |
| GPU | Metal 対応 / NVIDIA CUDA 対応 |
| プラグイン | `engines/` 配下に配置済み |

> DirectML は凍結。Windows は CUDA 主経路。

## 1. プラグインディレクトリ構成

デフォルトは `~/.llm-router/engines`（Windows は `%USERPROFILE%\.llm-router\engines`）。
変更する場合は `LLM_NODE_ENGINE_PLUGINS_DIR` を指定する。

```
~/.llm-router/engines/
  gptoss/
    manifest.json
    llm_engine_gptoss.*
  nemotron/
    manifest.json
    llm_engine_nemotron.*
```

## 2. manifest.json の例

```json
{
  "engine_id": "gptoss_cpp",
  "engine_version": "0.1.0",
  "abi_version": 2,
  "runtimes": ["gptoss_cpp"],
  "formats": ["safetensors"],
  "architectures": ["gpt_oss"],
  "modalities": ["completion"],
  "capabilities": ["text"],
  "gpu_targets": ["cuda"],
  "library": "llm_engine_gptoss"
}
```

## 3. Node 起動

```bash
# 例: プラグインディレクトリを明示
LLM_NODE_ENGINE_PLUGINS_DIR=~/.llm-router/engines \
  ./llm-router-node
```

起動ログに `Engine plugins loaded from ...` が出ていればロード成功。

## 4. 確認

- `/v1/models` に対象モデルが `ready` で表示されること。
- DLL やアーティファクト不足の場合は明確なエラーが返ること。

## 5. 失敗時のチェック

- `manifest.json` の `abi_version` が `EngineHost::kAbiVersion` と一致しているか
- `library` 名に対応する共有ライブラリが配置されているか
- `gpu_targets` がビルド済みのバックエンド（Metal/CUDA）と一致しているか
