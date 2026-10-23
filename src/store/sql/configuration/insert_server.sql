INSERT INTO servers (url, token_env, enabled, token)
VALUES (?1, ?2, ?3, ?4)
ON CONFLICT (url) DO UPDATE SET
    token_env = excluded.token_env,
    enabled = excluded.enabled,
    token = excluded.token
WHERE servers.token_env <> excluded.token_env
    OR servers.enabled <> excluded.enabled
    OR servers.token <> excluded.token;
