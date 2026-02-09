-- Remove endpoints.supports_responses_api
-- Responses API is treated as always supported, so the per-endpoint flag is no longer needed.

PRAGMA foreign_keys=off;

CREATE TABLE IF NOT EXISTS endpoints_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    base_url TEXT NOT NULL UNIQUE,
    api_key_encrypted TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    health_check_interval_secs INTEGER NOT NULL DEFAULT 30,
    inference_timeout_secs INTEGER NOT NULL DEFAULT 120,
    latency_ms INTEGER,
    last_seen TEXT,
    last_error TEXT,
    error_count INTEGER NOT NULL DEFAULT 0,
    registered_at TEXT NOT NULL,
    notes TEXT,
    capabilities TEXT DEFAULT '["chat_completion"]',
    device_info TEXT,
    inference_latency_ms REAL,
    endpoint_type TEXT NOT NULL DEFAULT 'unknown',
    endpoint_type_source TEXT NOT NULL DEFAULT 'auto',
    endpoint_type_reason TEXT,
    endpoint_type_detected_at TEXT
);

INSERT INTO endpoints_new (
    id,
    name,
    base_url,
    api_key_encrypted,
    status,
    health_check_interval_secs,
    inference_timeout_secs,
    latency_ms,
    last_seen,
    last_error,
    error_count,
    registered_at,
    notes,
    capabilities,
    device_info,
    inference_latency_ms,
    endpoint_type,
    endpoint_type_source,
    endpoint_type_reason,
    endpoint_type_detected_at
)
SELECT
    id,
    name,
    base_url,
    api_key_encrypted,
    status,
    health_check_interval_secs,
    inference_timeout_secs,
    latency_ms,
    last_seen,
    last_error,
    error_count,
    registered_at,
    notes,
    capabilities,
    device_info,
    inference_latency_ms,
    endpoint_type,
    endpoint_type_source,
    endpoint_type_reason,
    endpoint_type_detected_at
FROM endpoints;

DROP TABLE endpoints;
ALTER TABLE endpoints_new RENAME TO endpoints;

CREATE INDEX IF NOT EXISTS idx_endpoints_status ON endpoints(status);
CREATE INDEX IF NOT EXISTS idx_endpoints_name ON endpoints(name);
CREATE INDEX IF NOT EXISTS idx_endpoints_type ON endpoints(endpoint_type);
CREATE INDEX IF NOT EXISTS idx_endpoints_type_source ON endpoints(endpoint_type_source);

PRAGMA foreign_keys=on;
