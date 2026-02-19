# データモデル: 機能仕様書: Open Responses API対応

## 概要

Responses APIはパススルー方式のため、追加の永続データモデルは持ちません。
既存のOpenAI互換リクエスト/レスポンス処理に準拠し、
対応状況は `/v1/models` のメタ情報に反映されます。

## 影響範囲

- 既存のリクエスト履歴（SPEC-fbc50d97）に準拠
- エンドポイントの対応API情報の付与（SPEC-e8e9326e）

## 参照

- spec.md
- plan.md
