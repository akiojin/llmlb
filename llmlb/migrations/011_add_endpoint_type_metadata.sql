-- SPEC-e8e9326e: Add endpoint_type metadata columns
-- 判定ソース/理由/時刻のメタデータを追加する

ALTER TABLE endpoints ADD COLUMN endpoint_type_source TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE endpoints ADD COLUMN endpoint_type_reason TEXT;
ALTER TABLE endpoints ADD COLUMN endpoint_type_detected_at TEXT;

CREATE INDEX IF NOT EXISTS idx_endpoints_type_source ON endpoints(endpoint_type_source);
