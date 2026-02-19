# リサーチ（廃止）: ロードバランサー主導のモデル自動配布機能

本仕様は廃止され、ロードバランサーからノードへの push 配布は行いません。

## 現行の採用方針

- モデル一覧: `GET /v1/models`
- マニフェスト: `GET /api/models/registry/:model_name/manifest.json`
- Node が HF から直接ダウンロード

詳細は `SPEC-dcaeaec4` と `SPEC-68551ec8/contracts/api_models.md` を参照してください。
