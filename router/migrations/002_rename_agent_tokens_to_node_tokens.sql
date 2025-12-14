-- Rename agent_tokens to node_tokens (agent -> node terminology unification)
--
-- NOTE: This is a breaking change and intentionally drops the old table/index names.

ALTER TABLE agent_tokens RENAME TO node_tokens;
ALTER TABLE node_tokens RENAME COLUMN agent_id TO node_id;

DROP INDEX IF EXISTS idx_agent_tokens_token_hash;
CREATE INDEX IF NOT EXISTS idx_node_tokens_token_hash ON node_tokens(token_hash);
