INSERT INTO collections (id, name)
VALUES ((SELECT last_id + 1 FROM definition_identity_clock WHERE kind = 'folder'), ?1)
ON CONFLICT (name) DO UPDATE SET enabled = 1
WHERE collections.enabled = 0;
