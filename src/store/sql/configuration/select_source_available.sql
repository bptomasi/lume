SELECT f.server_id IS NULL OR COALESCE(b.present, 0) = 1
FROM feeds AS f
LEFT JOIN configuration_bindings AS b
    ON b.kind = 'server' AND b.entity = f.server_id
WHERE f.id = ?1;
