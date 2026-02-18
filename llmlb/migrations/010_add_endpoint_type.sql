-- SPEC-e8e9326e: エンドポイントタイプ自動判別機能
-- エンドポイントタイプ、モデルメタデータ、ダウンロードタスク

-- endpoints テーブルに endpoint_type 列を追加
ALTER TABLE endpoints ADD COLUMN endpoint_type TEXT NOT NULL DEFAULT 'unknown';

CREATE INDEX IF NOT EXISTS idx_endpoints_type ON endpoints(endpoint_type);

-- endpoint_models テーブルに max_tokens 列を追加
ALTER TABLE endpoint_models ADD COLUMN max_tokens INTEGER;

-- model_download_tasks テーブル（xLLMエンドポイント専用）
CREATE TABLE IF NOT EXISTS model_download_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    endpoint_id TEXT NOT NULL,
    model TEXT NOT NULL,
    filename TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    progress REAL NOT NULL DEFAULT 0.0,
    speed_mbps REAL,
    eta_seconds INTEGER,
    error_message TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_download_tasks_endpoint ON model_download_tasks(endpoint_id);
CREATE INDEX IF NOT EXISTS idx_download_tasks_status ON model_download_tasks(status);
