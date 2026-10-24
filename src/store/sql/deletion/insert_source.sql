-- Small JSON checkpoints are read once. Other protections use ownership
-- indexes for the selected source, without materializing the entire library.
WITH captured(article_id) AS MATERIALIZED (
    SELECT CAST(item.value AS INTEGER)
    FROM note_drafts AS d, json_tree(d.target) AS item
    WHERE item.key = 'article' AND item.type = 'integer'
    UNION
    SELECT json_extract(item.value, '$.article')
    FROM settings AS s, json_each(s.value, '$.items') AS item
    WHERE s.key IN ('offline_package', 'reading_session')
        AND (s.key = 'offline_package'
            OR CAST(item.key AS INTEGER) >= json_extract(s.value, '$.position'))
)
INSERT INTO temp.purge_articles
SELECT a.id FROM articles AS a
WHERE a.feed_id IN ({sources}) AND a.starred = 0 AND a.queued = 0
    AND NOT EXISTS (SELECT 1 FROM manual_sources WHERE feed_id = a.feed_id)
    AND NOT EXISTS (SELECT 1 FROM captured WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM members WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM comments WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM note_versions WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM note_links WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM article_tags WHERE article_id = a.id)
    AND NOT EXISTS (SELECT 1 FROM export_manifest WHERE article_id = a.id);
