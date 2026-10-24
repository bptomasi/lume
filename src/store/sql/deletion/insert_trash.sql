INSERT INTO temp.purge_articles
SELECT id FROM articles WHERE deleted = 1 AND (?1 IS NULL OR id = ?1);
