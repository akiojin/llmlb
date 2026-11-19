-- Operator-owned pause state and upstream 404 streak.
--
-- paused_at is deliberately separate from status: status is observed health,
-- paused_at is deliberate exclusion. A paused endpoint may still be online.
--
-- consecutive_404_count tracks consecutive upstream 404 responses. The proxy
-- treats a 404 as an endpoint failure and hops off the endpoint once the streak
-- exceeds 2. Any successful proxied request resets the streak to 0.

ALTER TABLE endpoints ADD COLUMN paused_at TEXT;

ALTER TABLE endpoints ADD COLUMN consecutive_404_count INTEGER NOT NULL DEFAULT 0;
