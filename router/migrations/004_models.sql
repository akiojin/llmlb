-- モデル管理テーブル
-- SPEC-dcaeaec4: モデルメタデータ管理
-- Phase 3: JSONファイル(models.json)からSQLiteへの移行

-- メインモデルテーブル
CREATE TABLE IF NOT EXISTS models (
    name TEXT PRIMARY KEY NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    description TEXT NOT NULL DEFAULT '',
    required_memory INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'predefined',
    download_url TEXT,
    path TEXT,
    chat_template TEXT,
    repo TEXT,
    filename TEXT,
    last_modified TEXT,
    status TEXT
);

-- モデルタグテーブル
CREATE TABLE IF NOT EXISTS model_tags (
    model_name TEXT NOT NULL REFERENCES models(name) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (model_name, tag)
);

-- モデル能力テーブル
CREATE TABLE IF NOT EXISTS model_capabilities (
    model_name TEXT NOT NULL REFERENCES models(name) ON DELETE CASCADE,
    capability TEXT NOT NULL,
    PRIMARY KEY (model_name, capability)
);

-- インデックス
CREATE INDEX IF NOT EXISTS idx_models_source ON models(source);
CREATE INDEX IF NOT EXISTS idx_models_status ON models(status);
CREATE INDEX IF NOT EXISTS idx_model_tags_model_name ON model_tags(model_name);
CREATE INDEX IF NOT EXISTS idx_model_capabilities_model_name ON model_capabilities(model_name);
CREATE INDEX IF NOT EXISTS idx_model_capabilities_capability ON model_capabilities(capability);
