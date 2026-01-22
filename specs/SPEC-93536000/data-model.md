# データモデル: ノードベースモデル管理とモデル対応ルーティング

**機能ID**: `SPEC-93536000`
**作成日**: 2026-01-03
**更新日**: 2026-01-03

## 設計原則

- **シンプルさ優先**: ロードバランサーはGPUバックエンド情報を持たない
- **メモリのみ**: DB永続化なし、ロードバランサー再起動時はノード再登録で復元
- **プル型**: ロードバランサーがノード登録時に/v1/modelsを取得

## Node側の型定義（C++）

### GpuBackend 列挙型

ノードのGPUバックエンド種別を表す。**ノード側のみで使用**。

**ファイル**: `node/src/system/gpu_detector.hpp`

| バリアント | 説明 | プラットフォーム |
|-----------|------|------------------|
| `Metal` | Apple Metal | macOS |
| `Cuda` | NVIDIA CUDA | Linux/Windows |
| `DirectML` | DirectX Machine Learning | Windows |
| `ROCm` | AMD ROCm | Linux |
| `Cpu` | CPU演算のみ | 全プラットフォーム |

## Load Balancer側の型拡張（Rust）

### Node 構造体

**ファイル**: `llmlb/src/registry/mod.rs`

**追加フィールド**:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `executable_models` | `Vec<String>` | このノードで実行可能なモデルID一覧 |
| `excluded_models` | `HashSet<String>` | 推論失敗により一時除外中のモデルID |

**注意**: `gpu_backend` フィールドは不要（ロードバランサーはGPU情報を保持しない）

### RegisterRequest / HealthCheckRequest

**変更なし** - 既存の構造体をそのまま使用

## データベーススキーマ

**変更なし** - executable_modelsはメモリのみで管理

## プラットフォーム文字列（ノード側）

モデルの `platforms` フィールドで使用される文字列（ノード側のみ）:

| 文字列 | 対応 GpuBackend |
|--------|----------------|
| `macos-metal` | `Metal` |
| `linux-cuda` | `Cuda` |
| `windows-cuda` | `Cuda` |
| `windows-directml` | `DirectML` |
| `linux-rocm` | `ROCm` |
| `cpu` | `Cpu` |

## GPU互換性判定ロジック（ノード側）

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

**シンプルなモデルID一覧のみ**（GPU情報は含まない）。

**モデルエントリの検証ルール**:

- 「id」フィールドのみ必須、「object」フィールドはオプショナル
- ID空・null・未定義のエントリはスキップ
- 同じIDが複数回出現した場合は重複排除
- 有効なエントリが1つ以上あれば登録を継続

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama2-7b-q4",
      "object": "model"
    },
    {
      "id": "llama3-8b-q4",
      "object": "model"
    }
  ]
}
```

### Load Balancer `/v1/models` レスポンス

オンラインノードのモデルを集約:

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama2-7b-q4",
      "object": "model",
      "created": 1704240000,
      "owned_by": "llmlb"
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

### ノード登録拒否（/v1/models取得失敗）

```json
{
  "error": {
    "message": "Failed to fetch model list from node: connection timeout",
    "type": "registration_error",
    "code": "model_list_unavailable"
  }
}
```

### ノード登録拒否（空のモデルリスト）

```json
{
  "error": {
    "message": "Node reported no executable models",
    "type": "registration_error",
    "code": "no_executable_models"
  }
}
```

## モデル除外の状態遷移

```text
[正常] --推論失敗--> [除外中] --ノード再起動--> [正常]
                         |
                         +--> ノードオフライン --> [除外状態リセット]
                         |
                         +--> ノード再登録 --> [除外状態リセット]
```

- 推論失敗: 1回で即座に除外
- 復帰条件: ノード再起動（再登録）のみ
- ノードオフライン時: 除外状態も含めてクリア
- ノード再登録時: excluded_modelsをクリアし新しいモデル一覧で更新
- 進行中リクエスト: 除外は新規リクエストのみに影響、既存リクエストは継続
