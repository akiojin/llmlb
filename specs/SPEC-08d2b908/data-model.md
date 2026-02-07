# データモデル: SPEC-08d2b908: モデル管理（統合仕様）

## 概要

モデル管理はエンドポイントごとのモデル情報と、全体カタログを統合して扱います。
永続化はSQLiteの既存テーブルに集約し、追加のデータモデルは最小限に留めます。

## 主なエンティティ

- EndpointModelInfo: エンドポイント単位のモデル一覧
- ModelCatalogEntry: 統合モデル一覧のエントリ
- ModelCapability: モダリティ/対応APIのメタ情報

## 参照

- spec.md
- plan.md
