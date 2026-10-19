INSERT INTO recommendation_matches (article_id, topic, headline)
VALUES (?1, ?2, ?3)
ON CONFLICT (article_id, topic) DO UPDATE
SET headline = excluded.headline;
