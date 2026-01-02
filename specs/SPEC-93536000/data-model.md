# データモデル: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03

## 新規型定義

### GpuBackend 列挙型

ノードのGPUバックエンド種別を表す。

**ファイル**: `common/src/types.rs`

| バリアント | 説明 | プラットフォーム |
|-----------|------|------------------|
| `Metal` | Apple Metal | macOS |
| `Cuda` | NVIDIA CUDA | Linux/Windows |
| `DirectML` | DirectX Machine Learning | Windows |
| `ROCm` | AMD ROCm | Linux |
| `Cpu` | CPU演算のみ | 全プラットフォーム |

**シリアライズ**: `snake_case` (例: `"metal"`, `"cuda"`, `"directml"`)

## 既存型の拡張

### Node 構造体

**ファイル**: `common/src/types.rs`

**追加フィールド**:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `gpu_backend` | `Option<GpuBackend>` | 検出されたGPUバックエンド |
| `executable_models` | `Vec<String>` | このノードで実行可能なモデルID一覧 |

### RegisterRequest 構造体

**ファイル**: `router/src/api/nodes.rs`

**追加フィールド**:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `gpu_backend` | `Option<GpuBackend>` | ノードが自己申告するGPUバックエンド |

### HealthCheckRequest 構造体

**ファイル**: `common/src/protocol.rs`

**追加フィールド**:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `executable_models` | `Vec<String>` | GPU互換モデル一覧 |
| `gpu_backend` | `Option<GpuBackend>` | GPUバックエンド |

## データベーススキーマ

### nodes テーブル拡張

```sql
ALTER TABLE nodes ADD COLUMN gpu_backend TEXT;
-- "metal", "cuda", "directml", "rocm", "cpu"
```

### node_executable_models テーブル（新規）

```sql
CREATE TABLE node_executable_models (
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    model_id TEXT NOT NULL,
    PRIMARY KEY (node_id, model_id)
);
```

## プラットフォーム文字列

モデルの `platforms` フィールドで使用される文字列:

| 文字列 | 対応 GpuBackend |
|--------|----------------|
| `macos-metal` | `Metal` |
| `linux-cuda` | `Cuda` |
| `windows-cuda` | `Cuda` |
| `windows-directml` | `DirectML` |
| `linux-rocm` | `ROCm` |
| `cpu` | `Cpu` |

## GPU互換性判定ロジック

```text
isCompatible(model, backend):
  platform_map = {
    Metal: ["macos-metal"],
    Cuda: ["linux-cuda", "windows-cuda"],
    DirectML: ["windows-directml"],
    ROCm: ["linux-rocm"],
    Cpu: ["cpu"]
  }

  required_platforms = platform_map[backend]
  return any(p in model.platforms for p in required_platforms)
```

## APIレスポンス形式

### Node `/v1/models` レスポンス

```json
{
  "object": "list",
  "gpu_backend": "metal",
  "data": [
    {
      "id": "llama2-7b-q4",
      "object": "model",
      "platforms": ["macos-metal", "linux-cuda"]
    }
  ]
}
```

### Router `/v1/models` レスポンス

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama2-7b-q4",
      "object": "model",
      "created": 1704240000,
      "owned_by": "llm-router"
    }
  ]
}
```

## エラーレスポンス

### 対応ノードなし (503)

```json
{
  "error": {
    "message": "No available nodes support model: llama2-7b",
    "type": "service_unavailable",
    "code": "no_capable_nodes"
  }
}
```

### モデル存在しない (404)

```json
{
  "error": {
    "message": "The model 'unknown-model' does not exist",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```
