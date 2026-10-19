INSERT INTO recommendation_dirty (article_id, analyze)
VALUES (?1, 0)
ON CONFLICT (article_id) DO NOTHING;
