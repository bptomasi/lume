UPDATE articles SET body = '![Kept](https://images.example/upgrade)' WHERE id = 1;
INSERT INTO image_references(article_id, source, url)
VALUES (1, 0, 'https://images.example/upgrade');
UPDATE image_reference_dirty SET generation = 5, dirty = 0 WHERE article_id = 1;
