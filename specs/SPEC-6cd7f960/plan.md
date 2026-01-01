# SPEC-6cd7f960: 実装計画

**ステータス**: 改定中（方針更新）

## 方針更新（2025-12-31）
- `POST /v0/models/pull` は採用しない
- URL登録（`/v0/models/register`）は維持し、ルーターはメタデータのみ保持
- Nodeがマニフェストに従ってHFから直接取得する
- Model Hub UI（対応モデル一覧 + Pull）は廃止し、URL登録フォームを維持

## 更新対象
- `/v0/models` の一覧内容（status/ready）を現行仕様に合わせる
- supported_models.json の役割（内部の対応モデル定義）を再整理
- ダッシュボード UI は `SPEC-11106000` の方針に合わせる

## 次のステップ
- 本Planは旧設計（ModelHub/Pull前提）を撤回し、
  新方針に基づく再設計版を作成する
