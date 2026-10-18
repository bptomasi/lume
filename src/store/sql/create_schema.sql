CREATE TABLE feeds (
    id INTEGER PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    etag TEXT NOT NULL DEFAULT '',
    modified TEXT NOT NULL DEFAULT '',
    error TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE articles (
    id INTEGER PRIMARY KEY,
    feed_id INTEGER NOT NULL REFERENCES feeds(id),
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    author TEXT NOT NULL,
    published INTEGER NOT NULL,
    body TEXT NOT NULL,
    unread INTEGER NOT NULL DEFAULT 1,
    starred INTEGER NOT NULL DEFAULT 0,
    UNIQUE (feed_id, external_id)
);

CREATE INDEX articles_published ON articles(published DESC, id DESC);

CREATE TABLE collections (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE
);

CREATE TABLE members (
    collection_id INTEGER NOT NULL
        REFERENCES collections(id) ON DELETE CASCADE,
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, article_id)
);

CREATE TABLE comments (
    id INTEGER PRIMARY KEY,
    article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    created INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX comments_article ON comments(article_id, id);

PRAGMA user_version = 1;
