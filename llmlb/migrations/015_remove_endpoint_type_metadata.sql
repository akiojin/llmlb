-- Remove endpoint_type detection metadata columns (source/reason/detected_at)
-- These columns were added in 011 but are no longer needed after simplifying
-- endpoint type detection logic.
-- Also change endpoint_type default from 'unknown' to 'openai_compatible'.
--
-- SQLite does not support DROP COLUMN on older versions, so we recreate the table.

-- Step 1: Create new table without the 3 metadata columns
CREATE TABLE endpoints_new (
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
    endpoint_type TEXT NOT NULL DEFAULT 'openai_compatible',
    total_requests INTEGER NOT NULL DEFAULT 0,
    successful_requests INTEGER NOT NULL DEFAULT 0,
    failed_requests INTEGER NOT NULL DEFAULT 0
);

-- Step 2: Copy data, mapping 'unknown' endpoint_type to 'openai_compatible'
INSERT INTO endpoints_new (
    id, name, base_url, api_key_encrypted,
    status, health_check_interval_secs, inference_timeout_secs,
    latency_ms, last_seen, last_error, error_count,
    registered_at, notes, capabilities, device_info,
    inference_latency_ms, endpoint_type,
    total_requests, successful_requests, failed_requests
)
SELECT
    id, name, base_url, api_key_encrypted,
    status, health_check_interval_secs, inference_timeout_secs,
    latency_ms, last_seen, last_error, error_count,
    registered_at, notes, capabilities, device_info,
    inference_latency_ms,
    CASE WHEN endpoint_type = 'unknown' THEN 'openai_compatible' ELSE endpoint_type END,
    total_requests, successful_requests, failed_requests
FROM endpoints;

-- Step 3: Drop old table and rename
DROP TABLE endpoints;
ALTER TABLE endpoints_new RENAME TO endpoints;

-- Step 4: Recreate indexes (excluding idx_endpoints_type_source which is dropped)
CREATE INDEX IF NOT EXISTS idx_endpoints_status ON endpoints(status);
CREATE INDEX IF NOT EXISTS idx_endpoints_name ON endpoints(name);
CREATE INDEX IF NOT EXISTS idx_endpoints_type ON endpoints(endpoint_type);
