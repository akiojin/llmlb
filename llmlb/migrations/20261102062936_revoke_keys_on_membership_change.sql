-- A returning account must explicitly receive a new organization key. Equality
-- with its current organization cannot prove that an old key is still authorized.
ALTER TABLE api_keys ADD COLUMN revoked_at TEXT;

CREATE TRIGGER users_revoke_attributed_keys_after_membership_change
AFTER UPDATE OF organization_id ON users
WHEN NEW.organization_id IS NOT OLD.organization_id
BEGIN
    UPDATE api_keys
    SET revoked_at = COALESCE(revoked_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    WHERE usage_account_id = OLD.id AND organization_id IS NOT NULL;
END;
