# クイックスタート: APIキースコープ & /v0 認証

## 1. 管理者がAPIキーを発行
- ダッシュボードの **API Keys** で新規キーを作成。
- 目的に応じてスコープを選択:
  - `node`（ノード登録/配信）
  - `api`（/v1 推論API）
  - `admin`（管理系API）

## 2. ノード登録
- ノード起動時に `LLM_NODE_API_KEY` を指定する。
- `node` スコープのキーを利用。
- ヘルスチェック（`/v0/health`）も同じAPIキー + `X-Node-Token` を使用する。

## 3. 推論API呼び出し
- `/v1/*` へ `Authorization: Bearer <api_key>` を付与。
- `api` スコープが必須。

## 4. 管理系API
- `/v0` 管理系は **JWT (admin)** または **APIキー (admin)** が必須。

## 5. 開発時
- デバッグビルドでは `sk_debug*` キーが利用可能。
