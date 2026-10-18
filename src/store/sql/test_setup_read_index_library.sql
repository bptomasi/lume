INSERT INTO feeds (id, url, title)
VALUES (1, 'https://journal.example/feed', 'Journal');

INSERT INTO articles (
    id, feed_id, external_id, title, url, author, published, body,
    unread, starred, deleted
)
VALUES
    (1, 1, 'one', 'Pending', '', 'Ada', 100, 'Original body', 1, 1, 0),
    (2, 1, 'two', 'Done', '', 'Ada', 200, 'Read body', 0, 0, 0),
    (3, 1, 'three', 'Trash', '', 'Ada', 300, 'Deleted body', 1, 0, 1);

INSERT INTO comments (article_id, body)
VALUES (1, 'Keep this private note');

INSERT INTO collections (id, name) VALUES (1, 'Research');
INSERT INTO members (collection_id, article_id) VALUES (1, 1);
