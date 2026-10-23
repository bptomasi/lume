INSERT INTO configuration_bindings (kind, entity, key)
SELECT 'server', s.id, 'server-' || lower(hex(randomblob(12)))
FROM servers AS s
WHERE NOT EXISTS (
    SELECT 1 FROM configuration_bindings AS b
    WHERE b.kind = 'server' AND b.entity = s.id
);
