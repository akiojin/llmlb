-- Existing rows stay unattributed; issuer audit is not proof of usage ownership.
CREATE TABLE organizations (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 128),
    name_key TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT
);
CREATE UNIQUE INDEX organizations_active_name ON organizations(name_key) WHERE archived_at IS NULL;

ALTER TABLE users ADD COLUMN organization_id TEXT REFERENCES organizations(id);
ALTER TABLE users ADD COLUMN organization_role TEXT CHECK (
    (organization_id IS NULL AND organization_role IS NULL) OR
    (organization_id IS NOT NULL AND organization_role IS NOT NULL AND organization_role IN ('admin', 'member'))
);
ALTER TABLE users ADD COLUMN is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0, 1));
CREATE INDEX users_organization ON users(organization_id, id);

-- Snapshot identifiers deliberately have no cascading relationship to accounts.
ALTER TABLE api_keys ADD COLUMN usage_account_id TEXT;
ALTER TABLE api_keys ADD COLUMN organization_id TEXT CHECK (
    (usage_account_id IS NULL AND organization_id IS NULL) OR
    (usage_account_id IS NOT NULL AND organization_id IS NOT NULL)
);
ALTER TABLE api_keys ADD COLUMN attribution_revoked_at TEXT;
CREATE INDEX api_keys_usage_attribution ON api_keys(organization_id, usage_account_id);

-- A move followed by a return must not reactivate an old organization's key.
CREATE TRIGGER users_revoke_attributed_keys_on_membership_change
AFTER UPDATE OF organization_id, is_active ON users
WHEN OLD.organization_id IS NOT NEW.organization_id OR (OLD.is_active = 1 AND NEW.is_active = 0)
BEGIN
    UPDATE api_keys SET attribution_revoked_at = COALESCE(
        attribution_revoked_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    ) WHERE usage_account_id = NEW.id AND organization_id IS NOT NULL;
    -- Deactivation invalidates old JWTs permanently; a transfer alone keeps the
    -- session valid and resolves the new membership at the next authorization.
    UPDATE users SET password_changed_at = MAX(
        password_changed_at + 1,
        CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
    ) WHERE id = NEW.id AND OLD.is_active = 1 AND NEW.is_active = 0;
END;
CREATE TRIGGER users_revoke_attributed_keys_on_delete
AFTER DELETE ON users
BEGIN
    UPDATE api_keys SET attribution_revoked_at = COALESCE(
        attribution_revoked_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    ) WHERE usage_account_id = OLD.id AND organization_id IS NOT NULL;
END;

-- Preserve the last effective system administrator, including deactivation.
DROP TRIGGER users_last_admin_delete_guard;
DROP TRIGGER users_last_admin_update_guard;
CREATE TRIGGER users_last_admin_delete_guard
BEFORE DELETE ON users
WHEN OLD.role = 'admin' AND OLD.is_active = 1
     AND (SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1) <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;
CREATE TRIGGER users_last_admin_update_guard
BEFORE UPDATE OF role, is_active ON users
WHEN OLD.role = 'admin' AND OLD.is_active = 1
     AND (NEW.role != 'admin' OR NEW.is_active = 0)
     AND (SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = 1) <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;
