-- 招待コードテーブル
-- 管理者が発行した招待コードでのみユーザー登録を可能にする

CREATE TABLE IF NOT EXISTS invitation_codes (
    id TEXT PRIMARY KEY NOT NULL,           -- UUID
    code_hash TEXT UNIQUE NOT NULL,         -- SHA-256 hash (plaintext is never stored)
    created_by TEXT NOT NULL,               -- Admin user who created it
    created_at TEXT NOT NULL,               -- ISO8601 format
    expires_at TEXT NOT NULL,               -- ISO8601 format (required)
    status TEXT NOT NULL DEFAULT 'active',  -- 'active', 'used', 'revoked'
    used_by TEXT,                           -- User ID who used it (nullable)
    used_at TEXT,                           -- When it was used (nullable)
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (used_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Index for fast lookup by code hash during registration
CREATE INDEX IF NOT EXISTS idx_invitation_codes_hash ON invitation_codes(code_hash);

-- Index for listing active/used codes
CREATE INDEX IF NOT EXISTS idx_invitation_codes_status ON invitation_codes(status);

-- Index for listing codes by creator
CREATE INDEX IF NOT EXISTS idx_invitation_codes_created_by ON invitation_codes(created_by);
