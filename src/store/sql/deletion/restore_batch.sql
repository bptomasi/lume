UPDATE articles SET deleted = 0
WHERE id IN (SELECT value FROM json_each(?1));
