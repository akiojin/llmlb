-- request_historyテーブルのレガシーカラム名をendpoint系に統一
-- SQLite 3.25.0+ の ALTER TABLE RENAME COLUMN を使用

ALTER TABLE request_history RENAME COLUMN node_id TO endpoint_id;
ALTER TABLE request_history RENAME COLUMN node_machine_name TO endpoint_name;
ALTER TABLE request_history RENAME COLUMN node_ip TO endpoint_ip;

-- インデックスを再作成（カラム名変更後）
DROP INDEX IF EXISTS idx_request_history_node_id;
CREATE INDEX IF NOT EXISTS idx_request_history_endpoint_id ON request_history(endpoint_id);
