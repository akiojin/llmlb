-- T044: リクエスト履歴のSQLiteスキーマ
-- SPEC-fbc50d97: リクエスト/レスポンス履歴保存機能

-- リクエスト履歴テーブル
CREATE TABLE IF NOT EXISTS request_history (
    id TEXT PRIMARY KEY NOT NULL,              -- UUID
    timestamp TEXT NOT NULL,                    -- ISO8601形式（リクエスト受信時刻）
    request_type TEXT NOT NULL,                 -- Chat / Generate / Image など
    model TEXT NOT NULL,                        -- 使用されたモデル名
    node_id TEXT NOT NULL,                      -- 処理したノードのUUID
    node_machine_name TEXT NOT NULL,            -- ノードのマシン名
    node_ip TEXT NOT NULL,                      -- ノードのIPアドレス
    client_ip TEXT,                             -- クライアントのIPアドレス（NULL可）
    request_body TEXT NOT NULL,                 -- リクエスト本文（JSON文字列）
    response_body TEXT,                         -- レスポンス本文（JSON文字列、NULL可）
    duration_ms INTEGER NOT NULL,               -- 処理時間（ミリ秒）
    status TEXT NOT NULL,                       -- success / error
    error_message TEXT,                         -- エラー時のメッセージ（NULL可）
    completed_at TEXT NOT NULL                  -- ISO8601形式（レスポンス完了時刻）
);

-- タイムスタンプインデックス（時系列順表示の高速化）
CREATE INDEX IF NOT EXISTS idx_request_history_timestamp ON request_history(timestamp);

-- モデル名インデックス（モデル別フィルタリング）
CREATE INDEX IF NOT EXISTS idx_request_history_model ON request_history(model);

-- ノードIDインデックス（ノード別フィルタリング）
CREATE INDEX IF NOT EXISTS idx_request_history_node_id ON request_history(node_id);

-- ステータスインデックス（成功/失敗フィルタリング）
CREATE INDEX IF NOT EXISTS idx_request_history_status ON request_history(status);

-- 複合インデックス（timestamp DESC + statusによる一覧表示の高速化）
CREATE INDEX IF NOT EXISTS idx_request_history_timestamp_status ON request_history(timestamp DESC, status);
