SELECT s.id, s.url, s.token_env, s.enabled, s.error, s.token, s.sync_settings
FROM servers AS s
WHERE NOT EXISTS (
    SELECT 1 FROM configuration_bindings AS b
    WHERE b.kind = 'server' AND b.entity = s.id AND b.present = 0
)
ORDER BY s.url, s.id;
