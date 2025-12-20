-- ノード管理テーブル
-- SPEC-94621a1f: ノード登録・管理
-- Phase 2: JSONファイル(nodes.json)からSQLiteへの移行

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

-- ノードGPUデバイステーブル
CREATE TABLE IF NOT EXISTS node_gpu_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    memory_bytes INTEGER
);

-- ノードロード済みモデルテーブル（全モデルタイプ統合）
CREATE TABLE IF NOT EXISTS node_loaded_models (
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    model_name TEXT NOT NULL,
    model_type TEXT NOT NULL,
    PRIMARY KEY (node_id, model_name, model_type)
);

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

-- インデックス
CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);
CREATE INDEX IF NOT EXISTS idx_nodes_machine_name ON nodes(machine_name);
CREATE INDEX IF NOT EXISTS idx_nodes_ip_address ON nodes(ip_address);
CREATE INDEX IF NOT EXISTS idx_node_gpu_devices_node_id ON node_gpu_devices(node_id);
CREATE INDEX IF NOT EXISTS idx_node_loaded_models_node_id ON node_loaded_models(node_id);
CREATE INDEX IF NOT EXISTS idx_node_loaded_models_model_type ON node_loaded_models(model_type);
