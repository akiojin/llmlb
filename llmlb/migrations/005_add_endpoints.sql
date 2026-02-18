-- SPEC-e8e9326e: llmlb主導エンドポイント登録システム
-- エンドポイント管理テーブル

-- endpoints テーブル
CREATE TABLE IF NOT EXISTS endpoints (
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
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_endpoints_status ON endpoints(status);
CREATE INDEX IF NOT EXISTS idx_endpoints_name ON endpoints(name);

-- endpoint_models テーブル
CREATE TABLE IF NOT EXISTS endpoint_models (
    endpoint_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    capabilities TEXT,
    last_checked TEXT,
    PRIMARY KEY (endpoint_id, model_id),
    FOREIGN KEY (endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_endpoint_models_model ON endpoint_models(model_id);

-- endpoint_health_checks テーブル（履歴、30日保持）
CREATE TABLE IF NOT EXISTS endpoint_health_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    success INTEGER NOT NULL,
    latency_ms INTEGER,
    error_message TEXT,
    status_before TEXT NOT NULL,
    status_after TEXT NOT NULL,
    FOREIGN KEY (endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_health_checks_endpoint ON endpoint_health_checks(endpoint_id);
CREATE INDEX IF NOT EXISTS idx_health_checks_checked_at ON endpoint_health_checks(checked_at);
