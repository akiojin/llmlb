# 実装計画: モデルメタデータSQLite統合

**機能ID**: `SPEC-47649000` | **日付**: 2025-12-20 | **仕様**: [spec.md](./spec.md)
**ステータス**: 計画中

## 概要

モデルメタデータ（従来のmodels.json）をSQLiteに統合し、認証システム（router.db）と
同じDBで管理する。タグ検索やソースフィルタリングを高速化する。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: SQLx, Axum
**ストレージ**: SQLite (router.db)
**テスト**: cargo test
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: single (router/)
**パフォーマンス目標**: モデル一覧取得 < 1秒、タグ検索 < 0.5秒
**制約**: 後方互換性100%維持

## 憲章チェック

**シンプルさ**: ✅ 合格

- プロジェクト数: 1 (router)
- フレームワークを直接使用: はい (SQLx)
- 単一データモデル: はい (Model, ModelTag)
- パターン回避: はい

**テスト**: ✅ 合格

- TDD順序: Contract→Integration→E2E→Unit
- 実DB使用: はい（SQLite）
- 移行テスト必須

## データモデル

### models テーブル

```sql
CREATE TABLE models (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    size INTEGER,
    description TEXT,
    required_memory INTEGER,
    source TEXT,
    path TEXT,
    artifacts_json TEXT,
    repo_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### model_tags テーブル

```sql
CREATE TABLE model_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id TEXT NOT NULL REFERENCES models(id),
    tag TEXT NOT NULL,
    UNIQUE(model_id, tag)
);

CREATE INDEX idx_model_tags_tag ON model_tags(tag);
```

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

1. マイグレーションスクリプト作成
2. Model/ModelTag エンティティ定義
3. CRUD操作実装
4. 既存API互換レイヤー
5. 自動移行ロジック（models.json → SQLite）
6. テスト（移行、CRUD、検索）

**順序戦略**:

- TDD順序: テストが実装より先
- 依存関係順序: マイグレーション → エンティティ → CRUD → API

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了
- [ ] Phase 3: Tasks生成済み
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [ ] 複雑さの逸脱を文書化済み

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
