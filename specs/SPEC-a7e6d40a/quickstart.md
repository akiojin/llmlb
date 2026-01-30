# クイックスタート: CLI インターフェース整備

## 概要

Load Balancer/NodeのCLIインターフェースの使用方法を説明する。

## Load Balancer CLI

### ヘルプ表示

```bash
llmlb --help
# または
llmlb -h
```

### バージョン表示

```bash
llmlb --version
# または
llmlb -V
```

出力例:

```text
llmlb 2.1.0
```

### サーバー起動（デフォルト）

```bash
# 引数なしで起動
llmlb
```

デフォルト設定:

- ポート: 8080
- ホスト: 0.0.0.0
- ログレベル: info

### 環境変数による設定

```bash
# ポートを変更
LLMLB_PORT=3000 llmlb

# ログレベルを変更
LLMLB_LOG_LEVEL=debug llmlb

# 複数設定
LLMLB_PORT=3000 \
LLMLB_LOG_LEVEL=debug \
LLMLB_JWT_SECRET=my-secret-key \
llmlb
```

## Node CLI

### ヘルプ表示

```bash
xllm --help
# または
xllm -h
```

### バージョン表示

```bash
xllm --version
# または
xllm -V
```

出力例:

```text
xllm 0.1.0
```

### サーバー起動

```bash
# 引数なしで起動
xllm
```

### 環境変数による設定

```bash
# ロードバランサーURLを指定
LLMLB_URL=http://192.168.1.100:8080 xllm

# モデルディレクトリを指定
XLLM_MODELS_DIR=/data/models xllm

# 複数設定
LLMLB_URL=http://router:8080 \
XLLM_PORT=11436 \
XLLM_LOG_LEVEL=debug \
xllm
```

## JWT_SECRET の自動管理

### 初回起動時

```bash
# 初回起動時、JWT_SECRETが自動生成される
llmlb

# 生成されたシークレットを確認
cat ~/.llmlb/jwt_secret
```

### 環境変数で上書き

```bash
# Kubernetes等での運用時
LLMLB_JWT_SECRET=$(cat /secrets/jwt-secret) llmlb
```

## 廃止されたコマンド

以下のコマンドは廃止されました。代わりにAPI/Dashboardを使用してください。

### ユーザー管理（廃止）

```bash
# 廃止: llmlb user list
# 代替: API経由
curl http://localhost:8080/api/users \
  -H "Authorization: Bearer $TOKEN"

# 廃止: llmlb user add admin
# 代替: API経由
curl -X POST http://localhost:8080/api/users \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "password123"}'
```

### モデル管理（廃止）

```bash
# 廃止: llmlb model list
# 代替: API経由
curl http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug"

# 廃止: llmlb model add llama-3.1-8b
# 代替: Dashboard経由でモデル登録
```

## トラブルシューティング

### 不正なオプションを指定した場合

```bash
llmlb --unknown-option
```

出力:

```text
error: unexpected argument '--unknown-option' found

Usage: llmlb [OPTIONS]

For more information, try '--help'.
```

### 必須環境変数が未設定の場合

```bash
# 初回起動時、ADMIN_PASSWORDが必須
llmlb
```

出力:

```text
Error: LLMLB_ADMIN_PASSWORD is required on first run
```

### ポート競合の場合

```bash
LLMLB_PORT=8080 llmlb
```

出力:

```text
Error: Failed to bind to 0.0.0.0:8080: Address already in use
```

## 環境変数一覧

### Load Balancer

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `LLMLB_PORT` | 8080 | 待受ポート |
| `LLMLB_HOST` | 0.0.0.0 | 待受アドレス |
| `LLMLB_LOG_LEVEL` | info | ログレベル |
| `LLMLB_JWT_SECRET` | (自動生成) | JWT署名キー |
| `LLMLB_ADMIN_USERNAME` | admin | 初期管理者名 |
| `LLMLB_ADMIN_PASSWORD` | (必須) | 初期管理者パスワード |

### Node

| 環境変数 | デフォルト | 説明 |
|----------|-----------|------|
| `LLMLB_URL` | `http://127.0.0.1:8080` | ロードバランサーURL |
| `XLLM_PORT` | 11435 | 待受ポート |
| `XLLM_MODELS_DIR` | ~/.runtime/models | モデル保存先 |
| `XLLM_LOG_LEVEL` | info | ログレベル |
| `XLLM_HEARTBEAT_SECS` | 10 | ハートビート間隔 |
