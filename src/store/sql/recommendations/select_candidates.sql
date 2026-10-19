SELECT
    a.id,
    a.feed_id,
    a.canonical_key,
    f.score,
    f.evidence,
    a.published
FROM recommendation_features AS f INDEXED BY recommendation_score
JOIN articles AS a ON a.id = f.article_id
WHERE f.score > 0
    AND f.excluded = 0
    AND a.deleted = 0
    AND (?1 OR a.unread = 1)
    AND (?2 IS NULL OR f.minutes <= ?2)
    AND NOT EXISTS (
        SELECT 1
        FROM recommendation_feedback AS feedback
        JOIN articles AS rated ON rated.id = feedback.article_id
        WHERE feedback.feedback != 'useful'
            AND (
                rated.id = a.id
                OR (a.canonical_key != ''
                    AND rated.canonical_key = a.canonical_key)
            )
    )
ORDER BY f.score DESC, f.article_id DESC
LIMIT 512;
