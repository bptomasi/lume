INSERT INTO recommendation_dirty (article_id, analyze)
SELECT DISTINCT m.article_id, 0
FROM recommendation_matches AS m
WHERE m.topic IN (
    SELECT topic FROM recommendation_feedback_topics WHERE article_id = ?1
) OR m.article_id = ?1
ON CONFLICT (article_id) DO NOTHING;
