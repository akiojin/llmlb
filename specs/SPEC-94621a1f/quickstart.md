# ノード自己登録システム クイックスタート

**SPEC-ID**: SPEC-94621a1f
**ステータス**: ✅ 実装済み

## 前提条件

- Rust（`cargo`）がインストールされている
- ルーターが起動している（`cargo run -p llm-router`）

## ノード登録（curlで確認）

> NOTE: ルーターは登録時にノードの OpenAI互換API（`runtime_port+1`）へ疎通確認を行います。
> ローカルでAPI契約だけを確認したい場合は、ルーター起動時に `LLM_ROUTER_SKIP_HEALTH_CHECK=1` を付けてください。

### 1. ルーター起動

```bash
LLM_ROUTER_SKIP_HEALTH_CHECK=1 cargo run -p llm-router
```

### 2. ノード登録

```bash
REGISTER_RES=$(curl -sS http://localhost:8080/api/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "server-01",
    "ip_address": "127.0.0.1",
    "runtime_version": "0.1.0",
    "runtime_port": 11434,
    "gpu_available": true,
    "gpu_devices": [{"model":"NVIDIA RTX 4090","count":1}]
  }')

echo "$REGISTER_RES" | jq .
```

### 3. ノード一覧確認

```bash
curl -sS http://localhost:8080/api/nodes | jq .
```

## ヘルスチェック送信（curlで確認）

`POST /api/nodes` のレスポンスに含まれる `node_token` を使って `POST /api/health` を呼び出します。

```bash
NODE_ID=$(echo "$REGISTER_RES" | jq -r .node_id)
NODE_TOKEN=$(echo "$REGISTER_RES" | jq -r .node_token)

curl -sS http://localhost:8080/api/health \
  -H "Content-Type: application/json" \
  -H "X-Node-Token: ${NODE_TOKEN}" \
  -d "{
    \"node_id\": \"${NODE_ID}\",
    \"cpu_usage\": 12.3,
    \"memory_usage\": 45.6,
    \"active_requests\": 0,
    \"average_response_time_ms\": 110.0,
    \"loaded_models\": [\"gpt-oss:20b\"],
    \"loaded_embedding_models\": [],
    \"initializing\": false,
    \"ready_models\": [1, 1]
  }" | jq .
```

## 次のステップ

- `SPEC-63acef08`: OpenAI互換プロキシ（`/v1/*`）
- `SPEC-443acc8c`: ヘルスチェック（Online/Offline判定）
- `SPEC-712c20cf`: 管理ダッシュボード（`/dashboard`）
