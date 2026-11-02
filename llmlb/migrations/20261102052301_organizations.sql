-- New attribution starts without guessing an organization from legacy issuers.
CREATE TABLE organizations (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    normalized_name TEXT NOT NULL CHECK (length(normalized_name) > 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT
);

CREATE UNIQUE INDEX organizations_active_name_unique
ON organizations(normalized_name) WHERE archived_at IS NULL;

ALTER TABLE users ADD COLUMN organization_id TEXT REFERENCES organizations(id);
ALTER TABLE users ADD COLUMN organization_role TEXT NOT NULL DEFAULT 'member'
    CHECK (organization_role IN ('admin', 'member'));
ALTER TABLE users ADD COLUMN disabled INTEGER NOT NULL DEFAULT 0 CHECK (disabled IN (0, 1));
CREATE INDEX users_organization_id ON users(organization_id);

-- These are receipt identity values, not ownership FKs. Account/key deletion must
-- never cascade into durable attribution or rewrite an issued key's organization.
ALTER TABLE api_keys ADD COLUMN usage_account_id TEXT;
ALTER TABLE api_keys ADD COLUMN organization_id TEXT;
CREATE INDEX api_keys_usage_account_organization ON api_keys(organization_id, usage_account_id);

CREATE TRIGGER organizations_identity_immutable
BEFORE UPDATE OF id ON organizations WHEN NEW.id != OLD.id
BEGIN
    SELECT RAISE(ABORT, 'organization_identity_immutable');
END;

CREATE TRIGGER organizations_archive_membership_guard
BEFORE UPDATE OF archived_at ON organizations
WHEN NEW.archived_at IS NOT NULL
     AND EXISTS (SELECT 1 FROM users WHERE organization_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'organization_has_members');
END;

CREATE TRIGGER users_active_organization_insert_guard
BEFORE INSERT ON users
WHEN NEW.organization_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM organizations WHERE id = NEW.organization_id AND archived_at IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'invalid_organization_membership');
END;

CREATE TRIGGER users_active_organization_update_guard
BEFORE UPDATE OF organization_id ON users
WHEN NEW.organization_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM organizations WHERE id = NEW.organization_id AND archived_at IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'invalid_organization_membership');
END;

CREATE TRIGGER users_unassigned_role_insert_guard
BEFORE INSERT ON users
WHEN NEW.organization_id IS NULL AND NEW.organization_role != 'member'
BEGIN
    SELECT RAISE(ABORT, 'unassigned_organization_admin');
END;

CREATE TRIGGER users_unassigned_role_update_guard
BEFORE UPDATE OF organization_id, organization_role ON users
WHEN NEW.organization_id IS NULL AND NEW.organization_role != 'member'
BEGIN
    SELECT RAISE(ABORT, 'unassigned_organization_admin');
END;

-- Retain existing last-admin protection and include account disablement.
DROP TRIGGER users_last_admin_delete_guard;
CREATE TRIGGER users_last_admin_delete_guard
BEFORE DELETE ON users
WHEN OLD.role = 'admin' AND OLD.disabled = 0
     AND (SELECT COUNT(*) FROM users WHERE role = 'admin' AND disabled = 0) <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;

DROP TRIGGER users_last_admin_update_guard;
CREATE TRIGGER users_last_admin_update_guard
BEFORE UPDATE OF role, disabled ON users
WHEN OLD.role = 'admin' AND OLD.disabled = 0
     AND (NEW.role != 'admin' OR NEW.disabled != 0)
     AND (SELECT COUNT(*) FROM users WHERE role = 'admin' AND disabled = 0) <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;

CREATE TRIGGER api_keys_attribution_pair_insert_guard
BEFORE INSERT ON api_keys
WHEN (NEW.usage_account_id IS NULL) != (NEW.organization_id IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'incomplete_key_attribution');
END;

CREATE TRIGGER api_keys_attribution_pair_update_guard
BEFORE UPDATE OF usage_account_id, organization_id ON api_keys
WHEN (NEW.usage_account_id IS NULL) != (NEW.organization_id IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'incomplete_key_attribution');
END;
