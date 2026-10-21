SELECT id, name, query
FROM note_collections
WHERE enabled = 1
ORDER BY name COLLATE NOCASE;
