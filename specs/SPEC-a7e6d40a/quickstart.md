# クイックスタート: CLI インターフェース整備

## 概要

Router/NodeのCLIインターフェースの使用方法を説明する。

## Router CLI

### ヘルプ表示

```bash
llm-router --help
# または
llm-router -h
```

### バージョン表示

```bash
llm-router --version
# または
llm-router -V
```

出力例:

```text
llm-router 2.1.0
```

### サーバー起動（デフォルト）

```bash
# 引数なしで起動
llm-router
```

デフォルト設定:

- ポート: 8080
- ホスト: 0.0.0.0
- ログレベル: info

### 環境変数による設定

```bash
# ポートを変更
LLM_ROUTER_PORT=3000 llm-router

# ログレベルを変更
LLM_ROUTER_LOG_LEVEL=debug llm-router

# 複数設定
LLM_ROUTER_PORT=3000 \
LLM_ROUTER_LOG_LEVEL=debug \
LLM_ROUTER_JWT_SECRET=my-secret-key \
llm-router
```

## Node CLI

### ヘルプ表示

```bash
allm --help
# または
allm -h
```

### バージョン表示

```bash
allm --version
# または
allm -V
```

出力例:

```text
allm 0.1.0
```

### サーバー起動

```bash
# 引数なしで起動
allm
```

### 環境変数による設定

```bash
# ルーターURLを指定
LLM_ROUTER_URL=http://192.168.1.100:8080 allm

# モデルディレクトリを指定
ALLM_MODELS_DIR=/data/models allm

# 複数設定
LLM_ROUTER_URL=http://router:8080 \
ALLM_PORT=11436 \
ALLM_LOG_LEVEL=debug \
allm
```

## JWT_SECRET の自動管理

### 初回起動時

```bash
# 初回起動時、JWT_SECRETが自動生成される
llm-router

# 生成されたシークレットを確認
cat ~/.llm-router/jwt_secret
```

### 環境変数で上書き

```bash
# Kubernetes等での運用時
LLM_ROUTER_JWT_SECRET=$(cat /secrets/jwt-secret) llm-router
```

## 廃止されたコマンド

以下のコマンドは廃止されました。代わりにAPI/Dashboardを使用してください。

### ユーザー管理（廃止）

```bash
# 廃止: llm-router user list
# 代替: API経由
curl http://localhost:8080/v0/users \
  -H "Authorization: Bearer $TOKEN"

# 廃止: llm-router user add admin
# 代替: API経由
curl -X POST http://localhost:8080/v0/users \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password123"}'
```

### モデル管理（廃止）

```bash
# 廃止: llm-router model list
# 代替: API経由
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug"

# 廃止: llm-router model add llama-3.1-8b
# 代替: Dashboard経由でモデル登録
```

## トラブルシューティング

### 不正なオプションを指定した場合

```bash
llm-router --unknown-option
```

出力:

```text
error: unexpected argument '--unknown-option' found

Usage: llm-router [OPTIONS]

For more information, try '--help'.
```

### 必須環境変数が未設定の場合

```bash
# 初回起動時、ADMIN_PASSWORDが必須
llm-router
```

出力:

```text
Error: LLM_ROUTER_ADMIN_PASSWORD is required on first run
```

### ポート競合の場合

```bash
LLM_ROUTER_PORT=8080 llm-router
```

出力:

```text
Error: Failed to bind to 0.0.0.0:8080: Address already in use
```

## 環境変数一覧

### Router

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `LLM_ROUTER_PORT` | 8080 | 待受ポート |
| `LLM_ROUTER_HOST` | 0.0.0.0 | 待受アドレス |
| `LLM_ROUTER_LOG_LEVEL` | info | ログレベル |
| `LLM_ROUTER_JWT_SECRET` | (自動生成) | JWT署名キー |
| `LLM_ROUTER_ADMIN_USERNAME` | admin | 初期管理者名 |
| `LLM_ROUTER_ADMIN_PASSWORD` | (必須) | 初期管理者パスワード |

### Node

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `LLM_ROUTER_URL` | `http://127.0.0.1:8080` | ルーターURL |
| `ALLM_PORT` | 11435 | 待受ポート |
| `ALLM_MODELS_DIR` | ~/.runtime/models | モデル保存先 |
| `ALLM_LOG_LEVEL` | info | ログレベル |
| `ALLM_HEARTBEAT_SECS` | 10 | ハートビート間隔 |
