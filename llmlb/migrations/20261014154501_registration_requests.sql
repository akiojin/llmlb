-- 登録申請は承認されるまで users と分離して保持する。
CREATE TABLE registration_requests (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- SQLite の書き込み直列化の中で判定し、並行申請や管理者の作成/改名も保護する。
CREATE TRIGGER registration_requests_insert_guard
BEFORE INSERT ON registration_requests
BEGIN
    SELECT RAISE(ABORT, 'username_reserved')
    WHERE EXISTS (SELECT 1 FROM users WHERE username = NEW.username);
    SELECT RAISE(ABORT, 'registration_requests_full')
    WHERE (SELECT COUNT(*) FROM registration_requests) >= 100;
END;

CREATE TRIGGER registration_requests_update_guard
BEFORE UPDATE OF username ON registration_requests
BEGIN
    SELECT RAISE(ABORT, 'username_reserved')
    WHERE EXISTS (SELECT 1 FROM users WHERE username = NEW.username);
END;

CREATE TRIGGER users_registration_insert_guard
BEFORE INSERT ON users
BEGIN
    SELECT RAISE(ABORT, 'username_reserved')
    WHERE EXISTS (SELECT 1 FROM registration_requests WHERE username = NEW.username);
END;

CREATE TRIGGER users_registration_update_guard
BEFORE UPDATE OF username ON users
BEGIN
    SELECT RAISE(ABORT, 'username_reserved')
    WHERE EXISTS (SELECT 1 FROM registration_requests WHERE username = NEW.username);
END;

CREATE TRIGGER users_last_admin_delete_guard
BEFORE DELETE ON users
WHEN OLD.role = 'admin' AND (SELECT COUNT(*) FROM users WHERE role = 'admin') <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;

CREATE TRIGGER users_last_admin_update_guard
BEFORE UPDATE OF role ON users
WHEN OLD.role = 'admin' AND NEW.role != 'admin'
     AND (SELECT COUNT(*) FROM users WHERE role = 'admin') <= 1
BEGIN
    SELECT RAISE(ABORT, 'last_administrator');
END;

-- 既存 API キーの残りの権限と順序を維持したまま、廃止した権限だけ除く。
UPDATE api_keys
SET permissions = (
    SELECT json_group_array(value)
    FROM json_each(api_keys.permissions)
    WHERE value != 'invitations.manage'
)
WHERE json_valid(permissions) AND json_type(permissions) = 'array';

DROP TABLE IF EXISTS invitation_keys;
DROP TABLE IF EXISTS invitation_codes;
DROP TABLE IF EXISTS invitations;
