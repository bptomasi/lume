WITH protected AS MATERIALIZED ({protections})
INSERT INTO temp.purge_articles
SELECT a.id FROM articles AS a
WHERE a.feed_id IN ({sources})
    AND NOT EXISTS (
        SELECT 1 FROM protected AS p WHERE p.article_id = a.id
            AND p.reason NOT IN ('undo history', 'workspace article')
    )
    AND NOT EXISTS (
        SELECT 1 FROM note_versions AS v WHERE v.article_id = a.id
    );
