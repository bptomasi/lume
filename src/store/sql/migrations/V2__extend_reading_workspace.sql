ALTER TABLE articles ADD COLUMN queued INTEGER NOT NULL DEFAULT 0;
ALTER TABLE articles ADD COLUMN progress INTEGER NOT NULL DEFAULT 0;
ALTER TABLE comments ADD COLUMN quote TEXT NOT NULL DEFAULT '';
ALTER TABLE comments ADD COLUMN anchor INTEGER NOT NULL DEFAULT 0;

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE smart_lists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    definition TEXT NOT NULL
);

CREATE TABLE undo_log (
    id INTEGER PRIMARY KEY,
    operation TEXT NOT NULL,
    payload TEXT NOT NULL,
    created INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE VIRTUAL TABLE article_search USING fts5(
    title,
    feed,
    author,
    body,
    notes,
    tokenize = 'unicode61 remove_diacritics 2'
);

INSERT INTO article_search (rowid, title, feed, author, body, notes)
SELECT
    a.id,
    a.title,
    f.title,
    a.author,
    a.body,
    coalesce((
        SELECT group_concat(c.body, ' ')
        FROM comments AS c
        WHERE c.article_id = a.id
    ), '')
FROM articles AS a
JOIN feeds AS f ON f.id = a.feed_id;

CREATE TRIGGER articles_search_insert AFTER INSERT ON articles
BEGIN
    INSERT INTO article_search (rowid, title, feed, author, body, notes)
    SELECT NEW.id, NEW.title, f.title, NEW.author, NEW.body, ''
    FROM feeds AS f WHERE f.id = NEW.feed_id;
END;

CREATE TRIGGER articles_search_update
AFTER UPDATE OF title, author, body ON articles
BEGIN
    UPDATE article_search
    SET title = NEW.title, author = NEW.author, body = NEW.body
    WHERE rowid = NEW.id;
END;

CREATE TRIGGER feeds_search_update AFTER UPDATE OF title ON feeds
BEGIN
    UPDATE article_search SET feed = NEW.title
    WHERE rowid IN (SELECT id FROM articles WHERE feed_id = NEW.id);
END;

CREATE TRIGGER articles_search_delete AFTER DELETE ON articles
BEGIN
    DELETE FROM article_search WHERE rowid = OLD.id;
END;

CREATE TRIGGER comments_search_insert AFTER INSERT ON comments
BEGIN
    UPDATE article_search
    SET notes = coalesce((
        SELECT group_concat(body || ' ' || quote, ' ')
        FROM comments WHERE article_id = NEW.article_id
    ), '')
    WHERE rowid = NEW.article_id;
END;

CREATE TRIGGER comments_search_update AFTER UPDATE ON comments
BEGIN
    UPDATE article_search
    SET notes = coalesce((
        SELECT group_concat(body || ' ' || quote, ' ')
        FROM comments WHERE article_id = NEW.article_id
    ), '')
    WHERE rowid = NEW.article_id;
END;

CREATE TRIGGER comments_search_delete AFTER DELETE ON comments
BEGIN
    UPDATE article_search
    SET notes = coalesce((
        SELECT group_concat(body || ' ' || quote, ' ')
        FROM comments WHERE article_id = OLD.article_id
    ), '')
    WHERE rowid = OLD.article_id;
END;

PRAGMA user_version = 2;
