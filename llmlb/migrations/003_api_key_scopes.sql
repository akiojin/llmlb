-- APIキーにスコープ列を追加（後方互換: NULL = 全権限）

ALTER TABLE api_keys ADD COLUMN scopes TEXT;
