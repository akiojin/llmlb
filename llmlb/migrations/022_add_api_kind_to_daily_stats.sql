-- SPEC-ba72f693: endpoint_daily_statsにapi_kindカラムを追加
-- TPS seeding時にAPI種別（chat_completions/completions/responses）を
-- 正確に復元するために必要
-- PKを(endpoint_id, model_id, date, api_kind)に変更するためテーブル再作成

CREATE TABLE endpoint_daily_stats_new (
    endpoint_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    date TEXT NOT NULL,
    api_kind TEXT NOT NULL DEFAULT 'chat_completions',
    total_requests INTEGER NOT NULL DEFAULT 0,
    successful_requests INTEGER NOT NULL DEFAULT 0,
    failed_requests INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    total_duration_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (endpoint_id, model_id, date, api_kind)
);

INSERT INTO endpoint_daily_stats_new (
    endpoint_id, model_id, date, api_kind,
    total_requests, successful_requests, failed_requests,
    total_output_tokens, total_duration_ms
)
SELECT
    endpoint_id, model_id, date, 'chat_completions',
    total_requests, successful_requests, failed_requests,
    total_output_tokens, total_duration_ms
FROM endpoint_daily_stats;

DROP TABLE endpoint_daily_stats;
ALTER TABLE endpoint_daily_stats_new RENAME TO endpoint_daily_stats;

CREATE INDEX IF NOT EXISTS idx_daily_stats_endpoint_date
    ON endpoint_daily_stats (endpoint_id, date);

CREATE INDEX IF NOT EXISTS idx_daily_stats_date
    ON endpoint_daily_stats (date);
