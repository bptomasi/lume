SELECT topic, headline
FROM recommendation_matches
WHERE article_id = ?1
ORDER BY topic;
