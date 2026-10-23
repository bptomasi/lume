INSERT INTO article_documents(article_id, url, final_url, body, revision, fetched_at, attempt_url, attempted_at)
VALUES (?1, 'https://journal.test/story', 'https://cdn.test/page/story', ?2, 1, 1, '', 1)
ON CONFLICT(article_id) DO UPDATE SET body = excluded.body;
