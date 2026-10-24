INSERT INTO temp.purge_articles
SELECT id FROM articles WHERE deleted = 1;
