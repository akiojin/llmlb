-- Issue #947: admission snapshots and durable, body-free usage aggregation.
-- Snapshot identifiers deliberately have no cascading foreign keys.
ALTER TABLE request_history ADD COLUMN organization_id TEXT;
ALTER TABLE request_history ADD COLUMN account_id TEXT;
CREATE INDEX idx_request_history_usage_scope
    ON request_history(organization_id, account_id, timestamp DESC);

CREATE TABLE usage_receipts (
    id TEXT PRIMARY KEY NOT NULL,
    received_at TEXT NOT NULL,
    organization_id TEXT,
    account_id TEXT,
    api_key_id TEXT,
    auth_source TEXT NOT NULL,
    endpoint_id TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    finalized INTEGER NOT NULL DEFAULT 0,
    successful INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    input_source TEXT NOT NULL DEFAULT 'missing',
    output_source TEXT NOT NULL DEFAULT 'missing',
    total_source TEXT NOT NULL DEFAULT 'missing'
);
CREATE INDEX idx_usage_receipts_scope_date
    ON usage_receipts(organization_id, account_id, received_at);

CREATE TABLE usage_daily (
    date TEXT NOT NULL,
    organization_id TEXT NOT NULL DEFAULT '',
    account_id TEXT NOT NULL DEFAULT '',
    endpoint_id TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    request_count INTEGER NOT NULL DEFAULT 0,
    successful_requests INTEGER NOT NULL DEFAULT 0,
    failed_requests INTEGER NOT NULL DEFAULT 0,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    provider_count INTEGER NOT NULL DEFAULT 0,
    estimated_count INTEGER NOT NULL DEFAULT 0,
    missing_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(date, organization_id, account_id, endpoint_id, model)
);
CREATE INDEX idx_usage_daily_account_date ON usage_daily(organization_id, account_id, date);

-- Existing raw rows remain available to global administrators, but are not
-- reconstructed into a historical ledger. Record the actual collection boundary.
INSERT OR IGNORE INTO settings(key, value, updated_at)
VALUES ('usage_observation_started_at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), datetime('now'));
