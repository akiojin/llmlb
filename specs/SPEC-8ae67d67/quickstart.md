# クイックスタート（廃止SPECの補足）: モデル同期（ノード主導）

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本SPECの「ロードバランサー主導の配布（push）」は採用しません。現行はノード主導のモデル同期です。

## ロードバランサー側: モデル登録（メタデータのみ）

Hugging Face 上のモデルをロードバランサーに登録します（バイナリのダウンロードや変換は行いません）。

```bash
curl -sS http://localhost:32768/api/models/register \
  -H "Content-Type: application/json" \
  -d '{"repo":"org/repo"}' | jq .
```

## エンドポイント側: モデル同期

ノードはロードバランサーのモデル一覧とマニフェストを参照し、
HFから直接ダウンロードして同期します。

- モデル一覧: `GET /v1/models`
- マニフェスト: `GET /api/models/registry/:model_name/manifest.json`

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
