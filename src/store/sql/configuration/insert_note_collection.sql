INSERT INTO note_collections (id, name, query)
VALUES ((SELECT last_id + 1 FROM definition_identity_clock WHERE kind = 'notes'), ?1, ?2)
ON CONFLICT (name) DO UPDATE
SET query = excluded.query,
    enabled = 1
WHERE note_collections.query <> excluded.query OR note_collections.enabled = 0;
