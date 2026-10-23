SELECT max(id) FROM (SELECT id FROM articles WHERE id > ?1 ORDER BY id LIMIT 256);
