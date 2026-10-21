INSERT INTO configuration_bindings (kind, entity, key, present)
VALUES (?1, ?2, ?3, 1)
ON CONFLICT (kind, entity) DO UPDATE
SET key = excluded.key, present = 1;
