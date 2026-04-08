-- ユーザーテーブルに updated_at 列を追加
-- パスワード更新/リセット時にこの列を更新する

ALTER TABLE users ADD COLUMN updated_at TEXT;
