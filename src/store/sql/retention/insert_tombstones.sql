INSERT INTO retention_tombstones(feed_id, external_id, canonical_key, removed_at)
SELECT feed_id, external_id, canonical_key, ?2 FROM articles WHERE id = ?1
UNION ALL
SELECT a.feed_id, 'miniflux:' || e.remote_id, a.canonical_key, ?2
FROM server_entries AS e JOIN articles AS a ON a.id = e.article_id
WHERE a.id = ?1
ON CONFLICT(feed_id, external_id) DO NOTHING;
