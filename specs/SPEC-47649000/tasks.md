# タスク一覧: モデルメタデータSQLite統合

**機能ID**: `SPEC-47649000`
**ステータス**: 計画中

## Phase 1: マイグレーション作成

- [ ] T001 [P] `router/migrations/004_models.sql` マイグレーション作成
  - modelsテーブル定義（name, size, description, required_memory, source, path, download_url, repo, filename, status）
  - model_tagsテーブル定義（model_name, tag）
  - インデックス作成（source, tags）
  - 依存: なし

## Phase 2: テスト作成 (RED)

- [ ] T002 `router/src/db/models.rs` SQLite対応テスト作成
  - save_model()のテスト
  - load_all_models()のテスト
  - find_models_by_tag()のテスト
  - find_models_by_source()のテスト
  - delete_model()のテスト
  - 依存: T001

## Phase 3: 実装 (GREEN)

- [ ] T003 `router/src/db/models.rs` SQLite実装
  - モデルCRUDをSQLite使用に書き換え
  - model_tagsテーブルへのINSERT/DELETE処理
  - 既存のインターフェースを維持
  - 依存: T002

- [ ] T004 `router/src/db/migrations.rs` マイグレーション登録
  - 004_models.sqlをマイグレーションリストに追加
  - 依存: T003

## Phase 4: 移行ロジック

- [ ] T005 JSON→SQLite移行ロジック実装
  - 起動時に既存models.jsonを検出
  - SQLiteにデータをインポート
  - JSONファイルを.migratedにリネーム
  - 依存: T004

## Phase 5: 品質保証

- [ ] T006 品質チェック＆コミット
  - `cargo fmt --check` 合格
  - `cargo clippy -- -D warnings` 合格
  - `cargo test` 全テスト合格
  - markdownlint 合格
  - 依存: T005
