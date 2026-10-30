INSERT INTO feeds (id, url, title) VALUES (1, 'https://journal.example/feed', 'Kept source');
INSERT INTO articles (id, feed_id, external_id, title, url, author, published, body, unread, starred)
VALUES (1, 1, 'kept', 'Kept title', 'https://journal.example/page', '', 1, 'Old publisherword content.', 0, 1);
INSERT INTO article_documents (article_id, url, final_url, body, revision, fetched_at, attempt_url, attempted_at, progress)
VALUES (1, 'https://journal.example/page', 'https://journal.example/final', 'Independent quartzcache full content.', 3, 10, 'https://journal.example/page', 10, 42);
INSERT INTO comments (id, article_id, body) VALUES (1, 1, 'Kept private note');
INSERT INTO settings (key, value) VALUES ('reading_metadata_v7', '1');
