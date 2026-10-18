INSERT INTO feeds (id, url, title)
VALUES (1, 'https://legacy.example/rss', 'Legacy journal');

INSERT INTO articles (
    id, feed_id, external_id, title, url, author, published, body,
    unread, starred
)
VALUES (
    42, 1, 'legacy', 'Old article', 'https://legacy.example/old',
    'Ada', 100, 'Original café content', 0, 1
);

INSERT INTO comments (id, article_id, body, created)
VALUES (7, 42, 'Migration keeps my insight', 1234);

INSERT INTO collections (id, name) VALUES (3, 'Keep forever');
INSERT INTO members (collection_id, article_id) VALUES (3, 42);
