SELECT s.id, b.key, s.url, s.token_env, s.enabled, s.token, s.sync_settings
FROM servers AS s
JOIN configuration_bindings AS b
    ON b.kind = 'server' AND b.entity = s.id
WHERE b.present = 1
ORDER BY b.key;
