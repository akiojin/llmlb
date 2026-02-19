# 契約: Models API 拡張 (SPEC-68551ec8)

## POST /api/models/register
- **Purpose**: HFリポジトリ/ファイルを対応モデルとして登録（メタデータのみ）。
- **Body**:
```json
{
  "repo": "org/repo",
  "filename": "model.safetensors"
}
```
- **Response** 201:
```json
{ "name": "hf/org/repo", "status": "registered" }
```
- **Errors**: 400 無効名/URL欠損, 409 重複, 424 HFから取得不可。

## GET /api/models
- **Purpose**: Node向けメタデータ一覧。
- **Response** 200:
```json
{
  "models": [
    {
      "name": "hf/org/repo",
      "repo": "org/repo",
      "filename": null,
      "source": "hf"
    }
  ]
}
```

## GET /api/models/registry/:model_name/manifest.json
- **Purpose**: Node向けマニフェスト（ファイル一覧）。
- **Response** 200:
```json
{
  "model": "hf/org/repo",
  "artifacts": [
    { "filename": "model.safetensors", "format": "safetensors", "size_bytes": 12345 },
    { "filename": "model.metal.bin", "format": "metal", "size_bytes": 6789 }
  ]
}
```

## GET /v1/models
- 対応モデルに HF 登録分も含めて返す。
- `ready` はNode同期結果に基づく。

---

## CLI コマンド
- `llmlb model add` は登録用途のみ。
- `model download` などロードバランサー主導のダウンロード系は廃止対象。
