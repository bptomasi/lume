INSERT INTO smart_lists (name, definition)
VALUES (?1, ?2)
ON CONFLICT (name) DO UPDATE
SET definition = excluded.definition,
    enabled = 1
WHERE smart_lists.definition <> excluded.definition OR smart_lists.enabled = 0;
