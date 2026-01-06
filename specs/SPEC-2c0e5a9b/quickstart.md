# クイックスタート: gpt-oss-20b safetensors 実行

## 前提条件

- GPU搭載ノードが登録済み（Metal または CUDA）
- HuggingFace アクセストークン（プライベートモデルの場合）
- ネットワーク接続（モデルダウンロード用）

## 1. モデル登録

```bash
# APIキー認証
export API_KEY="sk_your_api_key"

# gpt-oss-20b をsafetensors 形式で登録
curl -X POST http://localhost:3000/v1/models/register \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "gpt-oss-20b",
    "source": "openai/gpt-oss-20b",
    "format": "safetensors"
  }'
```

## 2. モデル状態確認

```bash
# モデル一覧取得（ready 状態を確認）
curl http://localhost:3000/v1/models \
  -H "Authorization: Bearer $API_KEY"
```

期待されるレスポンス:

```json
{
  "data": [
    {
      "id": "gpt-oss-20b",
      "object": "model",
      "ready": true
    }
  ]
}
```

## 3. 推論実行

```bash
# チャット補完
curl -X POST http://localhost:3000/v1/chat/completions \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-oss-20b",
    "messages": [
      {"role": "user", "content": "Hello, how are you?"}
    ]
  }'
```

## トラブルシューティング

### モデルが ready にならない

1. ノードのGPU状態を確認:

   ```bash
   curl http://localhost:3000/v0/nodes \
     -H "Authorization: Bearer $API_KEY"
   ```

2. 必須ファイルの存在を確認:
   - `config.json`
   - `tokenizer.json`
   - `model.safetensors.index.json`（シャーディングの場合）

### メタデータ不足エラー

登録時に以下のエラーが出る場合:

```json
{"error": {"message": "Required file missing: config.json"}}
```

HuggingFace リポジトリに必須ファイルが存在することを確認してください。

### GPU未対応エラー

Windows では CUDA を使用します。DirectML は実験扱いで、公式GPU最適化アーティファクトが必須です。
safetensors からの直接推論は後続バージョンで対応予定。
現時点の Hugging Face `openai/gpt-oss-*` には DirectML 向けアーティファクトが含まれていないため、
`model.directml.bin` / `model.dml.bin` を別途用意してください。
