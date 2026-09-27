-- SPEC #582 US-005: クラウドメトリクスエクスポート
-- openai / google / anthropic へのクラウドプロキシ要求を 1 件 1 行で保持し、
-- export 時に UTC 日次 (date) × provider で rollup する（p95 を正確に算出するため）。
-- 保持期間は rolling 90 days。古い行は定期 prune と export 時に削除する。
CREATE TABLE IF NOT EXISTS cloud_request_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('openai', 'google', 'anthropic')),
    status_code INTEGER NOT NULL,
    latency_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cloud_request_metrics_date_provider
    ON cloud_request_metrics (date, provider);
