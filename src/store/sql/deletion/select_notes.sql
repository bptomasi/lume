INSERT OR IGNORE INTO temp.purge_notes
SELECT id FROM comments
WHERE article_id IN (SELECT id FROM temp.purge_articles)
UNION
SELECT note_id FROM note_versions
WHERE article_id IN (SELECT id FROM temp.purge_articles);
