SELECT t.topic, sum(CASE f.feedback WHEN 'useful' THEN 1 ELSE -1 END)
FROM recommendation_feedback AS f
JOIN recommendation_feedback_topics AS t ON t.article_id = f.article_id
WHERE f.feedback IN ('useful', 'not-interested')
GROUP BY t.topic;
