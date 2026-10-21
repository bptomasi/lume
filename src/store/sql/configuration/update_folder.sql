UPDATE collections
SET name = ?2, enabled = 1
WHERE id = ?1 AND (name <> ?2 OR enabled = 0);
