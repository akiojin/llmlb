-- SPEC-8301d106: 監査ログ（Audit Log）テーブル

-- 監査ログエントリ
CREATE TABLE IF NOT EXISTS audit_log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TEXT NOT NULL,
    http_method TEXT NOT NULL,
    request_path TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT,
    actor_username TEXT,
    api_key_owner_id TEXT,
    client_ip TEXT,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    model_name TEXT,
    endpoint_id TEXT,
    detail TEXT,
    batch_id INTEGER REFERENCES audit_batch_hashes(id),
    is_migrated INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- バッチハッシュ（改ざん防止チェーン）
CREATE TABLE IF NOT EXISTS audit_batch_hashes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence_number INTEGER NOT NULL UNIQUE,
    batch_start TEXT NOT NULL,
    batch_end TEXT NOT NULL,
    record_count INTEGER NOT NULL,
    hash TEXT NOT NULL,
    previous_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- インデックス
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log_entries(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log_entries(actor_type, actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_path ON audit_log_entries(request_path);
CREATE INDEX IF NOT EXISTS idx_audit_log_status ON audit_log_entries(status_code);
CREATE INDEX IF NOT EXISTS idx_audit_log_batch ON audit_log_entries(batch_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_model ON audit_log_entries(model_name)
    WHERE model_name IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_audit_log_tokens ON audit_log_entries(timestamp, model_name)
    WHERE total_tokens IS NOT NULL;

-- FTS5全文検索仮想テーブル
CREATE VIRTUAL TABLE IF NOT EXISTS audit_log_fts USING fts5(
    request_path,
    actor_id,
    actor_username,
    detail,
    content=audit_log_entries,
    content_rowid=id
);

-- FTS同期トリガー（INSERT）
CREATE TRIGGER IF NOT EXISTS audit_log_fts_insert AFTER INSERT ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(rowid, request_path, actor_id, actor_username, detail)
    VALUES (new.id, new.request_path, new.actor_id, new.actor_username, new.detail);
END;

-- FTS同期トリガー（DELETE）
CREATE TRIGGER IF NOT EXISTS audit_log_fts_delete AFTER DELETE ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(audit_log_fts, rowid, request_path, actor_id, actor_username, detail)
    VALUES ('delete', old.id, old.request_path, old.actor_id, old.actor_username, old.detail);
END;

-- request_historyからのデータ移行
-- request_historyテーブルが存在する場合のみ実行
INSERT INTO audit_log_entries (
    timestamp, http_method, request_path, status_code,
    actor_type, actor_id, duration_ms,
    input_tokens, output_tokens, total_tokens,
    model_name, endpoint_id, is_migrated
)
SELECT
    rh.timestamp,
    'POST',
    '/v1/chat/completions',
    CASE WHEN rh.error_message IS NULL THEN 200 ELSE 500 END,
    'api_key',
    'unknown',
    rh.duration_ms,
    rh.input_tokens,
    rh.output_tokens,
    rh.total_tokens,
    rh.model,
    rh.node_id,
    1
FROM request_history rh
WHERE EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='request_history');
