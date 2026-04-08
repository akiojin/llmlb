-- 招待キーテーブル（新仕様: T-0004-T-0007）
-- 8文字ランダム英数字の招待キーでユーザーを招待

CREATE TABLE IF NOT EXISTS invitation_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT UNIQUE NOT NULL,                 -- 8文字ランダム英数（例: A3X7B9K2）
    username TEXT NOT NULL,                   -- 招待対象ユーザー名（メールアドレス形式）
    role TEXT NOT NULL,                       -- ロール: "admin" または "viewer"
    created_at TEXT NOT NULL,                 -- ISO8601 format
    expires_at TEXT NOT NULL,                 -- ISO8601 format（有効期限: 7日後）
    is_used INTEGER NOT NULL DEFAULT 0,       -- 0 = 未使用, 1 = 使用済み
    used_at TEXT,                             -- 使用日時（nullable）
    CONSTRAINT valid_role CHECK (role IN ('admin', 'viewer')),
    CONSTRAINT valid_is_used CHECK (is_used IN (0, 1))
);

-- Index for fast lookup by key
CREATE INDEX IF NOT EXISTS idx_invitation_keys_key ON invitation_keys(key);

-- Index for listing active (unused) keys
CREATE INDEX IF NOT EXISTS idx_invitation_keys_is_used ON invitation_keys(is_used);

-- Index for finding by username
CREATE INDEX IF NOT EXISTS idx_invitation_keys_username ON invitation_keys(username);
