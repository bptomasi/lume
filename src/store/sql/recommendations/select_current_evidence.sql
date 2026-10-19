SELECT f.evidence
FROM recommendation_features AS f
JOIN articles AS a ON a.id = f.article_id
WHERE a.id = ?1 AND a.deleted = 0
    AND NOT EXISTS (
        SELECT 1 FROM recommendation_dirty AS dirty
        WHERE dirty.article_id = a.id AND dirty.analyze = 1
    );
