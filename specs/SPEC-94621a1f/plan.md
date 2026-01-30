# 実装計画: ノード自己登録システム

**機能ID**: `SPEC-94621a1f` | **日付**: 2025-10-31 | **仕様**: [spec.md](./spec.md)
**ステータス**: ✅ 実装済み
**元のSPEC**: SPEC-32e2b31a から分割

## 概要

ノードは起動時にロードバランサーへ自己登録し、`runtime_token` を受け取る。以降は定期的にヘルスチェック（ハートビート＋メトリクス）を送信し、ロードバランサーはノード状態（Online/Offline/initializing 等）と負荷情報を更新する。

## API（実装済み）

- `POST /api/nodes` - ノード登録（GPU必須）
- `GET /api/nodes` - ノード一覧
- `POST /api/health` - ヘルスチェック受信（`X-Node-Token` 必須）

## 実装の要点

### 登録（POST /api/nodes）

- **GPU必須**:
  - `gpu_available=true`
  - `gpu_devices` が空の場合は拒否
- **登録時の到達性確認**:
  - ノードの OpenAI互換API を `http://{ip}:{runtime_port+1}` とみなし、`GET /v1/models` で疎通確認
  - テスト実行時（`cfg!(test)`）のみスキップ
- **レスポンス**:
  - `runtime_id`（UUID）
  - `runtime_token`（以降の `/api/health` 用）

### ヘルスチェック（POST /api/health）

- **認証**:
  - ヘッダー `X-Node-Token: <token>`
- **ボディ**:
  - `HealthCheckRequest`（`runtime_id`、CPU/メモリ/GPU、`loaded_models`、`initializing` など）
- **動作**:
  - `last_seen` 更新
  - ロードマネージャーにメトリクス記録

## 主要コード

- `common/src/protocol.rs`: `RegisterRequest`, `RegisterResponse`, `HealthCheckRequest`
- `llmlb/src/api/nodes.rs`: `register_node`, `list_nodes`
- `llmlb/src/api/health.rs`: `health_check`
- `llmlb/src/registry/mod.rs`: ノード状態管理（DB同期）
- `llmlb/src/auth/middleware.rs`: `runtime_token_auth_middleware`（`X-Node-Token`）
- `node/src/api/router_client.cpp`: `/api/nodes` 登録 + `/api/health` 送信

## リクエスト例

### POST /api/nodes

```json
{
  "machine_name": "server-01",
  "ip_address": "192.168.1.100",
  "runtime_version": "0.1.0",
  "runtime_port": 32768,
  "gpu_available": true,
  "gpu_devices": [
    { "model": "NVIDIA RTX 4090", "count": 1 }
  ]
}
```

### POST /api/health

Headers:

- `X-Node-Token: <runtime_token>`

Body:

```json
{
  "runtime_id": "123e4567-e89b-12d3-a456-426614174000",
  "cpu_usage": 12.3,
  "memory_usage": 45.6,
  "active_requests": 0,
  "average_response_time_ms": 110.0,
  "loaded_models": ["gpt-oss-20b"],
  "loaded_embedding_models": [],
  "initializing": false,
  "ready_models": [1, 1]
}
```
