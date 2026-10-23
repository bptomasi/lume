SELECT 'feed', feed_id, count(*)
FROM articles WHERE deleted = 0 GROUP BY feed_id
UNION ALL
SELECT 'server', f.server_id, count(*)
FROM articles AS a JOIN feeds AS f ON f.id = a.feed_id
WHERE a.deleted = 0 AND f.server_id IS NOT NULL GROUP BY f.server_id
UNION ALL
SELECT 'collection', m.collection_id, count(*)
FROM members AS m JOIN articles AS a ON a.id = m.article_id
WHERE a.deleted = 0 GROUP BY m.collection_id;
