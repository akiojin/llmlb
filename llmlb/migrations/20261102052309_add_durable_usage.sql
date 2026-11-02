-- Usage is observed from deployment onward. Legacy history and API-key issuers
-- cannot establish a historical organization, so this migration does no backfill.
CREATE TABLE usage_observation (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    started_at TEXT NOT NULL
);

INSERT INTO usage_observation (id, started_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

-- Minimal immutable attribution and permanent once-state. There are deliberately
-- no identity foreign keys, message bodies, or copies of the final token values.
-- Keep this metadata when raw history/audit data is cleaned up or archived.
CREATE TABLE usage_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT,
    organization_id TEXT,
    received_at TEXT NOT NULL,
    date TEXT NOT NULL,
    api_key_id TEXT,
    auth_source TEXT NOT NULL CHECK (length(auth_source) > 0),
    finalized_at TEXT,
    token_source TEXT CHECK (token_source IN ('provider', 'estimated', 'missing')),
    CHECK ((finalized_at IS NULL) = (token_source IS NULL))
);

CREATE INDEX idx_usage_receipts_scope
    ON usage_receipts (organization_id, account_id, received_at);
CREATE INDEX idx_usage_receipts_pending
    ON usage_receipts (request_id) WHERE finalized_at IS NULL;

CREATE TRIGGER usage_receipts_attribution_immutable
BEFORE UPDATE OF request_id, account_id, organization_id, received_at, date,
                 api_key_id, auth_source ON usage_receipts
BEGIN
    SELECT RAISE(ABORT, 'Inference receipt attribution is immutable');
END;

-- Empty strings give each unknown identity one stable, unique bucket. INTEGER
-- checks also reject SQLite's promotion to REAL on integer-addition overflow.
CREATE TABLE usage_daily (
    date TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(request_count) = 'integer' AND request_count >= 0),
    pending_requests INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(pending_requests) = 'integer' AND pending_requests >= 0),
    input_tokens INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(input_tokens) = 'integer' AND input_tokens >= 0),
    output_tokens INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(output_tokens) = 'integer' AND output_tokens >= 0),
    total_tokens INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(total_tokens) = 'integer' AND total_tokens >= 0),
    provider_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(provider_count) = 'integer' AND provider_count >= 0),
    estimated_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(estimated_count) = 'integer' AND estimated_count >= 0),
    missing_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(missing_count) = 'integer' AND missing_count >= 0),
    input_missing_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(input_missing_count) = 'integer' AND input_missing_count >= 0),
    output_missing_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(output_missing_count) = 'integer' AND output_missing_count >= 0),
    total_missing_count INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(total_missing_count) = 'integer' AND total_missing_count >= 0),
    PRIMARY KEY (date, organization_id, account_id)
);

CREATE INDEX idx_usage_daily_scope ON usage_daily (organization_id, account_id, date);
