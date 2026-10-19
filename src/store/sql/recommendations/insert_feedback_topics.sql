INSERT INTO recommendation_feedback_topics (article_id, topic)
SELECT article_id, topic
FROM recommendation_matches
WHERE article_id = ?1;
