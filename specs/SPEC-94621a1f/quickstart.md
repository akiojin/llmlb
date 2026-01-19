# ノード自己登録システム クイックスタート

**SPEC-ID**: SPEC-94621a1f
**ステータス**: ✅ 実装済み

## 前提条件

- Rust（`cargo`）がインストールされている
- ルーターが起動している（`cargo run -p llm-router`）

## ノード登録（curlで確認）

> NOTE: ルーターは登録時にノードの OpenAI互換API（`runtime_port+1`）へ疎通確認を行います。
> 実際のノード（allm）が起動している必要があります。

### 1. ルーター起動

```bash
cargo run -p llm-router
```

### 2. ノード登録

```bash
REGISTER_RES=$(curl -sS http://localhost:32768/v0/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "machine_name": "server-01",
    "ip_address": "127.0.0.1",
    "runtime_version": "0.1.0",
    "runtime_port": 32768,
    "gpu_available": true,
    "gpu_devices": [{"model":"NVIDIA RTX 4090","count":1}]
  }')

echo "$REGISTER_RES" | jq .
```

> NOTE: 登録直後のノードは `pending` として保存されます。運用対象にするには承認が必要です。

### 3. 管理者ログイン（JWT取得）

```bash
LOGIN_RES=$(curl -sS http://localhost:32768/v0/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "test"
  }')

echo "$LOGIN_RES" | jq .
JWT_TOKEN=$(echo "$LOGIN_RES" | jq -r .token)
```

### 4. ノード承認

```bash
NODE_ID=$(echo "$REGISTER_RES" | jq -r .runtime_id)

curl -sS http://localhost:32768/v0/nodes/${NODE_ID}/approve \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ${JWT_TOKEN}" | jq .
```

### 5. ノード一覧確認

```bash
curl -sS http://localhost:32768/v0/nodes | jq .
```

## ヘルスチェック送信（curlで確認）

`POST /v0/nodes` のレスポンスに含まれる `runtime_token` を使って `POST /v0/health` を呼び出します。

```bash
NODE_ID=$(echo "$REGISTER_RES" | jq -r .runtime_id)
NODE_TOKEN=$(echo "$REGISTER_RES" | jq -r .runtime_token)

curl -sS http://localhost:32768/v0/health \
  -H "Content-Type: application/json" \
  -H "X-Node-Token: ${NODE_TOKEN}" \
  -d "{
    \"runtime_id\": \"${NODE_ID}\",
    \"cpu_usage\": 12.3,
    \"memory_usage\": 45.6,
    \"active_requests\": 0,
    \"average_response_time_ms\": 110.0,
    \"loaded_models\": [\"gpt-oss-20b\"],
    \"loaded_embedding_models\": [],
    \"initializing\": false,
    \"ready_models\": [1, 1]
  }" | jq .
```

## 次のステップ

- `SPEC-63acef08`: OpenAI互換プロキシ（`/v1/*`）
- `SPEC-443acc8c`: ヘルスチェック（Online/Offline判定）
- `SPEC-712c20cf`: 管理ダッシュボード（`/dashboard`）
