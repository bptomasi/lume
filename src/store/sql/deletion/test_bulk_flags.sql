UPDATE articles SET unread = id % 2, starred = (id % 3 = 0),
    queued = (id % 4 = 0), progress = (id % 5 = 0), added_at = id * 100;
UPDATE articles SET canonical_key = 'https://journal.example/shared' WHERE id IN (1, 2);
INSERT INTO comments(article_id, body) SELECT id, 'Keep my note ' || id
FROM articles WHERE id % 6 = 0;
INSERT INTO recommendation_items(article_id, position)
SELECT id, row_number() OVER (ORDER BY id) FROM articles WHERE id % 7 = 0;
