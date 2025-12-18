# クイックスタート: HFモデル登録（ONNX優先・自動エクスポート）

## 前提
- Router が起動済み（例: `http://localhost:8080`）
- Node が登録済み（GPUデバイス情報が取得できること）
- HF へのネットワーク到達性あり（必要なら `HF_TOKEN`、社内ミラー利用時は `HF_BASE_URL`）

## 1. モデルを登録（ダウンロード/変換キュー）

### Dashboard
- Dashboard → Models → Registered Models → Register
- `repo`（例: `sshleifer/tiny-gpt2`）を入力して登録
- 変換でカスタムコードが必要なモデルは `trust_remote_code` をON（危険: 任意コード実行）

### API（curl）
```bash
curl -sS http://localhost:8080/v0/models/register \
  -H "Content-Type: application/json" \
  -d '{
    "repo": "sshleifer/tiny-gpt2",
    "trust_remote_code": false
  }' | jq .
```

- `filename` を省略すると、リポジトリ内の `.onnx` を探索し、無ければ Transformers → ONNX エクスポートを試みます。
- `.onnx` を直接指定したい場合は `filename` に `.onnx` を指定します。

## 2. 進捗/結果を確認（/v0/models）
`lifecycle_status` で状態を確認します。

```bash
curl -sS http://localhost:8080/v0/models \
  -H "Authorization: Bearer sk_debug" | jq .
```

- `pending`: キュー待ち
- `caching`: ダウンロード/変換中
- `registered`: ルーター上に実体ONNXがあり、ノードが同期可能
- `error`: 失敗（`download_progress.error` に理由）

## 3. 推論で利用（/v1/chat/completions）
OpenAI互換エンドポイントで `model` に登録ID（通常は `repo`）を指定します。

```bash
curl -sS http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "sshleifer/tiny-gpt2",
    "messages": [{"role":"user","content":"こんにちは"}],
    "max_tokens": 64
  }' | jq .
```

## トラブルシュート
- 登録が `error` になる: `download_progress.error` と Routerログを確認し、必要なら `trust_remote_code` をONにして再登録/Restoreします。
- 変換が重い: まず小さいモデルで確認します（例: `sshleifer/tiny-gpt2`）。
