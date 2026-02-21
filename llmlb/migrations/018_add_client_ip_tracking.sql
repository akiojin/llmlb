-- SPEC-62ac4b68: IPアドレスロギング＆クライアント分析
-- request_historyにapi_key_idカラム追加、インデックス追加、settingsテーブル新規作成

-- api_key_idカラムをrequest_historyに追加
ALTER TABLE request_history ADD COLUMN api_key_id TEXT;

-- client_ipにインデックス追加（IPフィルター・集計用）
CREATE INDEX IF NOT EXISTS idx_request_history_client_ip ON request_history(client_ip);

-- api_key_idにインデックス追加（APIキー別集計用）
CREATE INDEX IF NOT EXISTS idx_request_history_api_key_id ON request_history(api_key_id);

-- settingsテーブル新規作成
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- デフォルト閾値設定
INSERT OR IGNORE INTO settings (key, value, updated_at)
VALUES ('ip_alert_threshold', '100', datetime('now'));
