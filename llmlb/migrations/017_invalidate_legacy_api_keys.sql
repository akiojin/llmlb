-- Invalidate all legacy API keys issued before self-service key management.
DELETE FROM api_keys;
