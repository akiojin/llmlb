-- パスワードリセットトークン（SPEC #580 US-008 / AS-014〜AS-016）
-- 平文トークンは保存せず SHA-256 ハッシュのみを保持する。
-- 時刻はすべて epoch ミリ秒。used_at が NULL かつ expires_at が未来のものだけが有効。
CREATE TABLE IF NOT EXISTS password_reset_tokens (
    id TEXT PRIMARY KEY NOT NULL,               -- UUID
    user_id TEXT NOT NULL,                      -- 対象ユーザー
    token_hash TEXT UNIQUE NOT NULL,            -- SHA-256(hex)
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_password_reset_tokens_user_id ON password_reset_tokens(user_id);
