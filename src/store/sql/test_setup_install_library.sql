INSERT INTO feeds (id, url, title)
VALUES (1, 'https://journal.example/rss', 'Installation fixture');

INSERT INTO articles (
    id,
    feed_id,
    external_id,
    title,
    url,
    author,
    published,
    body,
    unread,
    starred
)
VALUES (
    42,
    1,
    'migration-fixture',
    'Preserved article',
    'https://journal.example/article',
    'Fixture author',
    1700000000,
    'Publisher content survives installation.',
    0,
    1
);

INSERT INTO comments (article_id, body)
VALUES (42, 'A personal annotation that must survive migration.');

INSERT INTO collections (id, name)
VALUES (7, 'Systems study');

INSERT INTO members (collection_id, article_id)
VALUES (7, 42);
