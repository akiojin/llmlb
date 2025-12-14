# リサーチ（廃止）: ルーター主導のモデル自動配布機能

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本仕様のリサーチ内容は採用されません。ルーターからノードへの配布（push）は実装しない方針です。

## 現行設計（採用）

- ノード主導でモデルを同期する
  - モデル一覧: `GET /v1/models`
  - モデル取得: `GET /api/models/blob/:model_name`
- ルーターはモデルの取得・登録・一覧化を担う（配布は行わない）
  - `GET /api/models/available`
  - `POST /api/models/register`
  - `GET /api/models/registered`

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
