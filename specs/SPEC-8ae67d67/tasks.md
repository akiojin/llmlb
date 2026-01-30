# タスク一覧（廃止）: ロードバランサー主導のモデル自動配布機能

**機能ID**: `SPEC-8ae67d67`
**ステータス**: 廃止 (2025-12-13)

本仕様のタスクは採用されません。ロードバランサー主導の配布（push）を前提とするAPI/実装は追加しない方針です。

## 現行設計（採用）

- ノードはロードバランサーのモデル一覧を取得し、自律的にモデルを同期する
  - `GET /v1/models`
- マニフェストに基づき、HFから直接ダウンロードする
  - `GET /api/models/registry/:model_name/manifest.json`

関連: `SPEC-dcaeaec4`, `SPEC-11106000/contracts/api_models.md`
