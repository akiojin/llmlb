-- Preserve known terminal outcomes independently of raw-history retention.
-- Prior completed receipts without proof of an outcome remain unknown.
ALTER TABLE usage_receipts ADD COLUMN outcome TEXT CHECK (outcome IN ('success', 'error'));
ALTER TABLE usage_daily ADD COLUMN successful_requests INTEGER NOT NULL DEFAULT 0
    CHECK (typeof(successful_requests) = 'integer' AND successful_requests >= 0);
ALTER TABLE usage_daily ADD COLUMN failed_requests INTEGER NOT NULL DEFAULT 0
    CHECK (typeof(failed_requests) = 'integer' AND failed_requests >= 0
           AND successful_requests + failed_requests <= request_count - pending_requests);
