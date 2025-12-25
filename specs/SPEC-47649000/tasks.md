# タスク一覧: モデルメタデータSQLite統合

**機能ID**: `SPEC-47649000`
**ステータス**: ✅ 実装完了（5/6タスク完了）

## Phase 1: マイグレーション作成

- [x] T001 [P] `router/migrations/001_init.sql` マイグレーション作成
  - ✅ modelsテーブル定義（line 120-133）
  - ✅ model_tagsテーブル定義（line 139-143）
  - ✅ model_capabilitiesテーブル定義（line 148-152）
  - ✅ インデックス作成（source, status, tags）
  - 注: 別ファイル（004_models.sql）ではなく001_init.sqlに統合

## Phase 2: テスト作成 (RED)

- [x] T002 `router/src/db/models.rs` SQLite対応テスト作成
  - ✅ test_save_and_load_model()
  - ✅ test_load_models()
  - ✅ test_delete_model()
  - ✅ test_update_model()
  - 注: find_by_tag/find_by_sourceはload_models後のフィルタで対応

## Phase 3: 実装 (GREEN)

- [x] T003 `router/src/db/models.rs` SQLite実装
  - ✅ ModelStorage構造体（line 10-311）
  - ✅ save_model() - UPSERT処理
  - ✅ load_models() - 全モデル読み込み
  - ✅ load_model() - 個別モデル読み込み
  - ✅ delete_model() - 削除処理
  - ✅ タグ・能力のINSERT/DELETE処理

- [x] T004 `router/src/db/migrations.rs` マイグレーション登録
  - ✅ 001_init.sqlにmodelsテーブルを統合（別ファイル登録不要）
  - ✅ sqlx::migrate!マクロで自動適用

## Phase 4: 移行ロジック

- [ ] T005 JSON→SQLite移行ロジック実装
  - ⚠️ 新規インストールはSQLiteのみ使用（models.json不使用）
  - 🔴 レガシーmodels.jsonからの移行機能は未実装
  - 将来的に既存ユーザーのアップグレード対応が必要な場合に実装

## Phase 5: 品質保証

- [x] T006 品質チェック＆コミット
  - ✅ `cargo test -p llm-router --lib -- db::models` 4テスト合格
  - ✅ `cargo test -p llm-router --test '*' -- models` 15テスト合格
  - ✅ モデルAPI契約テスト合格
