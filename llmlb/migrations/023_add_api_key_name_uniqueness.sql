-- APIキー名の重複禁止（同一ユーザー内）
--
-- 既存DBに重複データがある場合でもマイグレーションが失敗しないよう、
-- UNIQUE INDEX ではなくトリガーで新規INSERT/UPDATEを拒否する。

CREATE INDEX IF NOT EXISTS idx_api_keys_created_by_name ON api_keys(created_by, name);

CREATE TRIGGER IF NOT EXISTS trg_api_keys_reject_duplicate_name_insert
BEFORE INSERT ON api_keys
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM api_keys
    WHERE created_by = NEW.created_by
      AND name = NEW.name
)
BEGIN
    SELECT RAISE(ABORT, 'API key name already exists for this user');
END;

CREATE TRIGGER IF NOT EXISTS trg_api_keys_reject_duplicate_name_update
BEFORE UPDATE OF name, created_by ON api_keys
FOR EACH ROW
WHEN EXISTS (
    SELECT 1
    FROM api_keys
    WHERE created_by = NEW.created_by
      AND name = NEW.name
      AND id <> OLD.id
)
BEGIN
    SELECT RAISE(ABORT, 'API key name already exists for this user');
END;
