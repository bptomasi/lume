UPDATE smart_lists
SET name = ?1
WHERE id = ?2
    AND name = ?3 COLLATE BINARY
