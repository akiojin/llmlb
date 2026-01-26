-- 初期スキーマ
-- SQLite用マイグレーション

-- 外部キー制約を有効化
PRAGMA foreign_keys = ON;

--------------------------------------------------------------------------------
-- 認証系テーブル
--------------------------------------------------------------------------------

-- ユーザーテーブル
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,  -- UUID
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,  -- bcryptハッシュ
    role TEXT NOT NULL CHECK(role IN ('admin', 'viewer')),
    created_at TEXT NOT NULL,  -- ISO8601形式
    last_login TEXT  -- ISO8601形式、NULL可
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);

-- APIキーテーブル
CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY NOT NULL,  -- UUID
    key_hash TEXT UNIQUE NOT NULL,  -- SHA-256ハッシュ
    name TEXT NOT NULL,
    created_by TEXT NOT NULL,  -- 発行したユーザーのUUID
    created_at TEXT NOT NULL,  -- ISO8601形式
    expires_at TEXT,  -- ISO8601形式、NULL可
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_created_by ON api_keys(created_by);

-- ノードトークンテーブル
CREATE TABLE IF NOT EXISTS node_tokens (
    node_id TEXT PRIMARY KEY NOT NULL,  -- ノードUUID
    token_hash TEXT UNIQUE NOT NULL,  -- SHA-256ハッシュ
    created_at TEXT NOT NULL  -- ISO8601形式
);

CREATE INDEX IF NOT EXISTS idx_node_tokens_token_hash ON node_tokens(token_hash);

--------------------------------------------------------------------------------
-- ノード管理テーブル
--------------------------------------------------------------------------------

-- メインノードテーブル
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY NOT NULL,
    machine_name TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    runtime_version TEXT NOT NULL,
    runtime_port INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'offline',
    registered_at TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    online_since TEXT,
    custom_name TEXT,
    notes TEXT,
    gpu_available INTEGER NOT NULL DEFAULT 0,
    gpu_count INTEGER,
    gpu_model TEXT,
    gpu_model_name TEXT,
    gpu_compute_capability TEXT,
    gpu_capability_score INTEGER,
    node_api_port INTEGER,
    initializing INTEGER NOT NULL DEFAULT 0,
    ready_models_current INTEGER,
    ready_models_total INTEGER
);

CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);
CREATE INDEX IF NOT EXISTS idx_nodes_machine_name ON nodes(machine_name);
CREATE INDEX IF NOT EXISTS idx_nodes_ip_address ON nodes(ip_address);

-- ノードGPUデバイステーブル
CREATE TABLE IF NOT EXISTS node_gpu_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    memory_bytes INTEGER
);

CREATE INDEX IF NOT EXISTS idx_node_gpu_devices_node_id ON node_gpu_devices(node_id);

-- ノードロード済みモデルテーブル（全モデルタイプ統合）
CREATE TABLE IF NOT EXISTS node_loaded_models (
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    model_type TEXT NOT NULL,
    PRIMARY KEY (node_id, model_name, model_type)
);

CREATE INDEX IF NOT EXISTS idx_node_loaded_models_node_id ON node_loaded_models(node_id);
CREATE INDEX IF NOT EXISTS idx_node_loaded_models_model_type ON node_loaded_models(model_type);

-- ノードタグテーブル
CREATE TABLE IF NOT EXISTS node_tags (
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (node_id, tag)
);

-- ノードサポートランタイムテーブル
CREATE TABLE IF NOT EXISTS node_supported_runtimes (
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    runtime_type TEXT NOT NULL,
    PRIMARY KEY (node_id, runtime_type)
);

--------------------------------------------------------------------------------
-- モデル管理テーブル
--------------------------------------------------------------------------------

-- メインモデルテーブル
CREATE TABLE IF NOT EXISTS models (
    name TEXT PRIMARY KEY NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    description TEXT NOT NULL DEFAULT '',
    required_memory INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'predefined',
    chat_template TEXT,
    repo TEXT,
    filename TEXT,
    last_modified TEXT,
    status TEXT
);

CREATE INDEX IF NOT EXISTS idx_models_source ON models(source);
CREATE INDEX IF NOT EXISTS idx_models_status ON models(status);

-- モデルタグテーブル
CREATE TABLE IF NOT EXISTS model_tags (
    model_name TEXT NOT NULL REFERENCES models(name) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (model_name, tag)
);

CREATE INDEX IF NOT EXISTS idx_model_tags_model_name ON model_tags(model_name);

-- モデル能力テーブル
CREATE TABLE IF NOT EXISTS model_capabilities (
    model_name TEXT NOT NULL REFERENCES models(name) ON DELETE CASCADE,
    capability TEXT NOT NULL,
    PRIMARY KEY (model_name, capability)
);

CREATE INDEX IF NOT EXISTS idx_model_capabilities_model_name ON model_capabilities(model_name);
CREATE INDEX IF NOT EXISTS idx_model_capabilities_capability ON model_capabilities(capability);

--------------------------------------------------------------------------------
-- リクエスト履歴テーブル
--------------------------------------------------------------------------------

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

CREATE INDEX IF NOT EXISTS idx_request_history_timestamp ON request_history(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_history_model ON request_history(model);
CREATE INDEX IF NOT EXISTS idx_request_history_node_id ON request_history(node_id);
CREATE INDEX IF NOT EXISTS idx_request_history_status ON request_history(status);
CREATE INDEX IF NOT EXISTS idx_request_history_timestamp_status ON request_history(timestamp DESC, status);
