-- モデル名統一化: endpoint_modelsにcanonical_nameカラムを追加
ALTER TABLE endpoint_models ADD COLUMN canonical_name TEXT;

CREATE INDEX IF NOT EXISTS idx_endpoint_models_canonical ON endpoint_models(canonical_name);
