INSERT INTO note_collections (name, query)
VALUES (?1, ?2)
ON CONFLICT (name) DO UPDATE
SET query = excluded.query,
    enabled = 1
WHERE note_collections.query <> excluded.query OR note_collections.enabled = 0;
