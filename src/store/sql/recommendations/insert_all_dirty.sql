INSERT INTO recommendation_dirty (article_id, analyze)
SELECT id, ?1 FROM articles WHERE true
ON CONFLICT (article_id) DO UPDATE
SET analyze = max(recommendation_dirty.analyze, excluded.analyze);
