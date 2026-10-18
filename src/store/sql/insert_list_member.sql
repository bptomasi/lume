INSERT INTO members (collection_id, article_id)
VALUES (?1, ?2)
ON CONFLICT DO NOTHING;
