SELECT id
FROM articles
WHERE feed_id = ?1 AND canonical_key = ?2 AND canonical_key <> ''
ORDER BY id
LIMIT 1;
