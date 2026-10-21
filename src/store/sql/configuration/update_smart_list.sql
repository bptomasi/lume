UPDATE smart_lists
SET name = ?2,
    definition = ?3,
    enabled = 1
WHERE id = ?1
    AND (name <> ?2 OR definition <> ?3 OR enabled = 0);
