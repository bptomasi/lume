SELECT
    a.id,
    a.feed_id,
    a.url,
    a.canonical_key,
    a.title,
    max(1, a.reading_minutes),
    f.score,
    f.evidence
FROM recommendation_items AS r
JOIN articles AS a ON a.id = r.article_id
JOIN recommendation_features AS f ON f.article_id = a.id
WHERE a.deleted = 0
    AND a.unread = 1
    AND f.score > 0
    AND f.excluded = 0
ORDER BY r.position
LIMIT 100;
