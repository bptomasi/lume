UPDATE note_collections
SET name = ?2, query = ?3, enabled = 1
WHERE id = ?1 AND (name <> ?2 OR query <> ?3 OR enabled = 0);
