UPDATE articles SET unread = CASE WHEN id % 2 = 0 THEN 1 ELSE 0 END,
    added_at = 1000000000;
UPDATE articles SET reading_state_at = 1000000000;
INSERT INTO article_documents(article_id, url, final_url, body, revision,
    fetched_at, attempt_url, attempted_at)
SELECT id, url, url, replace(hex(zeroblob(32)), '00', body), 1,
    1000000000, url, 1000000000 FROM articles;
