ALTER TABLE feeds
ADD COLUMN article_policy TEXT NOT NULL DEFAULT 'missing'
    CHECK (article_policy IN ('missing', 'always', 'manual'));

ALTER TABLE comments
ADD COLUMN source TEXT NOT NULL DEFAULT 'feed'
    CHECK (source IN ('feed', 'full'));

ALTER TABLE comments
ADD COLUMN source_revision INTEGER NOT NULL DEFAULT 0
    CHECK (source_revision >= 0);

CREATE TABLE article_documents (
    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    url TEXT NOT NULL DEFAULT '',
    final_url TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT ''
        CHECK (length(CAST(body AS BLOB)) <= 524288),
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    fetched_at INTEGER NOT NULL DEFAULT 0,
    attempt_url TEXT NOT NULL,
    attempted_at INTEGER NOT NULL,
    error TEXT NOT NULL DEFAULT '',
    progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0)
);

DROP TRIGGER articles_search_update;

CREATE TRIGGER articles_search_update
AFTER UPDATE OF title, author, body, url ON articles
BEGIN
    UPDATE article_search
    SET
        title = NEW.title,
        author = NEW.author,
        body = NEW.body || char(10) || coalesce((
            SELECT d.body
            FROM article_documents AS d
            WHERE d.article_id = NEW.id
                AND d.url = NEW.url
        ), '')
    WHERE rowid = NEW.id;
END;

CREATE TRIGGER article_documents_search_insert
AFTER INSERT ON article_documents
BEGIN
    UPDATE article_search
    SET body = (
        SELECT a.body || char(10) || CASE
            WHEN a.url = NEW.url THEN NEW.body
            ELSE ''
        END
        FROM articles AS a
        WHERE a.id = NEW.article_id
    )
    WHERE rowid = NEW.article_id;
END;

CREATE TRIGGER article_documents_search_update
AFTER UPDATE OF body, url ON article_documents
BEGIN
    UPDATE article_search
    SET body = (
        SELECT a.body || char(10) || CASE
            WHEN a.url = NEW.url THEN NEW.body
            ELSE ''
        END
        FROM articles AS a
        WHERE a.id = NEW.article_id
    )
    WHERE rowid = NEW.article_id;
END;

PRAGMA user_version = 4;
