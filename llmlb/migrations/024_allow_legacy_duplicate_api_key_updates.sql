DROP TRIGGER IF EXISTS trg_api_keys_reject_duplicate_name_update;

CREATE TRIGGER IF NOT EXISTS trg_api_keys_reject_duplicate_name_update
BEFORE UPDATE OF name, created_by ON api_keys
FOR EACH ROW
WHEN (
    NEW.created_by IS NOT OLD.created_by
    OR NEW.name IS NOT OLD.name
)
AND EXISTS (
    SELECT 1
    FROM api_keys
    WHERE created_by = NEW.created_by
      AND name = NEW.name
      AND id <> OLD.id
)
BEGIN
    SELECT RAISE(ABORT, 'API key name already exists for this user');
END;
