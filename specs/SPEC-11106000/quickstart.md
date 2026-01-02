# クイックスタート: HF URL 登録（変換なし・Node直ダウンロード）

## 前提
- Router が起動済み
- Node が起動済み（HFへのネットワーク到達性あり）
- HF へのアクセスが必要な場合は **Node側**で `HF_TOKEN` を設定

## 1. モデルを登録

### Web
- 「モデル管理」画面のテキストエリアに HF リポジトリURL、
  もしくは `org/repo` を貼り付けて登録。

### API
```bash
curl -sS http://localhost:32768/v0/models/register \
  -H "Content-Type: application/json" \
  -d '{"repo":"org/repo"}' | jq .
```

ファイルURL登録の場合は `filename` を指定する:

```bash
curl -sS http://localhost:32768/v0/models/register \
  -H "Content-Type: application/json" \
  -d '{"repo":"org/repo","filename":"model.safetensors"}' | jq .
```

## 2. Nodeが同期して取得
- Nodeは起動時または同期通知時に /v0/models を参照し、
  マニフェストに従って HF から直接ダウンロードします。
- ルーターはバイナリを保持しません。

## 3. /v1/models で ready を確認
```bash
curl -sS http://localhost:32768/v1/models | jq .
```

`ready=true` になれば推論に利用できます。

## トラブルシュート
- HF 429/ダウン: Node側で `HF_TOKEN` を設定、または時間を置いて再試行。
- 登録失敗: 入力URL/ファイル名の誤り、HF非公開モデル、
  またはファイル一覧取得失敗の可能性。
