INSERT INTO article_tags (article_id, tag)
SELECT id, 'systems' FROM articles WHERE id % 3 = 0;
