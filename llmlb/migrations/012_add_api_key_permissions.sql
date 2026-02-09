-- APIキーにpermissions列を追加（スコープからの移行）
--
-- 旧: scopes = ["api"|"endpoint"|"admin"] (JSON配列, TEXT)
-- 新: permissions = ["openai.inference", ...] (JSON配列, TEXT)
--
-- 後方互換:
-- - scopes が NULL のキーは旧実装で「全権限」として扱われていたため、permissions も全権限へ移行する。

ALTER TABLE api_keys ADD COLUMN permissions TEXT;

-- Backfill permissions from legacy scopes.
UPDATE api_keys
SET permissions = CASE
    -- scopes 未設定（旧互換: 全権限）
    WHEN scopes IS NULL THEN '["openai.inference","openai.models.read","endpoints.read","endpoints.manage","api_keys.manage","users.manage","invitations.manage","models.manage","registry.read","logs.read","metrics.read"]'
    -- admin スコープ（旧互換: 全権限）
    WHEN instr(scopes, '"admin"') > 0 THEN '["openai.inference","openai.models.read","endpoints.read","endpoints.manage","api_keys.manage","users.manage","invitations.manage","models.manage","registry.read","logs.read","metrics.read"]'
    -- api + endpoint
    WHEN instr(scopes, '"api"') > 0 AND instr(scopes, '"endpoint"') > 0 THEN '["openai.inference","openai.models.read","registry.read"]'
    -- api
    WHEN instr(scopes, '"api"') > 0 THEN '["openai.inference","openai.models.read"]'
    -- endpoint
    WHEN instr(scopes, '"endpoint"') > 0 THEN '["registry.read"]'
    ELSE '[]'
END
WHERE permissions IS NULL;

