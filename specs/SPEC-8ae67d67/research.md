# リサーチ（廃止）: ルーター主導のモデル自動配布機能

本仕様は廃止され、ルーターからノードへの push 配布は行いません。

## 現行の採用方針

- モデル一覧: `GET /v1/models`
- マニフェスト: `GET /v0/models/registry/:model_name/manifest.json`
- Node が HF から直接ダウンロード

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
