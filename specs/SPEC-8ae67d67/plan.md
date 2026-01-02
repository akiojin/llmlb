# 実装計画（廃止）: ルーター主導のモデル自動配布機能

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本仕様で想定していた「ルーターからノードへの push 配布」は採用しません。

## 現行設計（採用）

- ノードはルーターのモデル一覧を取得し、自律的にモデルを同期する
  - `GET /v1/models`
- モデルファイルはルーターから配信せず、HFから直接取得する
  - マニフェスト取得: `GET /v0/models/registry/:model_name/manifest.json`
- ルーター側のモデル管理API（管理者向け）
  - `POST /v0/models/register`
  - `GET /v1/models`
  - `DELETE /v1/models/:model_name`

詳細は `SPEC-dcaeaec4` と `SPEC-11106000/contracts/api_models.md` を参照してください。
