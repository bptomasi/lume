SELECT count(*) FROM servers AS s
WHERE s.url <> ?1 AND NOT EXISTS (
    SELECT 1 FROM configuration_bindings AS b
    WHERE b.kind = 'server' AND b.entity = s.id AND b.present = 0
);
