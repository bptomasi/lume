SELECT
    a.id,
    a.deleted,
    EXISTS (SELECT 1 FROM manual_sources WHERE feed_id = a.feed_id)
FROM articles AS a
WHERE a.url = ?1
ORDER BY a.id
LIMIT 1;
