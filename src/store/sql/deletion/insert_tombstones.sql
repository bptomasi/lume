INSERT INTO retention_tombstones
    (feed_id, external_id, canonical_key, removed_at)
SELECT feed_id, external_id, canonical_key, unixepoch()
FROM articles WHERE id IN (SELECT id FROM temp.purge_articles)
UNION ALL
SELECT a.feed_id, 'miniflux:' || e.remote_id, a.canonical_key, unixepoch()
FROM server_entries AS e JOIN articles AS a ON a.id = e.article_id
WHERE a.id IN (SELECT id FROM temp.purge_articles)
ON CONFLICT(feed_id, external_id) DO NOTHING;
