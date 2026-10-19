SELECT f.article_id, f.minutes, f.evidence, f.excluded, a.starred, a.feed_id
FROM recommendation_features AS f
JOIN articles AS a ON a.id = f.article_id
WHERE f.article_id IN (SELECT value FROM json_each(?1));
