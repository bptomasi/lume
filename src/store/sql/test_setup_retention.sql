INSERT INTO feeds(id, url, title) VALUES (1, 'https://journal.example/feed', 'Journal');
WITH RECURSIVE ids(id) AS (VALUES(1) UNION ALL SELECT id + 1 FROM ids WHERE id < 24)
INSERT INTO articles(id, feed_id, external_id, title, url, author, published,
    body, unread, added_at, canonical_key)
SELECT id, 1, 'entry:' || id, 'Article ' || id,
    'https://journal.example/' || id, 'Author', 1000000000,
    'Original feed body ' || id, 0, 1000000000,
    'https://journal.example/' || id FROM ids;
UPDATE articles SET reading_state_at = 1000000000;
INSERT INTO article_documents(article_id, url, final_url, body, revision,
    fetched_at, attempt_url, attempted_at)
SELECT id, url, url, 'Full quartzcache body ' || id, 1,
    1000000000, url, 1000000000 FROM articles;
