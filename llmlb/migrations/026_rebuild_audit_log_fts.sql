-- SPEC-8301d106: Rebuild audit_log_fts with client_ip support
--
-- Migration 018 added client_ip to audit_log_entries, but migration 019 created
-- audit_log_fts without client_ip in the FTS index. This forward migration rebuilds
-- the FTS index to include client_ip for all environments (new and existing).

-- Drop and recreate FTS table to include client_ip in the index
DROP TABLE IF EXISTS audit_log_fts;

-- Recreate FTS5 virtual table with client_ip support
CREATE VIRTUAL TABLE audit_log_fts USING fts5(
    request_path,
    actor_id,
    actor_username,
    client_ip,
    detail,
    content=audit_log_entries,
    content_rowid=id
);

-- Rebuild FTS index from audit_log_entries
INSERT INTO audit_log_fts(rowid, request_path, actor_id, actor_username, client_ip, detail)
SELECT id, request_path, actor_id, actor_username, client_ip, detail FROM audit_log_entries;

-- Recreate FTS sync trigger for INSERT
DROP TRIGGER IF EXISTS audit_log_fts_insert;
CREATE TRIGGER audit_log_fts_insert AFTER INSERT ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(rowid, request_path, actor_id, actor_username, client_ip, detail)
    VALUES (new.id, new.request_path, new.actor_id, new.actor_username, new.client_ip, new.detail);
END;

-- Recreate FTS sync trigger for DELETE
DROP TRIGGER IF EXISTS audit_log_fts_delete;
CREATE TRIGGER audit_log_fts_delete AFTER DELETE ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(audit_log_fts, rowid, request_path, actor_id, actor_username, client_ip, detail)
    VALUES ('delete', old.id, old.request_path, old.actor_id, old.actor_username, old.client_ip, old.detail);
END;
