-- request_history テーブルにトークンカラム追加
ALTER TABLE request_history ADD COLUMN input_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN output_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN total_tokens INTEGER;

-- 集計用インデックス
CREATE INDEX idx_request_history_tokens ON request_history(timestamp DESC, model);
CREATE INDEX idx_request_history_node_tokens ON request_history(node_id, timestamp DESC);
