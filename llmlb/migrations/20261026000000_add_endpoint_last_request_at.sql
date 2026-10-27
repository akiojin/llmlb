-- 推論割当開始時刻。既存endpointの利用時刻は推測せずNULLを維持する。
ALTER TABLE endpoints ADD COLUMN last_request_at TEXT;
