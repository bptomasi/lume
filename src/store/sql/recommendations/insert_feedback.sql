INSERT INTO recommendation_feedback (article_id, feedback)
VALUES (?1, ?2)
ON CONFLICT (article_id) DO UPDATE SET feedback = excluded.feedback;
