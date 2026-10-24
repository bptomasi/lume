INSERT INTO smart_lists (id, name, definition)
VALUES ((SELECT last_id + 1 FROM definition_identity_clock WHERE kind = 'smart'), ?1, ?2)
ON CONFLICT (name) DO UPDATE
SET definition = excluded.definition,
    enabled = 1
WHERE smart_lists.definition <> excluded.definition OR smart_lists.enabled = 0;
