# 実装計画（廃止）: ルーター主導のモデル自動配布機能

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本仕様で想定していた「ルーターからノードへの push 配布」は採用しません。

## 現行設計（採用）

- ノードはルーターのモデル一覧を取得し、自律的にモデルを同期する
  - `GET /v1/models`
- モデルファイルはルーターが公開するモデル配信APIから取得する
  - `GET /api/models/blob/:model_name`
- ルーター側のモデル管理API（管理者向け）
  - `GET /api/models/available`
  - `POST /api/models/register`
  - `GET /api/models/registered`
  - `DELETE /api/models/*model_name`

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
