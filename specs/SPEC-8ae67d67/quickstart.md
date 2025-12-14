# クイックスタート（廃止SPECの補足）: モデル同期（ノード主導）

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本SPECの「ルーター主導の配布（push）」は採用しません。現行はノード主導のモデル同期です。

## ルーター側: モデル取得（キャッシュ＆登録）

Hugging Face からモデルファイルを取得して、ルーターに登録します。

```bash
curl -sS http://localhost:8080/api/models/pull \
  -H "Content-Type: application/json" \
  -d '{
    "repo": "TheBloke/gpt-oss-GGUF",
    "filename": "gpt-oss-20b.Q4_K_M.gguf"
  }' | jq .
```

登録済みモデルを確認します。

```bash
curl -sS http://localhost:8080/api/models/registered | jq .
```

## ノード側: モデル同期

ノードはルーターのモデル一覧を参照してモデルを同期します。

- モデル一覧: `GET /v1/models`
- モデル取得: `GET /api/models/blob/:model_name`

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
