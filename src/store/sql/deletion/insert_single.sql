INSERT INTO temp.purge_articles SELECT id FROM articles
WHERE id = ?1 AND deleted = 1;
