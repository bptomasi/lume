INSERT INTO servers (url, token_env, enabled)
VALUES (?1, ?2, ?3)
ON CONFLICT (url) DO UPDATE SET
    token_env = excluded.token_env,
    enabled = excluded.enabled
WHERE servers.token_env <> excluded.token_env
    OR servers.enabled <> excluded.enabled;
